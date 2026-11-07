// Standalone isolated Qt fixture; no application profile, IPC or credentials.
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <QTimer>
#include <QFile>
#include <cstdio>
int main(int argc, char **argv) {
    qInstallMessageHandler([](QtMsgType, const QMessageLogContext &, const QString &s) {
        fprintf(stderr,"QT %s\n",s.toUtf8().constData());
    });
    if (argc != 3) return 2;
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QtWebEngineQuick::initialize();
    QGuiApplication app(argc,argv);
    QFile bridge(argv[2]);
    if (!bridge.open(QIODevice::ReadOnly)) return 3;
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("privateBridgeSource",QString::fromUtf8(bridge.readAll()));
    engine.load(QUrl::fromLocalFile(argv[1]));
    if (engine.rootObjects().isEmpty()) return 4;
    QTimer::singleShot(10000,&app,&QCoreApplication::quit);
    return app.exec();
}
