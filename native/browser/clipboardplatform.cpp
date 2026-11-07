// This member stays on the real GUI display/seat. No clipboard body crosses
// QML.
#include "clipboardplatform.h"
#include "clipboardcohort.h"
#include "waylandoffers.h"
#include <QClipboard>
#include <QFile>
#include <QHash>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSocketNotifier>
#include <QTimer>
#include <QUuid>
#include <QVector>
#include <cerrno>
#include <cstring>
#include <poll.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/un.h>
#include <unistd.h>
struct ClipboardPlatform::State {
  ClipboardPlatform *owner;
  int listener = -1, peer = -1, pidfd = -1;
  pid_t worker = 0;
  QString address, binding, session;
  bool admitted = false;
  QSocketNotifier *acceptor = nullptr, *reader = nullptr, *death = nullptr;
  std::shared_ptr<NativeWaylandOffers> offers;
  QHash<QString, std::shared_ptr<ClipboardCohort>> cohorts;
  QString currentSource;
  bool publishing = false, lastAvailable = false;
  std::shared_ptr<bool> active = std::make_shared<bool>(true);
  QTimer *readyPoll = nullptr;
  void changed() {
    if (!publishing && !QGuiApplication::clipboard()->ownsClipboard())
      currentSource.clear();
    const auto values = cohorts;
    for (const auto &cohort : values)
      cohort->offerChanged(currentSource);
  }
  static qint64 parent(qint64 pid) {
    QFile file(QString("/proc/%1/status").arg(pid));
    if (!file.open(QIODevice::ReadOnly))
      return -1;
    for (const auto &row : file.readAll().split('\n'))
      if (row.startsWith("PPid:"))
        return row.mid(5).trimmed().toLongLong();
    return -1;
  }
  static bool pidfdMatches(int descriptor, qint64 pid) {
    QFile file(QString("/proc/self/fdinfo/%1").arg(descriptor));
    if (!file.open(QIODevice::ReadOnly))
      return false;
    bool matched = false;
    for (const auto &row : file.readAll().split('\n'))
      if (row.startsWith("Pid:"))
        matched = row.mid(4).trimmed().toLongLong() == pid;
    pollfd value{descriptor, POLLIN, 0};
    return matched && poll(&value, 1, 0) == 0;
  }
  bool reply(QJsonObject row) {
    const auto bytes = QJsonDocument(row).toJson(QJsonDocument::Compact);
    return send(peer, bytes.constData(), size_t(bytes.size()),
                MSG_NOSIGNAL | MSG_DONTWAIT) == bytes.size();
  }
  bool registerCohort(const QJsonObject &row, QVector<int> &rights) {
    const auto id = row["cohort"].toString();
    const auto pid = row["enginePid"].toInteger();
    if (row.size() != 5 || row["version"] != 1 ||
        row["binding"].toString() != session || id.isEmpty() ||
        QUuid(id).toString(QUuid::WithoutBraces) != id || pid <= 0 ||
        pid > 2147483647 || rights.size() != 3 || cohorts.contains(id) ||
        cohorts.size() >= 16 || !offers->ready())
      return false;
    const auto renderer = parent(pid);
    if (renderer <= 0 || parent(renderer) != worker ||
        !pidfdMatches(rights[2], pid))
      return false;
    int kind = 0;
    socklen_t kindSize = sizeof kind;
    ucred server{};
    socklen_t serverSize = sizeof server;
    sockaddr_un address{};
    socklen_t addressSize = sizeof address;
    if (getsockopt(rights[0], SOL_SOCKET, SO_TYPE, &kind, &kindSize) ||
        kind != SOCK_STREAM ||
        getsockopt(rights[0], SOL_SOCKET, SO_PEERCRED, &server, &serverSize) ||
        serverSize != sizeof server || server.uid != getuid() ||
        parent(server.pid) != worker ||
        getpeername(rights[0], reinterpret_cast<sockaddr *>(&address),
                    &addressSize) ||
        address.sun_family != AF_UNIX)
      return false;
    const auto token = session;
    auto life = active;
    auto cohort = ClipboardCohort::create(
        id, rights[0], rights[1], rights[2], pid, offers,
        [this, token, life](const QString &source, QMimeData *mime) {
          if (!*life) {
            delete mime;
            return;
          }
          if (session != token || !cohorts.contains(source)) {
            delete mime;
            return;
          }
          publishing = true;
          currentSource = source;
          QGuiApplication::clipboard()->setMimeData(mime);
          publishing = false;
          changed();
        });
    rights.clear();
    if (!cohort)
      return reply({{"type", "rejected"},
                    {"cohort", id},
                    {"code", "clipboard_cohort_unavailable"}});
    cohorts.insert(id, cohort);
    cohort->offerChanged(currentSource);
    return reply({{"type", "registered"}, {"cohort", id}});
  }

  explicit State(ClipboardPlatform *owner) : owner(owner) {}
  void closePeer() {
    cohorts.clear();
    session.clear();
    delete reader;
    reader = nullptr;
    if (peer >= 0)
      ::close(peer);
    peer = -1;
    if (admitted) {
      admitted = false;
      emit owner->workerAdmittedChanged();
    }
  }
  void retire() {
    closePeer();
    delete death;
    death = nullptr;
    if (pidfd >= 0)
      ::close(pidfd);
    pidfd = -1;
    worker = 0;
    binding.clear();
    if (acceptor)
      acceptor->setEnabled(false);
  }
  ~State() {
    *active = false;
    if (offers)
      offers->changed = {};
    if (readyPoll)
      readyPoll->stop();
    retire();
    delete acceptor;
    if (listener >= 0)
      ::close(listener);
  }
  bool workerAlive() const {
    pollfd value{pidfd, POLLIN, 0};
    return pidfd >= 0 && poll(&value, 1, 0) == 0;
  }
  void receive() {
    if (!workerAlive()) {
      retire();
      return;
    }
    char bytes[4096];
    alignas(cmsghdr) char
        ancillary[CMSG_SPACE(sizeof(ucred)) + CMSG_SPACE(sizeof(int) * 4)];
    iovec data{bytes, sizeof bytes};
    msghdr message{};
    message.msg_iov = &data;
    message.msg_iovlen = 1;
    message.msg_control = ancillary;
    message.msg_controllen = sizeof ancillary;
    const auto size = recvmsg(peer, &message, MSG_DONTWAIT | MSG_CMSG_CLOEXEC);
    if (size < 0 && (errno == EAGAIN || errno == EINTR))
      return;
    QVector<int> rights;
    struct CloseRights {
      QVector<int> &rights;
      ~CloseRights() {
        for (int fd : rights)
          ::close(fd);
      }
    } closeRights{rights};
    bool credential = false, unexpected = false;
    for (auto *header = CMSG_FIRSTHDR(&message); header;
         header = CMSG_NXTHDR(&message, header)) {
      if (header->cmsg_level == SOL_SOCKET &&
          header->cmsg_type == SCM_CREDENTIALS &&
          header->cmsg_len == CMSG_LEN(sizeof(ucred))) {
        ucred value;
        memcpy(&value, CMSG_DATA(header), sizeof value);
        credential = value.pid == worker && value.uid == getuid();
      } else {
        if (header->cmsg_level == SOL_SOCKET &&
            header->cmsg_type == SCM_RIGHTS) {
          const auto count = (header->cmsg_len - CMSG_LEN(0)) / sizeof(int);
          const auto *fds = reinterpret_cast<const int *>(CMSG_DATA(header));
          for (size_t i = 0; i < count; i++)
            rights.append(fds[i]);
        } else
          unexpected = true;
      }
    }
    if (size <= 0 || message.msg_flags & (MSG_TRUNC | MSG_CTRUNC) ||
        !credential || unexpected) {
      closePeer();
      return;
    }
    if (admitted) {
      const auto input = QByteArray(bytes, int(size));
      QJsonParseError error;
      const auto document = QJsonDocument::fromJson(input, &error);
      const auto row = document.object();
      if (error.error != QJsonParseError::NoError || !document.isObject() ||
          input != document.toJson(QJsonDocument::Compact) || !workerAlive()) {
        closePeer();
        return;
      }
      if (row["op"] == "register") {
        if (!registerCohort(row, rights))
          closePeer();
        return;
      }
      if (row["op"] == "retire" && row.size() == 4 && rights.isEmpty() &&
          row["version"] == 1 && row["binding"].toString() == session) {
        const auto id = row["cohort"].toString();
        cohorts.remove(id);
        if (!reply({{"type", "retired"}, {"cohort", id}}))
          closePeer();
        return;
      }
    }
    if (!rights.isEmpty()) {
      closePeer();
      return;
    }
    if (admitted && workerAlive() &&
        QByteArray(bytes, int(size)) ==
            QByteArrayLiteral("{\"op\":\"probe\",\"version\":1}")) {
      if (send(peer, "alive", 5, MSG_NOSIGNAL | MSG_DONTWAIT) != 5)
        closePeer();
      return;
    }
    if (QByteArray(bytes, int(size)) !=
            QByteArrayLiteral("{\"op\":\"hello\",\"version\":1}") ||
        admitted || !workerAlive()) {
      closePeer();
      return;
    }
    session = QUuid::createUuid().toString(QUuid::WithoutBraces);
    const auto reply = QJsonDocument(QJsonObject{{"type", "admitted"},
                                                 {"version", 1},
                                                 {"binding", session}})
                           .toJson(QJsonDocument::Compact);
    if (send(peer, reply.constData(), size_t(reply.size()),
             MSG_NOSIGNAL | MSG_DONTWAIT) != reply.size()) {
      closePeer();
      return;
    }
    admitted = true;
    emit owner->workerAdmittedChanged();
  }
  void accept() {
    const auto candidate =
        accept4(listener, nullptr, nullptr, SOCK_NONBLOCK | SOCK_CLOEXEC);
    if (candidate < 0)
      return;
    ucred credentials{};
    socklen_t size = sizeof credentials;
    int pass = 1;
    if (worker <= 0 || !workerAlive() || peer >= 0 ||
        getsockopt(candidate, SOL_SOCKET, SO_PEERCRED, &credentials, &size) ||
        size != sizeof credentials || credentials.pid != worker ||
        credentials.uid != getuid() ||
        setsockopt(candidate, SOL_SOCKET, SO_PASSCRED, &pass, sizeof pass)) {
      ::close(candidate);
      return;
    }
    peer = candidate;
    reader = new QSocketNotifier(peer, QSocketNotifier::Read, owner);
    QObject::connect(reader, &QSocketNotifier::activated, owner,
                     [this] { receive(); });
    receive();
  }
};
ClipboardPlatform::ClipboardPlatform(QObject *parent)
    : QObject(parent), state(std::make_unique<State>(this)) {
  // Merely constructing the member registers metadata asynchronously. The
  // normal Qt event loop reads the socket; the member dispatches only its own
  // proxies.
  if (!qGuiApp || !QGuiApplication::platformName().startsWith("wayland"))
    return;
  state->offers = std::make_shared<NativeWaylandOffers>();
  state->listener =
      socket(AF_UNIX, SOCK_SEQPACKET | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
  if (state->listener < 0)
    return;
  int pass = 1;
  if (setsockopt(state->listener, SOL_SOCKET, SO_PASSCRED, &pass,
                 sizeof pass)) {
    ::close(state->listener);
    state->listener = -1;
    return;
  }
  const auto name =
      ("o3-clipboard-" + QUuid::createUuid().toString(QUuid::WithoutBraces))
          .toLatin1();
  sockaddr_un address{};
  address.sun_family = AF_UNIX;
  memcpy(address.sun_path + 1, name.constData(), size_t(name.size()));
  if (::bind(state->listener, reinterpret_cast<sockaddr *>(&address),
             socklen_t(offsetof(sockaddr_un, sun_path) + 1 + name.size())) ||
      listen(state->listener, 4)) {
    ::close(state->listener);
    state->listener = -1;
    return;
  }
  auto life = state->active;
  state->offers->changed = [this, life] {
    if (*life)
      state->changed();
  };
  connect(QGuiApplication::clipboard(), &QClipboard::dataChanged, this,
          [this, life] {
            if (*life)
              state->changed();
          });
  state->readyPoll = new QTimer(this);
  state->readyPoll->setInterval(5);
  connect(state->readyPoll, &QTimer::timeout, this, [this] {
    const auto ready = available();
    if (ready != state->lastAvailable) {
      state->lastAvailable = ready;
      if (!ready)
        state->retire();
      emit availableChanged();
    }
    for (auto entry = state->cohorts.begin(); entry != state->cohorts.end();)
      if (!entry.value()->alive())
        entry = state->cohorts.erase(entry);
      else
        ++entry;
  });
  state->readyPoll->start();
  state->address = QString::fromLatin1(name);
  state->acceptor =
      new QSocketNotifier(state->listener, QSocketNotifier::Read, this);
  state->acceptor->setEnabled(false);
  connect(state->acceptor, &QSocketNotifier::activated, this,
          [this] { state->accept(); });
}
ClipboardPlatform::~ClipboardPlatform() = default;
QString ClipboardPlatform::endpoint() const { return state->address; }
bool ClipboardPlatform::available() const {
  return state->listener >= 0 && state->offers && state->offers->ready();
}
bool ClipboardPlatform::workerAdmitted() const { return state->admitted; }
QString ClipboardPlatform::bindWorker(qint64 pid) {
  if (!available() || pid <= 0 || pid > 2147483647)
    return {};
  QFile status(QString("/proc/%1/status").arg(pid));
  if (!status.open(QIODevice::ReadOnly))
    return {};
  const auto rows = status.readAll().split('\n');
  bool child = false;
  for (const auto &row : rows)
    if (row.startsWith("PPid:"))
      child = row.mid(5).trimmed().toLongLong() == getpid();
  if (!child)
    return {};
  const auto descriptor = int(syscall(SYS_pidfd_open, pid, 0));
  if (descriptor < 0)
    return {};
  state->retire();
  state->worker = pid_t(pid);
  state->pidfd = descriptor;
  state->binding = QUuid::createUuid().toString(QUuid::WithoutBraces);
  state->death = new QSocketNotifier(descriptor, QSocketNotifier::Read, this);
  const auto binding = state->binding;
  connect(state->death, &QSocketNotifier::activated, this,
          [this, binding] { retireWorker(binding); });
  state->acceptor->setEnabled(true);
  return binding;
}
void ClipboardPlatform::retireWorker(const QString &binding) {
  if (!binding.isEmpty() && binding == state->binding)
    state->retire();
}
