#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <QTimer>
#include <cstdio>
int main(int argc,char **argv){qInstallMessageHandler([](QtMsgType,const QMessageLogContext &,const QString &s){fprintf(stderr,"QT %s\n",s.toUtf8().constData());});fprintf(stderr,"PROBE_INIT\n");QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);QtWebEngineQuick::initialize();QGuiApplication app(argc,argv);fprintf(stderr,"PROBE_APP\n");QQmlApplicationEngine engine;engine.load(QUrl::fromLocalFile(argv[1]));fprintf(stderr,"PROBE_LOADED %lld\n",(long long)engine.rootObjects().size());QTimer::singleShot(10000,&app,&QCoreApplication::quit);return app.exec();}
