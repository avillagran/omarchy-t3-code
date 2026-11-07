#include "browserinputitem.h"
#include "browserinputchannel.h"
#include <QElapsedTimer>
#include <QThread>
#include <QInputMethodQueryEvent>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QFile>
#include <cstdio>
#include <limits>
#include <functional>
#include <fcntl.h>
#include <poll.h>
#include <unistd.h>
#include <sys/stat.h>
#include <sys/prctl.h>
#include <signal.h>
#include <cstdlib>
#include <sys/syscall.h>
#include <sys/ioctl.h>
#include <sys/wait.h>
#include <QDir>

static int checks = 0;
#define CHECK(value) do { if (!(value)) { fprintf(stderr, "check failed line %d\n", __LINE__); return 1; } ++checks; } while(false)
static bool waitFor(const std::function<bool()> &condition) {
  QElapsedTimer timer;
  timer.start();
  while (!condition() && timer.elapsed() < 3000) {
    QCoreApplication::processEvents();
    QThread::msleep(2);
  }
  return condition();
}

int main(int argc, char **argv) {
  QByteArray liveBytes;
  if (argc == 3 && QByteArray(argv[1]) == "--context-fd") {
    bool valid = false;
    const int fd = QByteArray(argv[2]).toInt(&valid);
    struct stat status {};
    const auto parent = getppid();
    CHECK(valid && fd >= 64 && fstat(fd, &status) == 0 &&
          S_ISFIFO(status.st_mode) && status.st_uid == geteuid());
    CHECK(fcntl(fd, F_SETFD, FD_CLOEXEC) == 0);
    CHECK(prctl(PR_SET_PDEATHSIG, SIGKILL) == 0 && getppid() == parent);
    auto exact = [&](char *data, int length) {
      QElapsedTimer timer; timer.start();
      int offset = 0;
      while (offset < length && timer.elapsed() < 3000) {
        pollfd ready {fd, POLLIN, 0};
        if (poll(&ready, 1, 50) <= 0) continue;
        const auto count = read(fd, data+offset, size_t(length-offset));
        if (count <= 0) return false;
        offset += int(count);
      }
      return offset == length;
    };
    unsigned char header[4];
    CHECK(exact(reinterpret_cast<char *>(header),4));
    const quint32 size = (quint32(header[0])<<24) | (quint32(header[1])<<16) |
                         (quint32(header[2])<<8) | header[3];
    CHECK(size > 0 && size <= 16384);
    liveBytes.resize(int(size));
    CHECK(exact(liveBytes.data(),int(size)));
    close(fd);
  }
  QGuiApplication app(argc, argv);
  QQuickWindow window;
  window.setTitle("Owned native context query");
  window.resize(320, 240);
  BrowserInputItem item;
  item.setParentItem(window.contentItem());
  item.setSize(QSizeF(320, 240));
  item.setScopeToken("owned-scope");
  item.setInputEnabled(true);
  window.show();
  item.forceActiveFocus();
  window.requestActivate();
  CHECK(waitFor([&] { return window.isActive() && item.hasActiveFocus(); }));
  BrowserInputContext::Binding lease{"owned-scope", "physical-connection-1",
      "physical-surface-3", "owned-tab", "owned-frame"};
  BrowserInputContext::Geometry geometry{QRectF(0,29,1280,995), QRectF(10,20,640,497.5)};
  QFile receipt("/fixture/native-state.json");
  if (liveBytes.isEmpty()) CHECK(receipt.open(QIODevice::ReadOnly));
  const auto native = QJsonDocument::fromJson(liveBytes.isEmpty()?receipt.readAll():liveBytes).object();
  const auto actual = native["state"].toObject();
  const auto actualRect = actual["rectangle"].toArray();
  const auto outer = native["bounds"].toObject();
  const auto content = native["contentBounds"].toObject();
  geometry.content = QRectF(content["x"].toDouble()-outer["x"].toDouble(),
      content["y"].toDouble()-outer["y"].toDouble(),
      content["width"].toDouble(),content["height"].toDouble());
  if (!liveBytes.isEmpty()) {
    const qreal scale = qMin(item.width()/geometry.content.width(),
                            item.height()/geometry.content.height());
    geometry.presentation = QRectF((item.width()-geometry.content.width()*scale)/2,
        (item.height()-geometry.content.height()*scale)/2,
        geometry.content.width()*scale,geometry.content.height()*scale);
  }
  BrowserInputContext::State state;
  state.serial = quint32(actual["serial"].toInt());
  CHECK(actualRect.size()==4);
  state.caret = QRectF(actualRect[0].toDouble(),actualRect[1].toDouble(),
                      actualRect[2].toDouble(),actualRect[3].toDouble());
  state.surrounding = liveBytes.isEmpty() ? QByteArray("A\xf0\x9f\x98\x80\xe6\x97\xa5\xe6\x9c\xacZQ")
      : QByteArray::fromBase64(actual["surroundingUtf8"].toString().toLatin1());
  state.cursorByte = actual["cursorByte"].toInt();
  state.anchorByte = actual["anchorByte"].toInt();
  state.hints = quint32(actual["contentHints"].toInt());
  state.purpose = quint32(actual["contentPurpose"].toInt());
  CHECK(!item.contextAvailable());
  CHECK(!item.inputMethodQuery(Qt::ImSurroundingText).isValid());
  CHECK(!item.admitNativeContext(lease, geometry, state));
  item.bindNativeContext(lease);
  CHECK(item.admitNativeContext(lease, geometry, state));
  QInputMethodQueryEvent query(Qt::ImSurroundingText | Qt::ImCursorRectangle |
                             Qt::ImCursorPosition | Qt::ImAnchorPosition);
  QCoreApplication::sendEvent(&item, &query);
  CHECK(query.value(Qt::ImSurroundingText).toString().size() == 7);
  CHECK(query.value(Qt::ImCursorPosition).toInt() == 7);
  CHECK(query.value(Qt::ImAnchorPosition).toInt() == 7);
  const auto expectedRectangle = liveBytes.isEmpty() ? QRectF(55,37,0,9) :
      QRectF(geometry.presentation.x()+(state.caret->x()-geometry.content.x())*geometry.presentation.width()/geometry.content.width(),
             geometry.presentation.y()+(state.caret->y()-geometry.content.y())*geometry.presentation.height()/geometry.content.height(),
             state.caret->width()*geometry.presentation.width()/geometry.content.width(),
             state.caret->height()*geometry.presentation.height()/geometry.content.height());
  CHECK(query.value(Qt::ImCursorRectangle).toRectF() == expectedRectangle);
  auto older = state;
  older.serial--;
  older.caret = QRectF(1,2,0,18);
  CHECK(!item.admitNativeContext(lease, geometry, older));
  CHECK(item.inputMethodQuery(Qt::ImCursorRectangle).toRectF() == expectedRectangle);
  state.anchorByte = 5;
  CHECK(item.admitNativeContext(lease, geometry, state));
  CHECK(item.inputMethodQuery(Qt::ImAnchorPosition).toInt() == 3);
  CHECK(item.inputMethodQuery(Qt::ImCurrentSelection).toString() == QString::fromUtf8("日本ZQ"));
  CHECK(BrowserInputItem::nativeMount("owned-scope") == &item);
  {
    BrowserInputItem duplicate;
    duplicate.setScopeToken("owned-scope");
    CHECK(BrowserInputItem::nativeMount("owned-scope") == nullptr);
  }
  CHECK(BrowserInputItem::nativeMount("owned-scope") == &item);
  item.setNativeBinding({{"connection",lease.connection},{"surface",lease.surface},
                         {"tab",lease.tab},{"frame",lease.frame}});
  item.setNativeContentRect(geometry.content);
  item.setFramePresentation(geometry.presentation);
  CHECK(item.admitBoundNativeContext(lease,state));
  CHECK(item.inputMethodQuery(Qt::ImCursorRectangle).toRectF() == expectedRectangle);
  const auto metadata = item.nativeBinding();
  auto changed = metadata;
  changed["frame"] = "replacement-frame";
  item.setNativeBinding(changed);
  CHECK(!item.contextAvailable());
  CHECK(!item.admitBoundNativeContext(lease,state));
  item.setNativeBinding(metadata);
  CHECK(item.admitBoundNativeContext(lease,state));
  changed = metadata;
  changed["unexpected"] = "not-retained";
  item.setNativeBinding(changed);
  CHECK(item.nativeBinding().isEmpty());
  CHECK(!item.contextAvailable());
  CHECK(!item.admitBoundNativeContext(lease,state));
  item.setNativeBinding(metadata);
  CHECK(item.admitBoundNativeContext(lease,state));
  auto foreign = lease;
  for (auto member : {&BrowserInputContext::Binding::scope, &BrowserInputContext::Binding::connection,
                     &BrowserInputContext::Binding::surface, &BrowserInputContext::Binding::tab,
                     &BrowserInputContext::Binding::frame}) {
    foreign = lease;
    foreign.*member = "foreign";
    CHECK(!item.admitNativeContext(foreign, geometry, state));
    CHECK(!item.retireNativeContext(foreign));
    CHECK(item.contextAvailable());
  }
  state.cursorByte = 2;
  CHECK(!item.admitNativeContext(lease, geometry, state));
  CHECK(!item.contextAvailable());
  state.cursorByte = 13;
  state.surrounding = QByteArray();
  state.cursorByte = state.anchorByte = 0;
  CHECK(item.admitNativeContext(lease, geometry, state));
  CHECK(item.inputMethodQuery(Qt::ImSurroundingText).isValid());
  CHECK(item.inputMethodQuery(Qt::ImSurroundingText).toString().isEmpty());
  state.surrounding.reset();
  CHECK(item.admitNativeContext(lease, geometry, state));
  CHECK(!item.inputMethodQuery(Qt::ImSurroundingText).isValid());
  CHECK(item.inputMethodQuery(Qt::ImCursorRectangle).isValid());
  state.hints = 64 | 128;
  state.purpose = 9;
  CHECK(item.admitNativeContext(lease, geometry, state));
  const auto hints = item.inputMethodQuery(Qt::ImHints).toInt();
  CHECK(hints & Qt::ImhHiddenText);
  CHECK(hints & Qt::ImhSensitiveData);
  CHECK(hints & Qt::ImhDigitsOnly);
  auto invalid = geometry;
  invalid.content.setWidth(0);
  CHECK(!item.admitNativeContext(lease, invalid, state));
  CHECK(!item.contextAvailable());
  state.caret = QRectF(-5,-10,0,18);
  CHECK(item.admitNativeContext(lease, geometry, state));
  CHECK(qAbs(item.inputMethodQuery(Qt::ImCursorRectangle).toRectF().x()-
      (geometry.presentation.x()+(-5-geometry.content.x())*geometry.presentation.width()/geometry.content.width())) < .000001);
  state.caret = QRectF(0,0,-1,18);
  CHECK(!item.admitNativeContext(lease, geometry, state));
  state.caret = QRectF(std::numeric_limits<double>::infinity(),0,0,18);
  CHECK(!item.admitNativeContext(lease, geometry, state));
  state.caret = QRectF(90,63,0,18);
  CHECK(item.admitNativeContext(lease, geometry, state));
  item.setScopeToken("replacement-scope");
  CHECK(!item.contextAvailable());
  CHECK(!item.admitNativeContext(lease, geometry, state));
  item.setScopeToken("owned-scope");
  item.bindNativeContext(lease);
  CHECK(item.admitNativeContext(lease, geometry, state));
  item.setInputEnabled(false);
  CHECK(!item.contextAvailable());
  CHECK(!item.inputMethodQuery(Qt::ImCursorRectangle).isValid());
  item.setInputEnabled(true);
  item.bindNativeContext(lease);
  CHECK(item.admitNativeContext(lease, geometry, state));
  item.setVisible(false);
  CHECK(!item.contextAvailable());
  item.setVisible(true);
  CHECK(waitFor([&] { return item.hasActiveFocus(); }));
  item.bindNativeContext(lease);
  CHECK(item.admitNativeContext(lease, geometry, state));
  {
    const QString channelId="owned-channel";
    const auto descriptorBaseline=QDir("/proc/self/fd").entryList(QDir::AllEntries|QDir::NoDotAndDotDot).size();
    auto bound=lease;
    bound.connection=channelId+":native-socket-1";
    item.setNativeBinding({{"connection",bound.connection},{"surface",bound.surface},
                           {"tab",bound.tab},{"frame",bound.frame}});
    QJsonObject binding{{"scope",bound.scope},{"connection",bound.connection},
       {"surface",bound.surface},{"tab",bound.tab},{"frame",bound.frame}};
    QJsonObject value{{"serial",10},{"hints",4},{"purpose",0},
      {"caret",QJsonArray{90,63,0,18}},
      {"surroundingUtf8",QString::fromLatin1(QByteArray("A\xf0\x9f\x98\x80\xe6\x97\xa5\xe6\x9c\xacZQ").toBase64())},
      {"cursorByte",13},{"anchorByte",13}};
    QJsonObject row{{"version",1},{"op","state"},{"binding",binding},{"state",value}};
    int pair[2];
    CHECK(socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0,pair)==0);
    const int pidfd=int(syscall(SYS_pidfd_open,getpid(),0));
    CHECK(pidfd>=0);
    auto channel=BrowserInputChannel::create(pair[0],pidfd,getpid(),channelId);
    CHECK(channel && channel->available());
    char requestBytes[4096];
    const auto requestSize=recv(pair[1],requestBytes,sizeof requestBytes,MSG_DONTWAIT);
    CHECK(requestSize>0);
    const auto request=QJsonDocument::fromJson(QByteArray(requestBytes,int(requestSize))).object();
    CHECK(request.size()==3 && request["op"]=="request" && request["binding"].toObject()==binding);
    CHECK(!QByteArray(requestBytes,int(requestSize)).contains("surrounding"));
    auto deliver=[&](const QJsonObject &message) {
      const auto bytes=QJsonDocument(message).toJson(QJsonDocument::Compact);
      if (send(pair[1],bytes.constData(),size_t(bytes.size()),MSG_NOSIGNAL)!=bytes.size()) return false;
      return waitFor([&] { int pending=-1;return ioctl(pair[0],FIONREAD,&pending)==0 && pending==0; });
    };
    CHECK(deliver(row));
    CHECK(item.contextAvailable());
    CHECK(item.inputMethodQuery(Qt::ImCursorPosition).toInt()==7);
    {
      BrowserInputItem duplicate;
      duplicate.setScopeToken("owned-scope");
      CHECK(!item.contextAvailable());
      CHECK(deliver(row));
      CHECK(!item.contextAvailable());
    }
    CHECK(deliver(row));
    CHECK(item.contextAvailable());
    auto foreignBinding=binding;
    foreignBinding["frame"]="foreign-frame";
    auto stale=row;stale["binding"]=foreignBinding;
    CHECK(deliver(stale));
    CHECK(item.contextAvailable() && channel->available());
    CHECK(deliver({{"version",1},{"op","invalidate"},{"binding",binding}}));
    CHECK(!item.contextAvailable() && channel->available());
    CHECK(deliver(row));
    CHECK(item.contextAvailable());
    ::close(pair[1]);
    CHECK(waitFor([&] { return !channel->available(); }));
    CHECK(!item.contextAvailable());
    channel.reset();

    CHECK(socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0,pair)==0);
    auto hostile=BrowserInputChannel::create(pair[0],int(syscall(SYS_pidfd_open,getpid(),0)),getpid(),channelId);
    CHECK(hostile && hostile->available());
    const auto packet=QJsonDocument(row).toJson(QJsonDocument::Compact);
    const auto sender=fork();
    CHECK(sender>=0);
    if (sender==0) _exit(send(pair[1],packet.constData(),size_t(packet.size()),MSG_NOSIGNAL)==packet.size()?0:1);
    int status=0;
    CHECK(waitpid(sender,&status,0)==sender && WIFEXITED(status) && WEXITSTATUS(status)==0);
    CHECK(waitFor([&] { return !hostile->available(); }));
    CHECK(!item.contextAvailable());
    ::close(pair[1]);
    hostile.reset();

    CHECK(socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0,pair)==0);
    auto unexpected=BrowserInputChannel::create(pair[0],int(syscall(SYS_pidfd_open,getpid(),0)),getpid(),channelId);
    CHECK(unexpected && unexpected->available());
    int ownedPipe[2];CHECK(pipe2(ownedPipe,O_CLOEXEC)==0);
    alignas(cmsghdr) char ancillary[CMSG_SPACE(sizeof(int))]{};
    iovec packetData{const_cast<char *>(packet.constData()),size_t(packet.size())};
    msghdr message{};message.msg_iov=&packetData;message.msg_iovlen=1;
    message.msg_control=ancillary;message.msg_controllen=sizeof ancillary;
    auto *header=CMSG_FIRSTHDR(&message);header->cmsg_level=SOL_SOCKET;
    header->cmsg_type=SCM_RIGHTS;header->cmsg_len=CMSG_LEN(sizeof(int));
    memcpy(CMSG_DATA(header),&ownedPipe[0],sizeof(int));
    CHECK(sendmsg(pair[1],&message,MSG_NOSIGNAL)==packet.size());
    CHECK(waitFor([&] { return !unexpected->available(); }));
    ::close(ownedPipe[0]);::close(ownedPipe[1]);::close(pair[1]);unexpected.reset();

    const auto engine=fork();
    CHECK(engine>=0);
    if (engine==0) {for (;;) pause();}
    CHECK(socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0,pair)==0);
    auto lifetime=BrowserInputChannel::create(pair[0],int(syscall(SYS_pidfd_open,engine,0)),getpid(),channelId);
    CHECK(lifetime && lifetime->available());
    CHECK(deliver(row));
    CHECK(item.contextAvailable());
    CHECK(kill(engine,SIGTERM)==0);
    CHECK(waitpid(engine,&status,0)==engine);
    CHECK(waitFor([&] { return !lifetime->available() && !item.contextAvailable(); }));
    ::close(pair[1]);
    lifetime.reset();
    CHECK(QDir("/proc/self/fd").entryList(QDir::AllEntries|QDir::NoDotAndDotDot).size()==descriptorBaseline);
  }
  window.hide();
  QCoreApplication::processEvents();
  CHECK(!item.contextAvailable());
  CHECK(!item.inputMethodQuery(Qt::ImSurroundingText).isValid());
  printf("BROWSER_NATIVE_CONTEXT_QUERY_PASS checks=%d realQtQueryEvent=true productWaylandRouteSelected=false\n", checks);
  return 0;
}
