#include "private-browser-pipe.h"
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QFile>
#include <QSocketNotifier>
#include <QTimer>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <fcntl.h>
#include <unistd.h>
#include <cerrno>
#include <csignal>
#include <sys/socket.h>
#include <sys/un.h>
#include <cstdio>

class PrivatePipe : public PrivateBrowserPipe {
    Q_OBJECT
public:
    using PrivateBrowserPipe::PrivateBrowserPipe;
    Q_INVOKABLE void loaded(const QString &url, const QString &title) {
        if(url.startsWith("http://127.0.0.1:") && url.endsWith("/owned") && title=="Private owned Qt navigation") {
            std::puts("OWNED_PAGE_LOADED");std::fflush(stdout);
        }
    }
};
int main(int argc,char **argv) {
    if(argc!=8)return 2;
    std::signal(SIGPIPE,SIG_IGN);
    bool valid=false;const int fd=QString::fromLocal8Bit(argv[2]).toInt(&valid);if(!valid||fd<64)return 3;
    if(!PrivateBrowserPipe::admit(fd))return 3;
    // Retain this endpoint only in the owned guest, never in Chromium subprocesses.
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QtWebEngineQuick::initialize();QGuiApplication app(argc,argv);
    PrivatePipe pipe(fd);if(!pipe.isOpen())return 3;QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("privatePipe",&pipe);
    engine.rootContext()->setContextProperty("threadKey",QString::fromUtf8(argv[3]));
    engine.rootContext()->setContextProperty("tabKey",QString::fromUtf8(argv[4]));
    QString source;
    for(int i=5;i<8;i++){QFile file(argv[i]);if(!file.open(QIODevice::ReadOnly))return 4;source+=QString::fromUtf8(file.readAll())+'\n';}
    engine.rootContext()->setContextProperty("privateSource",source);
    QObject::connect(&pipe,&PrivatePipe::ended,&app,&QCoreApplication::quit);
    engine.load(QUrl::fromLocalFile(argv[1]));if(engine.rootObjects().isEmpty())return 5;
    QTimer::singleShot(25000,&app,[]{QCoreApplication::exit(124);});
    return app.exec();
}
#include "owned-browser-pipe.moc"
