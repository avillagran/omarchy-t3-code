#pragma once
// Dedicated private browser descriptor. No raw CDP enters public status IPC.
#include <QObject>
#include <QByteArray>
#include <QString>
#include <QSocketNotifier>
#include <fcntl.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <cerrno>
class PrivateBrowserPipe : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool open READ isOpen NOTIFY ended)
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
            const auto count = ::send(descriptor, output.constData(), output.size(), MSG_NOSIGNAL);
            if (count > 0) output.remove(0, count);
            else if (count < 0 && errno == EINTR) continue;
            else if (count < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) break;
            else { retire(); return; }
        }
        writer.setEnabled(!output.isEmpty() && !closed);
    }
public:
    static bool admit(int fd) {
        if(fd<64)return false;
        int type=0; socklen_t typeSize=sizeof(type);
        ucred peer{};socklen_t peerSize=sizeof(peer);
        sockaddr_un address{};socklen_t addressSize=sizeof(address);
        const int fdFlags=::fcntl(fd,F_GETFD);
        if(fdFlags<0 || ::getsockopt(fd,SOL_SOCKET,SO_TYPE,&type,&typeSize)!=0 || type!=SOCK_STREAM ||
           ::getsockopt(fd,SOL_SOCKET,SO_PEERCRED,&peer,&peerSize)!=0 || peer.pid!=::getppid() || peer.uid!=::geteuid() ||
           ::getsockname(fd,reinterpret_cast<sockaddr*>(&address),&addressSize)!=0 || address.sun_family!=AF_UNIX ||
           ::fcntl(fd,F_SETFD,fdFlags|FD_CLOEXEC)!=0)return false;
        return true;
    }
    bool isOpen() const { return !closed; }
    explicit PrivateBrowserPipe(int fd) : descriptor(fd), reader(fd,QSocketNotifier::Read), writer(fd,QSocketNotifier::Write) {
        const int flags = ::fcntl(fd,F_GETFL);
        if (flags < 0 || ::fcntl(fd,F_SETFL,flags|O_NONBLOCK) < 0) { ::close(descriptor);closed=true; return; }
        writer.setEnabled(false);
        connect(&writer,&QSocketNotifier::activated,this,[this]{flush();});
        connect(&reader,&QSocketNotifier::activated,this,[this]{
            char chunk[65536];
            for (int reads=0; reads<16; ++reads) {
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
    ~PrivateBrowserPipe() override { if(!closed)::close(descriptor); }
    Q_INVOKABLE void close() { retire(); }
    Q_INVOKABLE void send(const QString &line) {
        if(closed)return;
        const auto bytes=line.toUtf8();
        if(bytes.size()>limit-output.size()){retire();return;}
        output.append(bytes);flush();
    }
signals:
    void command(const QString &line);
    void ended();
};
