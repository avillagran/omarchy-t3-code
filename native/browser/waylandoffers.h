#pragma once
// Bounded member on the actual Qt compositor connection. Initialization is
// asynchronous; only new private proxies use this queue. Toolkit-owned proxies,
// listeners and the default event queue remain under Qt ownership.
#include <QGuiApplication>
#include <QtGui/qguiapplication_platform.h>
#include <QSocketNotifier>
#include <QTimer>
#include <QEventLoop>
#include <QElapsedTimer>
#include <QThread>
#include <QStringList>
#include <wayland-client.h>
#include <memory>
#include <unordered_map>
#include <optional>
#include <functional>
#include <cstring>
#include <vector>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>

class NativeWaylandOffers final:public QObject {
 struct Offer {wl_data_offer *proxy=nullptr;QStringList formats;size_t metadata=0;bool invalid=false;~Offer(){if(proxy)wl_data_offer_destroy(proxy);}};
 wl_display *display=nullptr;wl_event_queue *queue=nullptr;wl_registry *registry=nullptr;wl_data_device_manager *manager=nullptr;wl_data_device *device=nullptr;wl_seat *seat=nullptr;
 std::unordered_map<wl_data_offer *,std::shared_ptr<Offer>> known;std::shared_ptr<Offer> selected;QTimer dispatch;bool usable=false;qsizetype retained=0;
 static void mime(void *data,wl_data_offer *,const char *name){auto *offer=static_cast<Offer *>(data);const auto size=strnlen(name,4097);if(offer->invalid)return;if(size>4096||offer->formats.size()>=128||offer->metadata+size>65536){offer->invalid=true;offer->formats.clear();return;}offer->metadata+=size;offer->formats.append(QString::fromUtf8(name,int(size)));}
 static void sourceActions(void *,wl_data_offer *,uint32_t){}static void action(void *,wl_data_offer *,uint32_t){}
 static void offered(void *data,wl_data_device *,wl_data_offer *proxy){auto *self=static_cast<NativeWaylandOffers *>(data);if(self->known.size()>=32){wl_data_offer_destroy(proxy);return;}auto offer=std::make_shared<Offer>();offer->proxy=proxy;self->known.emplace(proxy,offer);static const wl_data_offer_listener listener{mime,sourceActions,action};wl_data_offer_add_listener(proxy,&listener,offer.get());}
 static void enter(void *,wl_data_device *,uint32_t,wl_surface *,wl_fixed_t,wl_fixed_t,wl_data_offer *){}static void leave(void *,wl_data_device *){}static void motion(void *,wl_data_device *,uint32_t,wl_fixed_t,wl_fixed_t){}static void drop(void *,wl_data_device *){}
 static void selection(void *data,wl_data_device *,wl_data_offer *proxy){auto *self=static_cast<NativeWaylandOffers *>(data);const auto found=self->known.find(proxy);self->selected=found==self->known.end()?nullptr:found->second;for(auto it=self->known.begin();it!=self->known.end();)if(it->second!=self->selected&&it->second.use_count()==1)it=self->known.erase(it);else++it;if(self->changed)self->changed();}
 static void global(void *data,wl_registry *registry,uint32_t name,const char *interface,uint32_t version){auto *self=static_cast<NativeWaylandOffers *>(data);if(strcmp(interface,wl_data_device_manager_interface.name)==0&&!self->manager){self->manager=static_cast<wl_data_device_manager *>(wl_registry_bind(registry,name,&wl_data_device_manager_interface,qMin(version,uint32_t(3))));self->device=wl_data_device_manager_get_data_device(self->manager,self->seat);static const wl_data_device_listener listener{offered,enter,leave,motion,drop,selection};wl_data_device_add_listener(self->device,&listener,self);self->usable=true;wl_display_flush(self->display);}}
 static void removed(void *,wl_registry *,uint32_t){}
 void pending(){if(display&&queue&&wl_display_dispatch_queue_pending(display,queue)<0){usable=false;selected.reset();if(changed)changed();}}
 public:
 std::function<void()> changed;
 quint64 streamedBytes=0,maxRetainedBytes=0,maxAssemblyCapacity=0,maxBufferEstimate=0,limitFailures=0;QString lastError;
 explicit NativeWaylandOffers(QObject *parent=nullptr):QObject(parent){
  auto *native=qGuiApp->nativeInterface<QNativeInterface::QWaylandApplication>();if(!native)return;display=native->display();seat=native->seat();if(!display||!seat)return;
  queue=wl_display_create_queue(display);if(!queue)return;
  auto *wrapper=static_cast<wl_display *>(wl_proxy_create_wrapper(display));if(!wrapper)return;wl_proxy_set_queue(reinterpret_cast<wl_proxy *>(wrapper),queue);registry=wl_display_get_registry(wrapper);wl_proxy_wrapper_destroy(wrapper);
  static const wl_registry_listener registryListener{global,removed};wl_registry_add_listener(registry,&registryListener,this);wl_display_flush(display);
  dispatch.setInterval(5);connect(&dispatch,&QTimer::timeout,this,[this]{pending();});dispatch.start();
 }
 ~NativeWaylandOffers()override{dispatch.stop();changed={};pending();usable=false;for(auto &entry:known){if(entry.second->proxy){wl_data_offer_destroy(entry.second->proxy);entry.second->proxy=nullptr;}}selected.reset();known.clear();if(device){if(wl_proxy_get_version(reinterpret_cast<wl_proxy *>(device))>=2)wl_data_device_release(device);else wl_data_device_destroy(device);}if(manager)wl_data_device_manager_destroy(manager);if(registry)wl_registry_destroy(registry);if(queue)wl_event_queue_destroy(queue);}
 bool ready()const{return usable;}
 std::shared_ptr<const void> identity()const{return selected;}
 QStringList formats(){pending();return selected&&!selected->invalid?selected->formats:QStringList();}
 bool metadataAllowed(){pending();return selected&&!selected->invalid;}
 std::optional<QByteArray> read(const QString &requested,qsizetype limit,const std::function<bool()> &admitted){
  pending();lastError.clear();auto offer=selected;if(!usable||!offer||offer->invalid||!admitted()){lastError="offer_unavailable";return std::nullopt;}QString format=requested;
  if(format=="text/plain"){if(offer->formats.contains("text/plain;charset=utf-8"))format="text/plain;charset=utf-8";else if(!offer->formats.contains(format)){lastError="format_unavailable";return std::nullopt;}}
  if(!offer->formats.contains(format)){lastError="format_unavailable";return std::nullopt;}
  int descriptors[2];if(pipe2(descriptors,O_CLOEXEC)){lastError="offer_pipe_failed";return std::nullopt;}
  if(fcntl(descriptors[0],F_SETFL,fcntl(descriptors[0],F_GETFL)|O_NONBLOCK)<0){close(descriptors[0]);close(descriptors[1]);lastError="offer_pipe_failed";return std::nullopt;}
  const auto encoded=format.toUtf8();wl_data_offer_receive(offer->proxy,encoded.constData(),descriptors[1]);close(descriptors[1]);wl_display_flush(display);
  constexpr qsizetype block=65536;std::vector<std::unique_ptr<char[]>> blocks;qsizetype used=0,reserved=0;bool done=false,failed=false;QString error;QElapsedTimer timer;timer.start();
  auto drain=[&]{char chunk[block];while(!done){if(!admitted()){error="lease_revoked";failed=true;done=true;break;}const auto size=::read(descriptors[0],chunk,sizeof chunk);if(size>0){streamedBytes+=quint64(size);if(size>limit-used){limitFailures++;error="stream_limit";failed=true;done=true;break;}qsizetype at=0;while(at<size){const auto index=used/block,offset=used%block;if(index==qsizetype(blocks.size())){if(block>limit-retained){limitFailures++;error="stream_limit";failed=true;done=true;break;}blocks.push_back(std::make_unique<char[]>(block));reserved+=block;retained+=block;maxRetainedBytes=qMax(maxRetainedBytes,quint64(retained));}const auto count=qMin<qsizetype>(block-offset,size-at);memcpy(blocks[size_t(index)].get()+offset,chunk+at,size_t(count));used+=count;at+=count;}}else if(size==0){done=true;}else{if(errno!=EAGAIN&&errno!=EWOULDBLOCK&&errno!=EINTR){error="offer_read_failed";failed=true;done=true;}break;}}};
  QSocketNotifier readable(descriptors[0],QSocketNotifier::Read);connect(&readable,&QSocketNotifier::activated,this,drain);
  while(!done&&timer.elapsed()<5000&&usable&&offer==selected&&admitted()){pending();drain();if(done)break;QCoreApplication::processEvents(QEventLoop::ExcludeUserInputEvents,2);QThread::msleep(1);}
  readable.setEnabled(false);close(descriptors[0]);pending();if(!done||failed||!usable||offer!=selected||!admitted()){retained-=reserved;lastError=error.isEmpty()?(offer!=selected?"offer_replaced":"offer_timeout"):error;return std::nullopt;}
  QByteArray result(used,Qt::Uninitialized);maxAssemblyCapacity=qMax(maxAssemblyCapacity,quint64(result.capacity()));maxBufferEstimate=qMax(maxBufferEstimate,quint64(retained)+quint64(result.capacity()));qsizetype at=0;for(const auto &chunk:blocks){const auto count=qMin<qsizetype>(block,used-at);memcpy(result.data()+at,chunk.get(),size_t(count));at+=count;}retained-=reserved;return result;
 }
};
