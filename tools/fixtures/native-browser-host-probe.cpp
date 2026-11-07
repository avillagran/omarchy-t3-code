#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QFile>
#include <QTimer>
#include <QImage>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <cstdio>
class Images:public QObject {
 Q_OBJECT
public:
 Q_INVOKABLE QVariantMap inspect(const QString &data) {
  const auto image=QImage::fromData(QByteArray::fromBase64(data.toUtf8()));
  return {{"width",image.width()},{"height",image.height()},{"color",image.isNull()?QString():image.pixelColor(image.width()-10,image.height()-10).name()}};
 }
};
int main(int argc,char **argv) {
 if(argc!=4)return 2;
 qInstallMessageHandler([](QtMsgType,const QMessageLogContext&,const QString &s){fprintf(stderr,"QT %s\n",s.toUtf8().constData());});
 QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);QtWebEngineQuick::initialize();QGuiApplication app(argc,argv);
 QFile core(argv[2]),host(argv[3]);if(!core.open(QIODevice::ReadOnly)||!host.open(QIODevice::ReadOnly))return 3;
 QQmlApplicationEngine engine;Images images;
 engine.rootContext()->setContextProperty("bridgeSource",QString::fromUtf8(core.readAll()));engine.rootContext()->setContextProperty("nativeSource",QString::fromUtf8(host.readAll()));engine.rootContext()->setContextProperty("images",&images);
 engine.load(QUrl::fromLocalFile(argv[1]));if(engine.rootObjects().isEmpty())return 4;
 QTimer::singleShot(17000,&app,&QCoreApplication::quit);return app.exec();
}
#include "native-browser-host-probe.moc"
