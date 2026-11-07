// Isolated feasibility fixture. Production requires the lifetime/stream bounds
// documented in docs/NATIVE_CLIPBOARD_PROXY.md; this is not a product plugin.
#include <QGuiApplication>
#include <QRasterWindow>
#include <QPainter>
#include <QClipboard>
#include <QMimeData>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSocketNotifier>
#include <QTimer>
#include <QElapsedTimer>
#include <QHash>
#include <xcb/xcb.h>
#include <xcb/xfixes.h>
#include <xcb/res.h>
#include <unistd.h>
#include <fcntl.h>
#include <cstdio>
#include <cstdlib>
#include <memory>
#include <cstring>
#include <QVector>
#include <QEventLoop>
#include <QThread>
#include <QImage>
#include <functional>

class ProxyWindow final:public QRasterWindow {
 void paintEvent(QPaintEvent *)override{QPainter painter(this);painter.fillRect(QRect(QPoint(),size()),QColor("#dddddd"));}
};
class OfferedMime final:public QMimeData {
 QStringList offered;std::function<QVariant(const QString &,QMetaType)> retrieve;
 public:OfferedMime(QStringList formats,std::function<QVariant(const QString &,QMetaType)> getter):offered(std::move(formats)),retrieve(std::move(getter)){}
 QStringList formats()const override{return offered;}
 protected:QVariant retrieveData(const QString &mime,QMetaType type)const override{return retrieve(mime,type);}
};
struct Receiving {xcb_window_t window=0,owner=0;xcb_atom_t target=0;QByteArray data;bool active=false,done=false,failed=false,incremental=false;uint8_t format=0;};
struct Transfer {xcb_atom_t type;QByteArray bytes;qsizetype offset=0;qint64 deadline;quint64 lease;};
int main(int argc,char **argv){
 QGuiApplication app(argc,argv);if(argc!=2)return 2;
 const auto engine=quint32(getppid());QElapsedTimer clock;clock.start();
 ProxyWindow window;window.setTitle("Owned clipboard platform proxy");window.resize(360,240);window.show();
 int screenNumber=0;auto *connection=xcb_connect(argv[1],&screenNumber);if(!connection||xcb_connection_has_error(connection))return 2;
 const auto *extension=xcb_get_extension_data(connection,&xcb_xfixes_id);const auto *resource=xcb_get_extension_data(connection,&xcb_res_id);if(!extension||!extension->present||!resource||!resource->present)return 2;
 auto *version=xcb_xfixes_query_version_reply(connection,xcb_xfixes_query_version(connection,5,0),nullptr);if(!version)return 2;free(version);
 auto *resVersion=xcb_res_query_version_reply(connection,xcb_res_query_version(connection,1,2),nullptr);if(!resVersion||resVersion->server_major<1||(resVersion->server_major==1&&resVersion->server_minor<2))return 2;free(resVersion);
 auto screens=xcb_setup_roots_iterator(xcb_get_setup(connection));for(int i=0;i<screenNumber;i++)xcb_screen_next(&screens);if(!screens.rem)return 2;const auto root=screens.data->root;
 auto atom=[&](const char *name){auto *reply=xcb_intern_atom_reply(connection,xcb_intern_atom(connection,0,uint16_t(strlen(name)),name),nullptr);if(!reply)return xcb_atom_t(0);const auto value=reply->atom;free(reply);return value;};
 const auto clipboard=atom("CLIPBOARD"),targets=atom("TARGETS"),timestamp=atom("TIMESTAMP"),incr=atom("INCR"),utf8=atom("UTF8_STRING"),text=atom("text/plain;charset=utf-8"),plain=atom("text/plain"),png=atom("image/png"),html=atom("text/html");
 const xcb_window_t owners[]={xcb_generate_id(connection),xcb_generate_id(connection)};
 for(auto owner:owners)xcb_create_window(connection,XCB_COPY_FROM_PARENT,owner,root,0,0,1,1,0,XCB_WINDOW_CLASS_INPUT_OUTPUT,XCB_COPY_FROM_PARENT,0,nullptr);
 xcb_xfixes_select_selection_input(connection,owners[0],clipboard,XCB_XFIXES_SELECTION_EVENT_MASK_SET_SELECTION_OWNER|XCB_XFIXES_SELECTION_EVENT_MASK_SELECTION_WINDOW_DESTROY|XCB_XFIXES_SELECTION_EVENT_MASK_SELECTION_CLIENT_CLOSE);
 auto lifetime=std::make_shared<bool>(true);auto *system=QGuiApplication::clipboard();int ownerIndex=0;qint64 allowedUntil=-1;quint64 lease=0;quint64 dataReads=0,metadataReads=0,denied=0,foreign=0,synthetic=0,transferred=0,fetchedBytes=0,maxFetchedBytes=0,capDenied=0;bool alive=true,paused=false,publishing=false;xcb_timestamp_t selectedAt=0;
 QHash<quint64,Transfer> pending;Receiving receiving;xcb_window_t nativeOwner=0;xcb_timestamp_t nativeStamp=0;qint64 writeUntil=-1;quint64 publications=0,egressReads=0,egressBytes=0,egressDenied=0;QString lastEgressMime;bool lastEgressDuringPublish=false;
 std::function<void()> pump;std::function<void(xcb_window_t,xcb_timestamp_t)> publish;

 auto reply=[](QJsonObject value){const auto line=QJsonDocument(value).toJson(QJsonDocument::Compact);fprintf(stdout,"%s\n",line.constData());fflush(stdout);};
 auto announce=[&]{ownerIndex^=1;xcb_set_selection_owner(connection,owners[ownerIndex],clipboard,XCB_CURRENT_TIME);xcb_flush(connection);};
 QObject::connect(system,&QClipboard::dataChanged,&app,[&]{if(!publishing&&!system->ownsClipboard()){nativeOwner=0;if(receiving.active){receiving.failed=true;receiving.done=true;}announce();}});
 announce();
 auto valid=[&]{return alive&&allowedUntil>0&&allowedUntil>=clock.elapsed();};
 auto notify=[&](const xcb_selection_request_event_t &request,xcb_atom_t property){xcb_selection_notify_event_t response{};response.response_type=XCB_SELECTION_NOTIFY;response.time=request.time;response.requestor=request.requestor;response.selection=request.selection;response.target=request.target;response.property=property;xcb_send_event(connection,0,request.requestor,XCB_EVENT_MASK_NO_EVENT,reinterpret_cast<const char *>(&response));xcb_flush(connection);};
 auto client=[&](xcb_window_t requestor){xcb_res_client_id_spec_t spec{requestor,XCB_RES_CLIENT_ID_MASK_LOCAL_CLIENT_PID};auto *result=xcb_res_query_client_ids_reply(connection,xcb_res_query_client_ids(connection,1,&spec),nullptr);quint32 pid=0;if(result){auto ids=xcb_res_query_client_ids_ids_iterator(result);for(;ids.rem;xcb_res_client_id_value_next(&ids))if((ids.data->spec.mask&XCB_RES_CLIENT_ID_MASK_LOCAL_CLIENT_PID)&&xcb_res_client_id_value_value_length(ids.data)==1)pid=*xcb_res_client_id_value_value(ids.data);free(result);}return pid;};
 QByteArray input;
 auto controls=[&]{char bytes[4096];ssize_t size=0;while((size=read(STDIN_FILENO,bytes,sizeof bytes))>0){input.append(bytes,size);if(input.size()>4096){alive=false;app.exit(2);return;}}
  if(size==0){alive=false;allowedUntil=-1;pending.clear();app.quit();return;}
  int end;while((end=input.indexOf('\n'))>=0){const auto document=QJsonDocument::fromJson(input.left(end));input.remove(0,end+1);if(!document.isObject()){app.exit(2);return;}const auto row=document.object();const auto op=row["op"].toString();
   if(op=="grant"){if(!valid())lease++;allowedUntil=clock.elapsed()+5000;}
   else if(op=="revoke"){allowedUntil=-1;lease++;for(auto entry=pending.cbegin();entry!=pending.cend();++entry)xcb_change_property(connection,XCB_PROP_MODE_REPLACE,xcb_window_t(entry.key()>>32),xcb_atom_t(entry.key()),entry->type,8,0,nullptr);pending.clear();announce();if(row.contains("id"))reply({{"id",row["id"]},{"revoked",true}});}
   else if(op=="human-copy"){writeUntil=clock.elapsed()+1000;if(row.contains("id"))reply({{"id",row["id"]},{"armed",true}});}
   else if(op=="pause"){paused=true;if(row.contains("id"))reply({{"id",row["id"]},{"paused",true}});}
   else if(op=="stats")reply({{"id",row["id"]},{"dataReads",qint64(dataReads)},{"metadataReads",qint64(metadataReads)},{"denied",qint64(denied)},{"foreign",qint64(foreign)},{"synthetic",qint64(synthetic)},{"bytes",qint64(transferred)},{"capDenied",qint64(capDenied)},{"fetchedBytes",qint64(fetchedBytes)},{"maxFetchedBytes",qint64(maxFetchedBytes)},{"activeTransfers",pending.size()},{"focused",window.isActive()},{"publications",qint64(publications)},{"egressReads",qint64(egressReads)},{"egressBytes",qint64(egressBytes)},{"egressDenied",qint64(egressDenied)},{"lastEgressMime",lastEgressMime},{"lastEgressDuringPublish",lastEgressDuringPublish}});
   else if(op=="quit"){alive=false;pending.clear();app.quit();return;}
   else {app.exit(2);return;}
  }
 };
 fcntl(STDIN_FILENO,F_SETFL,fcntl(STDIN_FILENO,F_GETFL)|O_NONBLOCK);
 QSocketNotifier control(STDIN_FILENO,QSocketNotifier::Read);QObject::connect(&control,&QSocketNotifier::activated,&app,controls);
 QSocketNotifier events(xcb_get_file_descriptor(connection),QSocketNotifier::Read);
 auto receivedProperty=[&]{
  const uint32_t limit=receiving.target==targets?512:16*1024*1024;
  auto *property=xcb_get_property_reply(connection,xcb_get_property(connection,1,receiving.window,atom("OWNED_PROXY_RECEIVE"),XCB_GET_PROPERTY_TYPE_ANY,0,(limit+3)/4),nullptr);
  if(!property){receiving.failed=true;receiving.done=true;return;}
  const auto size=xcb_get_property_value_length(property);
  if(property->bytes_after||size<0||uint32_t(size)>limit){receiving.failed=true;receiving.done=true;}
  else if(property->type==incr&&!receiving.incremental&&property->format==32&&size==4){const uint32_t length=*static_cast<const uint32_t *>(xcb_get_property_value(property));if(length>limit){receiving.failed=true;receiving.done=true;}else receiving.incremental=true;}
  else if(property->type!=(receiving.target==targets?xcb_atom_t(XCB_ATOM_ATOM):receiving.target)||property->format!=(receiving.target==targets?32:8)){receiving.failed=true;receiving.done=true;}
  else if(receiving.data.size()+size>limit){receiving.failed=true;receiving.done=true;}
  else{receiving.format=property->format;receiving.data.append(static_cast<const char *>(xcb_get_property_value(property)),size);if(!receiving.incremental||size==0)receiving.done=true;}
  free(property);xcb_flush(connection);
 };
 pump=[&]{while(auto *raw=xcb_poll_for_event(connection)){
  const auto type=raw->response_type&0x7f;
  if(type==extension->first_event+XCB_XFIXES_SELECTION_NOTIFY){auto *event=reinterpret_cast<xcb_xfixes_selection_notify_event_t *>(raw);controls();if(event->owner==owners[ownerIndex])selectedAt=event->selection_timestamp;
   if(receiving.active&&event->owner!=receiving.owner){receiving.failed=true;receiving.done=true;}
   if(event->owner&&event->owner!=owners[0]&&event->owner!=owners[1]){if(client(event->owner)==engine){nativeOwner=event->owner;nativeStamp=event->selection_timestamp;if(writeUntil>=clock.elapsed()||valid()){writeUntil=-1;const auto owner=nativeOwner,stamp=nativeStamp;QTimer::singleShot(0,&app,[&,owner,stamp,lifetime]{if(*lifetime)publish(owner,stamp);});}}else{nativeOwner=0;egressDenied++;announce();}}
  }
  else if(type==XCB_SELECTION_NOTIFY&&receiving.active&&!receiving.done){const auto *event=reinterpret_cast<xcb_selection_notify_event_t *>(raw);if(event->requestor==receiving.window&&event->selection==clipboard&&event->target==receiving.target){if(event->property==XCB_ATOM_NONE){receiving.failed=true;receiving.done=true;}else receivedProperty();}}
  else if(type==XCB_SELECTION_REQUEST){const auto request=*reinterpret_cast<xcb_selection_request_event_t *>(raw);controls();
   if(raw->response_type&0x80){synthetic++;denied++;notify(request,XCB_ATOM_NONE);free(raw);continue;}
   if(request.selection!=clipboard||(request.owner!=owners[0]&&request.owner!=owners[1])||client(request.requestor)!=engine){foreign++;denied++;notify(request,XCB_ATOM_NONE);free(raw);continue;}
   const auto epoch=lease;const auto property=request.property==XCB_ATOM_NONE?request.target:request.property;
   if(!valid()){denied++;notify(request,XCB_ATOM_NONE);free(raw);continue;}
   if(request.target==timestamp){xcb_change_property(connection,XCB_PROP_MODE_REPLACE,request.requestor,property,XCB_ATOM_INTEGER,32,1,&selectedAt);notify(request,property);free(raw);continue;}
   if(request.target!=targets&&request.target!=utf8&&request.target!=text&&request.target!=plain&&request.target!=png&&request.target!=html){denied++;notify(request,XCB_ATOM_NONE);free(raw);continue;}
   const auto *mime=system->mimeData();metadataReads++;if(!mime||!valid()||epoch!=lease){denied++;notify(request,XCB_ATOM_NONE);free(raw);continue;}
   const auto formats=mime->formats();qsizetype metadataBytes=0;for(const auto &format:formats)metadataBytes+=format.toUtf8().size();if(formats.size()>128||metadataBytes>65536){capDenied++;denied++;notify(request,XCB_ATOM_NONE);free(raw);continue;}
   if(request.target==targets){QVector<xcb_atom_t> available{targets,timestamp};if(mime->hasText())available<<utf8<<text<<plain;if(mime->hasFormat("image/png")||mime->hasImage())available<<png;if(mime->hasHtml())available<<html;if(available.size()>128){denied++;notify(request,XCB_ATOM_NONE);}else{xcb_change_property(connection,XCB_PROP_MODE_REPLACE,request.requestor,property,XCB_ATOM_ATOM,32,uint32_t(available.size()),available.data());notify(request,property);}free(raw);continue;}
   QByteArray payload;if(request.target==utf8||request.target==text||request.target==plain)payload=mime->text().toUtf8();else if(request.target==png)payload=mime->data("image/png");else payload=mime->html().toUtf8();dataReads++;fetchedBytes+=quint64(payload.size());maxFetchedBytes=qMax(maxFetchedBytes,quint64(payload.size()));
   if(!valid()||epoch!=lease||payload.size()>16*1024*1024){if(payload.size()>16*1024*1024)capDenied++;denied++;notify(request,XCB_ATOM_NONE);free(raw);continue;}
   transferred+=quint64(payload.size());const quint64 key=(quint64(request.requestor)<<32)|property;
   qsizetype retained=payload.size();for(const auto &entry:pending)retained+=entry.bytes.size();
   if(payload.size()>65536){if(pending.size()>=8||retained>16*1024*1024||pending.contains(key)){capDenied++;denied++;notify(request,XCB_ATOM_NONE);}else{const uint32_t mask=XCB_EVENT_MASK_PROPERTY_CHANGE|XCB_EVENT_MASK_STRUCTURE_NOTIFY;xcb_change_window_attributes(connection,request.requestor,XCB_CW_EVENT_MASK,&mask);const uint32_t length=uint32_t(payload.size());xcb_change_property(connection,XCB_PROP_MODE_REPLACE,request.requestor,property,incr,32,1,&length);pending.insert(key,{request.target,std::move(payload),0,clock.elapsed()+5000,epoch});notify(request,property);}}
   else{xcb_change_property(connection,XCB_PROP_MODE_REPLACE,request.requestor,property,request.target,8,uint32_t(payload.size()),payload.constData());notify(request,property);}
  }
  else if(type==XCB_PROPERTY_NOTIFY){auto *event=reinterpret_cast<xcb_property_notify_event_t *>(raw);if(receiving.active&&receiving.incremental&&event->window==receiving.window&&event->atom==atom("OWNED_PROXY_RECEIVE")&&event->state==XCB_PROPERTY_NEW_VALUE)receivedProperty();const quint64 key=(quint64(event->window)<<32)|event->atom;auto entry=pending.find(key);if(event->state==XCB_PROPERTY_DELETE&&entry!=pending.end()){
   if(!valid()||entry->lease!=lease||entry->deadline<clock.elapsed()){pending.erase(entry);}
   else if(!paused){const auto size=qMin<qsizetype>(65536,entry->bytes.size()-entry->offset);xcb_change_property(connection,XCB_PROP_MODE_REPLACE,event->window,event->atom,entry->type,8,uint32_t(size),entry->bytes.constData()+entry->offset);entry->offset+=size;if(size==0)pending.erase(entry);xcb_flush(connection);}
  }}
  else if(type==XCB_DESTROY_NOTIFY){auto *event=reinterpret_cast<xcb_destroy_notify_event_t *>(raw);for(auto entry=pending.begin();entry!=pending.end();)if(xcb_window_t(entry.key()>>32)==event->window)entry=pending.erase(entry);else++entry;}
  free(raw);
 }if(xcb_connection_has_error(connection)){alive=false;pending.clear();app.exit(2);}};
 auto fetch=[&](xcb_atom_t target,xcb_window_t owner,xcb_timestamp_t stamp)->QByteArray{
  if(receiving.active||!alive||nativeOwner!=owner||nativeStamp!=stamp||client(owner)!=engine)return {};
  auto *current=xcb_get_selection_owner_reply(connection,xcb_get_selection_owner(connection,clipboard),nullptr);const bool matched=current&&current->owner==owner;free(current);if(!matched)return {};
  receiving=Receiving{};receiving.active=true;receiving.owner=owner;receiving.target=target;receiving.window=xcb_generate_id(connection);const uint32_t mask=XCB_EVENT_MASK_PROPERTY_CHANGE;xcb_create_window(connection,XCB_COPY_FROM_PARENT,receiving.window,root,0,0,1,1,0,XCB_WINDOW_CLASS_INPUT_OUTPUT,XCB_COPY_FROM_PARENT,XCB_CW_EVENT_MASK,&mask);xcb_convert_selection(connection,receiving.window,clipboard,target,atom("OWNED_PROXY_RECEIVE"),XCB_CURRENT_TIME);xcb_flush(connection);const auto deadline=clock.elapsed()+5000;
  while(alive&&!receiving.done&&clock.elapsed()<deadline){pump();QCoreApplication::processEvents(QEventLoop::ExcludeUserInputEvents,2);if(!receiving.done)QThread::msleep(1);}
  auto *after=xcb_get_selection_owner_reply(connection,xcb_get_selection_owner(connection,clipboard),nullptr);const bool stillOwner=after&&after->owner==owner;free(after);pump();const bool ok=stillOwner&&alive&&receiving.done&&!receiving.failed&&nativeOwner==owner&&nativeStamp==stamp;auto result=ok?receiving.data:QByteArray();xcb_destroy_window(connection,receiving.window);xcb_flush(connection);receiving=Receiving{};if(!ok)egressDenied++;return result;
 };
 publish=[&](xcb_window_t owner,xcb_timestamp_t stamp){
  if(!alive||nativeOwner!=owner||nativeStamp!=stamp)return;
  if(receiving.active){QTimer::singleShot(10,&app,[&,owner,stamp,lifetime]{if(*lifetime)publish(owner,stamp);});return;}
  const auto available=fetch(targets,owner,stamp);if(available.isEmpty()||available.size()%4||available.size()>512){egressDenied++;return;}
  QVector<xcb_atom_t> supported;for(qsizetype i=0;i<available.size();i+=4){xcb_atom_t value;memcpy(&value,available.constData()+i,4);supported.append(value);}
  QHash<QString,xcb_atom_t> mapping;if(supported.contains(utf8))mapping.insert("text/plain",utf8);else if(supported.contains(text))mapping.insert("text/plain",text);else if(supported.contains(plain))mapping.insert("text/plain",plain);if(supported.contains(html))mapping.insert("text/html",html);if(supported.contains(png)){mapping.insert("image/png",png);mapping.insert("application/x-qt-image",png);}if(mapping.isEmpty())return;
  const auto formats=mapping.keys();auto retrieve=[&,owner,stamp,mapping,lifetime](const QString &mime,QMetaType type)->QVariant{
   if(!*lifetime||!mapping.contains(mime)||nativeOwner!=owner||nativeStamp!=stamp)return {};
   lastEgressMime=mime;lastEgressDuringPublish=publishing;const auto body=fetch(mapping.value(mime),owner,stamp);egressReads++;egressBytes+=quint64(body.size());
   if(mime=="application/x-qt-image"){if(body.size()<24||body.left(8)!=QByteArray::fromHex("89504e470d0a1a0a"))return {};const auto number=[&](int at){return (quint32(quint8(body[at]))<<24)|(quint32(quint8(body[at+1]))<<16)|(quint32(quint8(body[at+2]))<<8)|quint32(quint8(body[at+3]));};const auto width=number(16),height=number(20);if(!width||!height||quint64(width)*height>4*1024*1024)return {};return QImage::fromData(body,"PNG");}
   if(type==QMetaType::fromType<QString>()){return QString::fromUtf8(body);}
   return body;
  };
  publishing=true;system->setMimeData(new OfferedMime(formats,retrieve));publishing=false;publications++;
 };
 QObject::connect(&events,&QSocketNotifier::activated,&app,pump);
 QTimer timer;timer.setInterval(25);QObject::connect(&timer,&QTimer::timeout,&app,[&]{pump();for(auto entry=pending.begin();entry!=pending.end();)if(entry->deadline<clock.elapsed()||!valid()||entry->lease!=lease)entry=pending.erase(entry);else++entry;});timer.start();
 reply({{"ready",true},{"enginePid",qint64(engine)}});QTimer::singleShot(40000,&app,&QCoreApplication::quit);const auto result=app.exec();*lifetime=false;alive=false;pending.clear();xcb_disconnect(connection);return result;
}
