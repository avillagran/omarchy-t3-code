// Real asynchronous Qt frame loading with a retained image and native IME item.
#include <QGuiApplication>
#include <QElapsedTimer>
#include <QInputMethodEvent>
#include <QImage>
#include <QJSValue>
#include <QJsonArray>
#include <QJsonObject>
#include <QMutex>
#include <QMutexLocker>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickImageProvider>
#include <QQuickItem>
#include <QQuickWindow>
#include <QThread>
#include <QWaitCondition>
#include <cstdio>
#include <memory>
#include <functional>
#define CHECK(value) do {if(!(value)){fprintf(stderr,"RETAINED_IME_FAIL line=%d\n",__LINE__);return 2;}}while(false)
static bool waitFor(const std::function<bool()> &test) {
  QElapsedTimer timer;timer.start();
  while(!test() && timer.elapsed()<3000){QCoreApplication::processEvents();QThread::msleep(2);}
  return test();
}
class HeldImage final : public QQuickImageProvider {
  QMutex mutex;
  QWaitCondition available;
  int started=0,released=0;
public:
  HeldImage():QQuickImageProvider(Image,ForceAsynchronousImageLoading){}
  QImage requestImage(const QString &,QSize *size,const QSize &)override {
    QMutexLocker lock(&mutex);
    const int request=++started;
    QElapsedTimer bound;bound.start();
    while(released<request && bound.elapsed()<5000)available.wait(&mutex,100);
    QImage image(16,16,QImage::Format_ARGB32_Premultiplied);image.fill(Qt::white);
    *size=image.size();return image;
  }
  int count(){QMutexLocker lock(&mutex);return started;}
  void release(){QMutexLocker lock(&mutex);released=started;available.wakeAll();}
};
int main(int argc,char **argv) {
  QGuiApplication app(argc,argv);
  QQmlEngine engine;engine.addImportPath("/fixture/qml");
  auto *provider=new HeldImage;engine.addImageProvider("held",provider);
  QImage initial(16,16,QImage::Format_ARGB32_Premultiplied);initial.fill(Qt::white);
  CHECK(initial.save("/tmp/owned-retained-frame.png"));
  QQuickWindow window;window.resize(320,240);window.setTitle("Owned retained-frame IME");
  QQmlComponent component(&engine);
  component.setData(R"(import QtQuick
import "page" as Product
Product.T3BrowserPage {
 width:320;height:240;interactive:true
 page:({requestId:"retained",nativeInput:true,url:"file:///tmp/owned-retained-frame.png",width:16,height:16,
   nativeInputBinding:{connection:"retained-connection",surface:"retained-surface",tab:"retained-tab",frame:"retained-frame"},
   nativeInputContent:{x:0,y:0,width:16,height:16}})
 property var records:[]
 function holdFrame(){page=Object.assign({},page,{url:"image://held/frame",revision:(page.revision||0)+1})}
 onInput:function(token,event){records=records.concat([{token:token,event:event}])}
})",QUrl("file:///fixture/retained.qml"));
  CHECK(waitFor([&]{return component.status()!=QQmlComponent::Loading;}));
  std::unique_ptr<QQuickItem> page(qobject_cast<QQuickItem *>(component.create()));
  CHECK(page);page->setParentItem(window.contentItem());window.show();window.requestActivate();
  CHECK(waitFor([&]{return page->property("ready").toBool() && window.isActive() &&
    qvariant_cast<QQuickItem *>(page->property("nativeInputItem"));}));
  auto *native=qvariant_cast<QQuickItem *>(page->property("nativeInputItem"));
  native->forceActiveFocus();CHECK(waitFor([&]{return native->hasActiveFocus();}));
  CHECK(native->property("nativeBinding").toMap().size()==4);
  auto records=[&]{return QJsonArray::fromVariantList(page->property("records").value<QJSValue>().toVariant().toList());};
  auto hold=[&]{const auto before=provider->count();return QMetaObject::invokeMethod(page.get(),"holdFrame") &&
    waitFor([&]{return provider->count()>before && !page->property("ready").toBool();});};
  auto release=[&]{provider->release();return waitFor([&]{return page->property("ready").toBool();});};
  QInputMethodEvent first(QString::fromUtf8("候補"),{});QCoreApplication::sendEvent(native,&first);
  CHECK(first.isAccepted() && records().size()==1);
  CHECK(hold());
  const bool enabledDuringLoad=native->property("inputEnabled").toBool();
  const auto rectangle=native->property("framePresentation").toRectF();
  const bool metadataUnavailable=native->property("nativeBinding").toMap().isEmpty() &&
      native->property("nativeContentRect").toRectF().isNull();
  QInputMethodEvent cancel;QCoreApplication::sendEvent(native,&cancel);
  const bool cancelled=cancel.isAccepted() && records().size()==2 && records().last().toObject()["event"].toObject()["preedit"]=="";
  CHECK(release());
  fprintf(stderr,"RETAINED_FRAME_OBSERVATION inputEnabled=%d retainedWidth=%g retainedHeight=%g cancelDelivered=%d\n",
      int(enabledDuringLoad),rectangle.width(),rectangle.height(),int(cancelled));
  CHECK(enabledDuringLoad && rectangle.width()>0 && rectangle.height()>0 && cancelled && metadataUnavailable);
  CHECK(native->property("nativeBinding").toMap().size()==4);
  QInputMethodEvent second(QString::fromUtf8("次"),{});QCoreApplication::sendEvent(native,&second);
  CHECK(second.isAccepted() && records().size()==3);
  CHECK(hold());
  QInputMethodEvent commit;commit.setCommitString(QString::fromUtf8("確定"));QCoreApplication::sendEvent(native,&commit);
  const bool committed=commit.isAccepted() && records().size()==4 && records().last().toObject()["event"].toObject()["text"]==QString::fromUtf8("確定");
  CHECK(release());CHECK(committed);
  CHECK(hold());
  QInputMethodEvent during(QString::fromUtf8("再"),{});QCoreApplication::sendEvent(native,&during);
  QInputMethodEvent finish;QCoreApplication::sendEvent(native,&finish);
  const bool composed=during.isAccepted() && finish.isAccepted() && records().size()==6;
  CHECK(release());CHECK(composed);
  for(const auto &value:records())CHECK(value.toObject()["token"]=="retained");
  CHECK(!native->property("contextAvailable").toBool());
  puts("BROWSER_RETAINED_FRAME_IME_PASS real-provider preedit-commit-cancel same-scope retained-pixels unknown-context");
  puts("BROWSER_QT_INPUT_METHOD_PASS retained-frame-producer");
  return 0;
}
