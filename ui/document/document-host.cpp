// Qt frontend bootstrap only. All connections, file access and document data
// remain in Rust. Separate from Quickshell: WebEngine requires intact argv.
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QJsonDocument>
#include <QQuickWindow>
#include <QTimer>
#include <QFile>
#include <QTemporaryFile>
#include <QUrl>
#include <QDir>
#include <QJsonObject>
#include <QSocketNotifier>
#include <QWheelEvent>
#include <QMouseEvent>
#include <QKeyEvent>
#include <QClipboard>
#include <fcntl.h>
#include <unistd.h>
#include <atomic>
class DocumentHost: public QObject {
 Q_OBJECT
 QQuickWindow *window=nullptr;
 QTimer capture;
 QString directory;
 int sequence=0;
 bool allowCopy=false;
 std::atomic<bool> grabbing{false};
 QByteArray input;
 QTemporaryFile reviewPage;
public:
 DocumentHost(QString directory):directory(directory){
  reviewPage.setFileTemplate(QDir(directory).filePath("review-XXXXXX.html"));
  connect(QGuiApplication::clipboard(),&QClipboard::dataChanged,this,[this](){if(!allowCopy)return;allowCopy=false;auto text=QGuiApplication::clipboard()->text();if(text.size()>120000)return;auto out=QJsonDocument(QJsonObject{{"clipboard",text}}).toJson(QJsonDocument::Compact);fprintf(stdout,"%s\n",out.constData());fflush(stdout);});
  // No capture FPS cap: service each repaint on the next event-loop turn.
  // Coalesce pending requests; captures themselves never schedule idle work.
  capture.setSingleShot(true);capture.setInterval(0);
  connect(&capture,&QTimer::timeout,this,[this](){
   if(!window)return;grabbing=true;auto image=window->grabWindow();grabbing=false;if(image.isNull())return;
   QString name=QString("frame-%1.png").arg(++sequence%2),path=this->directory+"/"+name,tmp=path+".next";
   // PNG remains lossless; favor fast local transport over smallest files.
   if(!image.save(tmp,"PNG",80))return;QFile::setPermissions(tmp,QFileDevice::ReadOwner|QFileDevice::WriteOwner);if(::rename(QFile::encodeName(tmp).constData(),QFile::encodeName(path).constData())!=0)return;
   auto out=QJsonDocument(QJsonObject{{"frame",name},{"revision",sequence},{"width",image.width()},{"height",image.height()}}).toJson(QJsonDocument::Compact);
   fprintf(stdout,"%s\n",out.constData());fflush(stdout);
  });
 }
 void attach(QQuickWindow *w){window=w;
  // Chromium repaint requests (including canvas and CSS animations) drive
  // captures. A capture's own render must not schedule another idle frame.
  connect(w,&QQuickWindow::afterRendering,this,[this](){if(!grabbing.load())QMetaObject::invokeMethod(this,[this](){requestFrame();},Qt::QueuedConnection);},Qt::DirectConnection);
  requestFrame();
 }
 Q_INVOKABLE QUrl reviewPageUrl(const QString &html){
  if(html.size()>32*1024*1024)return QUrl();
  if(!reviewPage.isOpen()&&!reviewPage.open())return QUrl();
  reviewPage.resize(0);auto bytes=html.toUtf8();if(reviewPage.write(bytes)!=bytes.size()||!reviewPage.flush())return QUrl();
  return QUrl::fromLocalFile(reviewPage.fileName());
 }
 Q_INVOKABLE void requestFrame(){if(!capture.isActive())capture.start();}
 Q_INVOKABLE void reviewState(const QString &error){auto out=QJsonDocument(QJsonObject{{"reviewReady",true},{"reviewError",error.left(256)}}).toJson(QJsonDocument::Compact);fprintf(stdout,"%s\n",out.constData());fflush(stdout);}
 // A JSON-RPC request from an MCP app page, for the host to answer.
 Q_INVOKABLE void appRequest(const QString &json){
  if(json.size()>256*1024)return;auto doc=QJsonDocument::fromJson(json.toUtf8());if(!doc.isObject())return;
  auto out=QJsonDocument(QJsonObject{{"appRequest",doc.object()}}).toJson(QJsonDocument::Compact);fprintf(stdout,"%s\n",out.constData());fflush(stdout);
 }
 Q_INVOKABLE void contentHeight(int height){
  if(height<1||height>100000)return;auto out=QJsonDocument(QJsonObject{{"contentHeight",height}}).toJson(QJsonDocument::Compact);fprintf(stdout,"%s\n",out.constData());fflush(stdout);
 }
 void command(const QJsonObject &v){
  if(!window)return;QString type=v["type"].toString();
  if(type=="close"){QCoreApplication::quit();return;}
  if(type=="review-options"){QString layout=v["layout"].toString();if((layout=="split"||layout=="stacked")&&v["wrap"].isBool())QMetaObject::invokeMethod(window,"setReviewOptions",Q_ARG(QVariant,layout),Q_ARG(QVariant,v["wrap"].toBool()));}
  else if(type=="app-response"){auto r=v["response"];if(r.isObject()){auto json=QString::fromUtf8(QJsonDocument(r.toObject()).toJson(QJsonDocument::Compact));QMetaObject::invokeMethod(window,"appResponse",Q_ARG(QVariant,json));}}
  else if(type=="resize"){window->resize(qBound(240,v["width"].toInt(),2560),qBound(180,v["height"].toInt(),1800));}
  else if(type=="zoom"){QMetaObject::invokeMethod(window,"setZoom",Q_ARG(QVariant,qBound(0.5,v["value"].toDouble(),2.0)));}
  else if(type=="wheel"){
   QPointF p(v["x"].toDouble(),v["y"].toDouble());QWheelEvent e(p,p,QPoint(v["px"].toInt(),v["py"].toInt()),QPoint(v["ax"].toInt(),v["ay"].toInt()),Qt::NoButton,Qt::KeyboardModifiers(v["modifiers"].toInt()),Qt::NoScrollPhase,false);QCoreApplication::sendEvent(window,&e);
  }else if(type=="press"||type=="release"||type=="move"||type=="double"){
   QPointF p(v["x"].toDouble(),v["y"].toDouble());QEvent::Type kind=type=="press"?QEvent::MouseButtonPress:type=="release"?QEvent::MouseButtonRelease:type=="double"?QEvent::MouseButtonDblClick:QEvent::MouseMove;
   QMouseEvent e(kind,p,p,p,Qt::MouseButton(v["button"].toInt()),Qt::MouseButtons(v["buttons"].toInt()),Qt::KeyboardModifiers(v["modifiers"].toInt()));QCoreApplication::sendEvent(window,&e);
  }else if(type=="key"||type=="keyup"){
   if(type=="key"&&v["key"].toInt()==Qt::Key_C&&(v["modifiers"].toInt()&(Qt::ControlModifier|Qt::MetaModifier))){allowCopy=true;QMetaObject::invokeMethod(window,"copySelection");QTimer::singleShot(1000,this,[this](){allowCopy=false;});return;}
   auto modifiers=Qt::KeyboardModifiers(v["modifiers"].toInt());if(v["key"].toInt()==Qt::Key_A&&modifiers.testFlag(Qt::MetaModifier)){modifiers.setFlag(Qt::MetaModifier,false);modifiers.setFlag(Qt::ControlModifier,true);}
   QKeyEvent e(type=="key"?QEvent::KeyPress:QEvent::KeyRelease,v["key"].toInt(),modifiers,v["text"].toString());QCoreApplication::sendEvent(window,&e);
  }
  requestFrame();if(type!="move"){QTimer::singleShot(110,this,[this](){requestFrame();});QTimer::singleShot(250,this,[this](){requestFrame();});}
 }
 void readInput(){
  char data[16384];ssize_t n;while((n=::read(STDIN_FILENO,data,sizeof data))>0){input.append(data,n);if(input.size()>65536){QCoreApplication::quit();return;}}
  if(n==0){QCoreApplication::quit();return;}
  int end;while((end=input.indexOf('\n'))>=0){auto row=input.left(end);input.remove(0,end+1);auto doc=QJsonDocument::fromJson(row);if(doc.isObject())command(doc.object());}
 }
};
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
extern "C" int t3_document_main(const char *payload, const char *qml) {
  int argc=1; char name[]="omarchy-t3-document"; char *argv[]={name,nullptr};
  QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
  QtWebEngineQuick::initialize();
  QGuiApplication app(argc,argv);
  qInstallMessageHandler([](QtMsgType,const QMessageLogContext&,const QString&message){fprintf(stderr,"%s\n",qPrintable(message));});
  app.setApplicationName("omarchy-t3-document");
  app.setApplicationDisplayName("T3 Code · Omarchy");
  auto data=QJsonDocument::fromJson(payload).object();
  std::unique_ptr<DocumentHost> host;
  if(data["offscreen"].toBool())host=std::make_unique<DocumentHost>(data["frameDirectory"].toString());
  QQmlApplicationEngine engine;
  if(host)engine.rootContext()->setContextProperty("documentHost",host.get());
  QObject::connect(&engine,&QQmlEngine::warnings,[](const QList<QQmlError> &errors){for(const auto &error:errors)fprintf(stderr,"%s\n",qPrintable(error.toString()));});
  engine.rootContext()->setContextProperty("documentData",QJsonDocument::fromJson(payload).toVariant());
  engine.load(QUrl::fromLocalFile(qml));
  if(engine.rootObjects().isEmpty()){fprintf(stderr,"Qt document QML failed to load: %s\n",qml);return 1;}
  QObject::connect(&engine,&QQmlEngine::quit,&app,&QCoreApplication::quit);
  QObject::connect(&engine,&QQmlEngine::exit,&app,&QCoreApplication::exit);
  std::unique_ptr<QSocketNotifier> reader;
  if(host){
   host->attach(qobject_cast<QQuickWindow*>(engine.rootObjects().first()));
   fcntl(STDIN_FILENO,F_SETFL,fcntl(STDIN_FILENO,F_GETFL)|O_NONBLOCK);
   reader=std::make_unique<QSocketNotifier>(STDIN_FILENO,QSocketNotifier::Read);
   QObject::connect(reader.get(),&QSocketNotifier::activated,&app,[&host](){host->readInput();});
  }
  fprintf(stdout,"T3_DOCUMENT_READY\n");fflush(stdout);
  if(qEnvironmentVariableIsSet("T3_DOCUMENT_VERIFY"))QTimer::singleShot(1600,[&engine](){QMetaObject::invokeMethod(engine.rootObjects().first(),"verifyDocument");});
  return app.exec();
}

#include "document-host.moc"
