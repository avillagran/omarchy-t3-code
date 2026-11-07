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
#include <cstdio>

class PrivatePipe : public QObject {
    Q_OBJECT
    int descriptor;
    QByteArray input, output;
    QSocketNotifier reader, writer;
    static constexpr qsizetype limit = 256 * 1024 * 1024;
    bool closed = false;
    void retire() {
        if (closed) return;
        closed = true; reader.setEnabled(false); writer.setEnabled(false);
        ::close(descriptor); input.clear(); output.clear(); emit ended();
    }
    void flush() {
        while (!output.isEmpty()) {
            const auto count = ::write(descriptor, output.constData(), output.size());
            if (count > 0) output.remove(0, count);
            else if (count < 0 && errno == EINTR) continue;
            else if (count < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) break;
            else { retire(); return; }
        }
        writer.setEnabled(!output.isEmpty() && !closed);
    }
public:
    explicit PrivatePipe(int fd) : descriptor(fd), reader(fd,QSocketNotifier::Read), writer(fd,QSocketNotifier::Write) {
        const int flags = ::fcntl(fd,F_GETFL);
        if (flags < 0 || ::fcntl(fd,F_SETFL,flags|O_NONBLOCK) < 0) { closed=true; return; }
        writer.setEnabled(false);
        connect(&writer,&QSocketNotifier::activated,this,[this]{flush();});
        connect(&reader,&QSocketNotifier::activated,this,[this]{
            char chunk[65536];
            for (;;) {
                const auto count = ::read(descriptor,chunk,sizeof(chunk));
                if (count > 0) {
                    if (input.size()+count > limit) { retire(); return; }
                    input.append(chunk,count);
                    for (;;) {
                        const auto end=input.indexOf('\n'); if(end<0)break;
                        const auto line=input.left(end);input.remove(0,end+1);
                        emit command(QString::fromUtf8(line));
                    }
                } else if(count<0&&errno==EINTR)continue;
                else if(count<0&&(errno==EAGAIN||errno==EWOULDBLOCK))break;
                else {retire();return;}
            }
        });
    }
    ~PrivatePipe() override { if(!closed)::close(descriptor); }
    Q_INVOKABLE void send(const QString &line) {
        if(closed)return;
        const auto bytes=line.toUtf8();
        if(bytes.size()>limit-output.size()){retire();return;}
        output.append(bytes);flush();
    }
    Q_INVOKABLE void loaded(const QString &url, const QString &title) {
        if(url.startsWith("http://127.0.0.1:") && url.endsWith("/owned") && title=="Private owned Qt navigation") {
            std::puts("OWNED_PAGE_LOADED");std::fflush(stdout);
        }
    }
signals:
    void command(const QString &line);
    void ended();
};
int main(int argc,char **argv) {
    if(argc!=8)return 2;
    std::signal(SIGPIPE,SIG_IGN);
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QtWebEngineQuick::initialize();QGuiApplication app(argc,argv);
    bool valid=false;const int fd=QString::fromLocal8Bit(argv[2]).toInt(&valid);if(!valid||fd<3)return 3;
    PrivatePipe pipe(fd);QQmlApplicationEngine engine;
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
