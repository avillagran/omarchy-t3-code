// Qt frontend bootstrap only. All connections, file access and document data
// remain in Rust. Separate from Quickshell: WebEngine requires intact argv.
#include "document-host.h"
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
