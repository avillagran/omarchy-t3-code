#pragma once
// Dedicated GUI-owner channel. Context text never uses the registration socket,
// QML properties, generic worker stdout, or a durable mutation queue.
#include "browserinputitem.h"
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSocketNotifier>
#include <QHash>
#include <sys/socket.h>
#include <poll.h>
#include <unistd.h>
#include <fcntl.h>
#include <cstring>
#include <limits>
#include <memory>
#include <QThread>

class BrowserInputChannel final : public QObject {
  int socket = -1, engine = -1;
  pid_t worker = 0;
  QString identity;
  bool retiring=false;
  QSocketNotifier *reader = nullptr, *death = nullptr;
  QMetaObject::Connection requester;
  struct Mount {BrowserInputContext::Binding binding;QPointer<BrowserInputItem> item;};
  QHash<QString,Mount> admitted;
  static bool integer(const QJsonValue &value, qint64 low, qint64 high, qint64 &out) {
    if (!value.isDouble()) return false;
    const auto number = value.toDouble();
    if (!qIsFinite(number) || number < double(low) || number > double(high) ||
        number != double(qint64(number))) return false;
    out = qint64(number);
    return true;
  }
  static QString opaque(const QJsonValue &value) {
    const auto text = value.toString();
    return value.isString() && !text.isEmpty() && text.size() <= 256 ? text : QString();
  }
  bool alive() const {
    pollfd descriptor{engine,POLLIN,0};
    return engine >= 0 && poll(&descriptor,1,0) == 0;
  }
  void request(BrowserInputItem *item) {
    if (!available() || !item || !item->nativeContextInteractive() ||
        BrowserInputItem::nativeMount(item->scopeToken())!=item) return;
    const auto metadata=item->nativeBinding();
    if (metadata.size()!=4 || !metadata.value("connection").toString().startsWith(identity+":")) return;
    auto binding=QJsonObject::fromVariantMap(metadata);
    binding.insert("scope",item->scopeToken());
    const auto bytes=QJsonDocument(QJsonObject{{"binding",binding},{"op","request"},{"version",1}}).toJson(QJsonDocument::Compact);
    if (send(socket,bytes.constData(),size_t(bytes.size()),MSG_NOSIGNAL|MSG_DONTWAIT)!=bytes.size()) retire();
  }
  void watch(QObject *focus) {
    disconnect(requester);
    if (!available()) return;
    auto *item=qobject_cast<BrowserInputItem *>(focus);
    if (!item) return;
    QPointer<BrowserInputItem> guarded=item;
    requester=connect(item,&BrowserInputItem::contextRequested,this,
        [this,guarded](const QString &) {if(guarded)request(guarded);});
    request(item);
  }
  bool handle(const QByteArray &bytes) {
    QJsonParseError error;
    const auto document = QJsonDocument::fromJson(bytes,&error);
    if (error.error != QJsonParseError::NoError || !document.isObject() ||
        bytes != document.toJson(QJsonDocument::Compact)) return false;
    const auto row = document.object();
    if (row["version"] != 1 || !row["binding"].isObject()) return false;
    const auto source = row["binding"].toObject();
    if (source.size() != 5) return false;
    BrowserInputContext::Binding binding{opaque(source["scope"]),opaque(source["connection"]),
      opaque(source["surface"]),opaque(source["tab"]),opaque(source["frame"])};
    if (!binding.valid() || !binding.connection.startsWith(identity+":")) return false;
    auto *mount = BrowserInputItem::nativeMount(binding.scope);
    if (row["op"] == "invalidate" && row.size() == 3) {
      if (mount) mount->retireNativeContext(binding);
      if (admitted.contains(binding.scope) && admitted.value(binding.scope).binding == binding)
        admitted.remove(binding.scope);
      return true;
    }
    if (row["op"] != "state" || row.size() != 4 || !row["state"].isObject()) return false;
    const auto input = row["state"].toObject();
    const bool surrounding = input.contains("surroundingUtf8");
    const bool caret = input.contains("caret");
    if (input.size() != 3 + (surrounding?3:0) + (caret?1:0)) return false;
    qint64 serial,hints,purpose;
    if (!integer(input["serial"],0,4294967295LL,serial) ||
        !integer(input["hints"],0,1023,hints) || !integer(input["purpose"],0,13,purpose)) return false;
    BrowserInputContext::State state;
    state.serial=quint32(serial);state.hints=quint32(hints);state.purpose=quint32(purpose);
    if (caret) {
      const auto coordinates = input["caret"].toArray();
      if (!input["caret"].isArray() || coordinates.size()!=4) return false;
      qint64 values[4];
      for (int index=0;index<4;index++)
        if (!integer(coordinates[index],index<2?-2147483648LL:0,2147483647,values[index])) return false;
      state.caret=QRectF(qreal(values[0]),qreal(values[1]),qreal(values[2]),qreal(values[3]));
    }
    if (surrounding) {
      const auto encoded = input["surroundingUtf8"].toString();
      if (!input["surroundingUtf8"].isString() || encoded.size()>5336 ||
          QString::fromLatin1(encoded.toLatin1())!=encoded) return false;
      const auto decoded = QByteArray::fromBase64Encoding(encoded.toLatin1(),QByteArray::AbortOnBase64DecodingErrors);
      qint64 cursor,anchor;
      if (!decoded || decoded.decoded.size()>4000 ||
          !integer(input["cursorByte"],0,4000,cursor) || !integer(input["anchorByte"],0,4000,anchor)) return false;
      state.surrounding=decoded.decoded;state.cursorByte=int(cursor);state.anchorByte=int(anchor);
    }
    if (!admitted.contains(binding.scope) && admitted.size()>=16) return false;
    if (mount && mount->admitBoundNativeContext(binding,state)) {
      admitted.insert(binding.scope,Mount{binding,mount});
    }
    return true; // A retired GUI mount is never resurrected by state traffic.
  }
  void receive() {
    if (!alive()) { retire(); return; }
    char bytes[16384];
    alignas(cmsghdr) char control[CMSG_SPACE(sizeof(ucred))+CMSG_SPACE(16*sizeof(int))];
    iovec data{bytes,sizeof bytes};
    msghdr message{};
    message.msg_iov=&data;message.msg_iovlen=1;
    message.msg_control=control;message.msg_controllen=sizeof control;
    const auto size=recvmsg(socket,&message,MSG_DONTWAIT|MSG_CMSG_CLOEXEC);
    if (size<0 && (errno==EAGAIN || errno==EINTR)) return;
    bool credential=false,unexpected=false;
    for (auto *header=CMSG_FIRSTHDR(&message);header;header=CMSG_NXTHDR(&message,header)) {
      if (header->cmsg_level==SOL_SOCKET && header->cmsg_type==SCM_CREDENTIALS &&
          header->cmsg_len==CMSG_LEN(sizeof(ucred))) {
        ucred value{};memcpy(&value,CMSG_DATA(header),sizeof value);
        credential=value.pid==worker && value.uid==getuid();
      } else {
        unexpected=true;
        if (header->cmsg_level==SOL_SOCKET && header->cmsg_type==SCM_RIGHTS) {
          const auto count=(header->cmsg_len-CMSG_LEN(0))/sizeof(int);
          const auto *rights=reinterpret_cast<const int *>(CMSG_DATA(header));
          for (size_t index=0;index<count;index++) ::close(rights[index]);
        }
      }
    }
    if (size<=0 || message.msg_flags&(MSG_TRUNC|MSG_CTRUNC) || !credential ||
        unexpected || !handle(QByteArray(bytes,int(size)))) retire();
  }
  BrowserInputChannel(int descriptor,int pidfd,pid_t expected,QString id)
      :socket(descriptor),engine(pidfd),worker(expected),identity(std::move(id)) {
    reader=new QSocketNotifier(socket,QSocketNotifier::Read,this);
    death=new QSocketNotifier(engine,QSocketNotifier::Read,this);
    connect(reader,&QSocketNotifier::activated,this,[this]{receive();});
    connect(death,&QSocketNotifier::activated,this,[this]{retire();});
    connect(qGuiApp,&QGuiApplication::focusObjectChanged,this,[this](QObject *focus){watch(focus);});
    watch(qGuiApp->focusObject());
  }
public:
  // The registration owner validates engine ancestry/pidfd identity before
  // transferring these owned FDs; this member independently authenticates every
  // worker message. Failure always consumes and closes both supplied handles.
  static std::shared_ptr<BrowserInputChannel> create(int descriptor,int pidfd,
      pid_t expected,const QString &identity) {
    auto fail=[&]() { if(descriptor>=0)::close(descriptor);if(pidfd>=0&&pidfd!=descriptor)::close(pidfd);return std::shared_ptr<BrowserInputChannel>(); };
    int kind=0; socklen_t kindSize=sizeof kind;
    ucred peer{};socklen_t peerSize=sizeof peer;
    int enabled=1;
    pollfd life{pidfd,POLLIN,0};
    if (!qGuiApp || QThread::currentThread()!=qGuiApp->thread() ||
        expected<=0 || identity.isEmpty() || identity.size()>128 ||
        descriptor<0 || pidfd<0 || descriptor==pidfd ||
        getsockopt(descriptor,SOL_SOCKET,SO_TYPE,&kind,&kindSize) || kind!=SOCK_SEQPACKET ||
        getsockopt(descriptor,SOL_SOCKET,SO_PEERCRED,&peer,&peerSize) || peerSize!=sizeof peer ||
        peer.pid!=expected || peer.uid!=getuid() || poll(&life,1,0)!=0 ||
        fcntl(descriptor,F_SETFD,FD_CLOEXEC) || fcntl(pidfd,F_SETFD,FD_CLOEXEC) ||
        setsockopt(descriptor,SOL_SOCKET,SO_PASSCRED,&enabled,sizeof enabled)) return fail();
    const int receiveBudget=32768;
    if (setsockopt(descriptor,SOL_SOCKET,SO_RCVBUF,&receiveBudget,sizeof receiveBudget)) return fail();
    auto result=std::shared_ptr<BrowserInputChannel>(new BrowserInputChannel(descriptor,pidfd,expected,identity));
    return result->available()?result:std::shared_ptr<BrowserInputChannel>();
  }
  bool available() const { return !retiring && socket>=0 && alive(); }
  void retire() {
    if (retiring) return;
    retiring=true;
    disconnect(requester);
    if (reader) reader->setEnabled(false);
    if (death) death->setEnabled(false);
    for (const auto &mount : admitted) {
      if (mount.item) mount.item->retireNativeContext(mount.binding);
    }
    admitted.clear();
    if (socket>=0) ::close(socket);
    if (engine>=0) ::close(engine);
    socket=engine=-1;
  }
  ~BrowserInputChannel() override { retire(); }
};
