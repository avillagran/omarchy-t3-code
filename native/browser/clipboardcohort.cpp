// The native engine arbitrates clipboard permissions; this member transports
// only its real selection requests and retains the one current GUI offer.
#include "clipboardcohort.h"
#include <QClipboard>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QFile>
#include <QGuiApplication>
#include <QHash>
#include <QImage>
#include <QQueue>
#include <QSocketNotifier>
#include <QThread>
#include <QTimer>
#include <cstring>
#include <fcntl.h>
#include <optional>
#include <poll.h>
#include <sys/stat.h>
#include <unistd.h>
#include <xcb/res.h>
#include <xcb/xcb.h>
#include <xcb/xfixes.h>
class CurrentClipboardMime final : public QMimeData {
  QHash<QString, xcb_atom_t> mapping;
  QHash<xcb_atom_t, QByteArray> bodies;

public:
  CurrentClipboardMime(QHash<QString, xcb_atom_t> mapping,
                       QHash<xcb_atom_t, QByteArray> bodies)
      : mapping(std::move(mapping)), bodies(std::move(bodies)) {}
  QStringList formats() const override { return mapping.keys(); }

protected:
  QVariant retrieveData(const QString &mime, QMetaType type) const override {
    if (!mapping.contains(mime))
      return {};
    const auto body = bodies.value(mapping.value(mime));
    if (mime == "application/x-qt-image") {
      if (body.size() < 24 ||
          body.left(8) != QByteArray::fromHex("89504e470d0a1a0a"))
        return {};
      const auto number = [&](int at) {
        return (quint32(quint8(body[at])) << 24) |
               (quint32(quint8(body[at + 1])) << 16) |
               (quint32(quint8(body[at + 2])) << 8) |
               quint32(quint8(body[at + 3]));
      };
      const auto width = number(16), height = number(20);
      if (!width || !height || quint64(width) * height > 4 * 1024 * 1024)
        return {};
      return QImage::fromData(body, "PNG");
    }
    if (type == QMetaType::fromType<QString>())
      return QString::fromUtf8(body);
    return body;
  }
};
struct ClipboardCohort::State : QObject, std::enable_shared_from_this<State> {
  static constexpr qsizetype limit = 16 * 1024 * 1024;
  struct Transfer {
    xcb_atom_t type;
    QByteArray bytes;
    qsizetype offset = 0;
    qint64 deadline;
    quint64 lease;
  };
  struct Receiving {
    xcb_window_t window = 0, owner = 0;
    xcb_atom_t target = 0;
    QByteArray bytes;
    bool active = false, done = false, failed = false, incremental = false;
    uint32_t limit = 0;
  };
  QString id;
  int enginefd;
  quint32 engine;
  xcb_connection_t *connection = nullptr;
  xcb_window_t root = 0, owners[2]{};
  int ownerIndex = 0, eventBase = 0;
  xcb_atom_t clipboard = 0, targets = 0, timestamp = 0, incr = 0, utf8 = 0,
             text = 0, plain = 0, png = 0, html = 0, property = 0;
  xcb_window_t nativeOwner = 0;
  xcb_timestamp_t nativeStamp = 0, selectedAt = 0;
  quint64 lease = 1;
  bool alive = false, publishing = false;
  QElapsedTimer clock;
  QHash<quint64, Transfer> pending;
  Receiving receiving;
  bool reading = false, queuedScheduled = false;
  QQueue<xcb_selection_request_event_t> queued;
  std::shared_ptr<NativeWaylandOffers> offers;
  std::shared_ptr<const void> lastOffer;
  Publish publish;
  QSocketNotifier *events = nullptr, *death = nullptr;
  QTimer *timer = nullptr;
  State(QString id, int enginefd, quint32 engine,
        std::shared_ptr<NativeWaylandOffers> offers, Publish publish)
      : id(std::move(id)), enginefd(enginefd), engine(engine),
        offers(std::move(offers)), publish(std::move(publish)) {
    clock.start();
  }
  ~State() override {
    retire();
    if (connection)
      xcb_disconnect(connection);
    if (enginefd >= 0)
      ::close(enginefd);
  }
  bool valid() const {
    pollfd value{enginefd, POLLIN, 0};
    return alive && poll(&value, 1, 0) == 0;
  }
  void retire() {
    if (alive && connection) {
      for (auto entry = pending.cbegin(); entry != pending.cend(); ++entry)
        xcb_change_property(
            connection, XCB_PROP_MODE_REPLACE, xcb_window_t(entry.key() >> 32),
            xcb_atom_t(entry.key()), entry->type, 8, 0, nullptr);
      xcb_flush(connection);
    }
    alive = false;
    lease++;
    if (events)
      events->setEnabled(false);
    if (death)
      death->setEnabled(false);
    if (timer)
      timer->stop();
    for (const auto &request : queued)
      notify(request, XCB_ATOM_NONE);
    queued.clear();
    pending.clear();
    if (receiving.active) {
      receiving.failed = true;
      receiving.done = true;
    }
  }
  xcb_atom_t atom(const char *name) {
    auto *reply = xcb_intern_atom_reply(
        connection,
        xcb_intern_atom(connection, 0, uint16_t(strlen(name)), name), nullptr);
    if (!reply)
      return 0;
    const auto value = reply->atom;
    free(reply);
    return value;
  }
  quint32 client(xcb_window_t requestor) {
    xcb_res_client_id_spec_t spec{requestor,
                                  XCB_RES_CLIENT_ID_MASK_LOCAL_CLIENT_PID};
    auto *result = xcb_res_query_client_ids_reply(
        connection, xcb_res_query_client_ids(connection, 1, &spec), nullptr);
    quint32 pid = 0;
    if (result) {
      auto values = xcb_res_query_client_ids_ids_iterator(result);
      for (; values.rem; xcb_res_client_id_value_next(&values))
        if ((values.data->spec.mask &
             XCB_RES_CLIENT_ID_MASK_LOCAL_CLIENT_PID) &&
            xcb_res_client_id_value_value_length(values.data) == 1)
          pid = *xcb_res_client_id_value_value(values.data);
      free(result);
    }
    return pid;
  }
  bool initialize(int socket, int authority) {
    struct stat status{};
    const auto seals = fcntl(authority, F_GET_SEALS);
    const bool sealed =
        seals >= 0 &&
        (seals & (F_SEAL_WRITE | F_SEAL_GROW | F_SEAL_SHRINK | F_SEAL_SEAL)) ==
            (F_SEAL_WRITE | F_SEAL_GROW | F_SEAL_SHRINK | F_SEAL_SEAL);
    const bool regular = fstat(authority, &status) == 0 &&
                         S_ISREG(status.st_mode) && status.st_uid == getuid() &&
                         status.st_size == 44;
    if (!regular || !sealed) {
      ::close(authority);
      ::close(socket);
      return false;
    }
    QByteArray record(44, '\0');
    const auto size = pread(authority, record.data(), size_t(record.size()), 0);
    ::close(authority);
    if (!regular || !sealed || size != 44 ||
        record.left(8) != QByteArray::fromHex("ffff000000000012") ||
        record.mid(8, 18) != "MIT-MAGIC-COOKIE-1" ||
        record.mid(26, 2) != QByteArray::fromHex("0010")) {
      record.fill(0);
      ::close(socket);
      return false;
    }
    xcb_auth_info_t auth{18, record.data() + 8, 16, record.data() + 28};
    connection = xcb_connect_to_fd(socket, &auth);
    record.fill(0);
    if (!connection || xcb_connection_has_error(connection))
      return false;
    const auto flags = fcntl(xcb_get_file_descriptor(connection), F_GETFD);
    if (flags < 0 || !(flags & FD_CLOEXEC))
      return false;
    const auto *fixes = xcb_get_extension_data(connection, &xcb_xfixes_id);
    const auto *resource = xcb_get_extension_data(connection, &xcb_res_id);
    if (!fixes || !fixes->present || !resource || !resource->present)
      return false;
    eventBase = fixes->first_event;
    auto *version = xcb_xfixes_query_version_reply(
        connection, xcb_xfixes_query_version(connection, 5, 0), nullptr);
    if (!version)
      return false;
    free(version);
    auto *res = xcb_res_query_version_reply(
        connection, xcb_res_query_version(connection, 1, 2), nullptr);
    if (!res || res->server_major < 1 ||
        (res->server_major == 1 && res->server_minor < 2)) {
      free(res);
      return false;
    }
    free(res);
    const auto screens = xcb_setup_roots_iterator(xcb_get_setup(connection));
    if (!screens.rem)
      return false;
    root = screens.data->root;
    clipboard = atom("CLIPBOARD");
    targets = atom("TARGETS");
    timestamp = atom("TIMESTAMP");
    incr = atom("INCR");
    utf8 = atom("UTF8_STRING");
    text = atom("text/plain;charset=utf-8");
    plain = atom("text/plain");
    png = atom("image/png");
    html = atom("text/html");
    property = atom("O3_CLIPBOARD_RECEIVE");
    if (!clipboard || !targets || !timestamp || !incr || !utf8 || !text ||
        !plain || !png || !html || !property)
      return false;
    for (auto &owner : owners) {
      owner = xcb_generate_id(connection);
      xcb_create_window(connection, XCB_COPY_FROM_PARENT, owner, root, 0, 0, 1,
                        1, 0, XCB_WINDOW_CLASS_INPUT_OUTPUT,
                        XCB_COPY_FROM_PARENT, 0, nullptr);
    }
    xcb_xfixes_select_selection_input(
        connection, owners[0], clipboard,
        XCB_XFIXES_SELECTION_EVENT_MASK_SET_SELECTION_OWNER |
            XCB_XFIXES_SELECTION_EVENT_MASK_SELECTION_WINDOW_DESTROY |
            XCB_XFIXES_SELECTION_EVENT_MASK_SELECTION_CLIENT_CLOSE);
    alive = true;
    auto weak = weak_from_this();
    events = new QSocketNotifier(xcb_get_file_descriptor(connection),
                                 QSocketNotifier::Read, this);
    connect(events, &QSocketNotifier::activated, this, [weak] {
      if (auto self = weak.lock())
        self->pump();
    });
    death = new QSocketNotifier(enginefd, QSocketNotifier::Read, this);
    connect(death, &QSocketNotifier::activated, this, [weak] {
      if (auto self = weak.lock())
        self->retire();
    });
    timer = new QTimer(this);
    timer->setInterval(25);
    connect(timer, &QTimer::timeout, this, [weak] {
      if (auto self = weak.lock()) {
        self->pump();
        for (auto entry = self->pending.begin(); entry != self->pending.end();)
          if (entry->deadline < self->clock.elapsed() ||
              entry->lease != self->lease)
            entry = self->pending.erase(entry);
          else
            ++entry;
      }
    });
    timer->start();
    announce();
    return true;
  }
  void announce() {
    if (!valid())
      return;
    ownerIndex ^= 1;
    xcb_set_selection_owner(connection, owners[ownerIndex], clipboard,
                            XCB_CURRENT_TIME);
    xcb_flush(connection);
  }
  void offerChanged(const QString &source) {
    if (!valid() || source == id)
      return;
    const auto identity = offers->identity();
    if (lastOffer == identity)
      return;
    lastOffer = identity;
    nativeOwner = 0;
    lease++;
    for (const auto &request : queued)
      notify(request, XCB_ATOM_NONE);
    queued.clear();
    if (receiving.active) {
      receiving.failed = true;
      receiving.done = true;
    }
    for (auto entry = pending.cbegin(); entry != pending.cend(); ++entry)
      xcb_change_property(connection, XCB_PROP_MODE_REPLACE,
                          xcb_window_t(entry.key() >> 32),
                          xcb_atom_t(entry.key()), entry->type, 8, 0, nullptr);
    pending.clear();
    announce();
  }
  void notify(const xcb_selection_request_event_t &request, xcb_atom_t result) {
    xcb_selection_notify_event_t response{};
    response.response_type = XCB_SELECTION_NOTIFY;
    response.time = request.time;
    response.requestor = request.requestor;
    response.selection = request.selection;
    response.target = request.target;
    response.property = result;
    xcb_send_event(connection, 0, request.requestor, XCB_EVENT_MASK_NO_EVENT,
                   reinterpret_cast<const char *>(&response));
    xcb_flush(connection);
  }
  void received() {
    const auto budget = receiving.limit;
    auto *reply = xcb_get_property_reply(
        connection,
        xcb_get_property(connection, 1, receiving.window, property,
                         XCB_GET_PROPERTY_TYPE_ANY, 0, (budget + 3) / 4),
        nullptr);
    if (!reply) {
      receiving.failed = receiving.done = true;
      return;
    }
    const auto size = xcb_get_property_value_length(reply);
    if (reply->bytes_after || size < 0 || uint32_t(size) > budget) {
      receiving.failed = receiving.done = true;
    } else if (reply->type == incr && !receiving.incremental &&
               reply->format == 32 && size == 4) {
      if (*static_cast<const uint32_t *>(xcb_get_property_value(reply)) >
          budget)
        receiving.failed = receiving.done = true;
      else
        receiving.incremental = true;
    } else if (reply->type != (receiving.target == targets
                                   ? xcb_atom_t(XCB_ATOM_ATOM)
                                   : receiving.target) ||
               reply->format != (receiving.target == targets ? 32 : 8) ||
               receiving.bytes.size() + size > budget)
      receiving.failed = receiving.done = true;
    else {
      receiving.bytes.append(
          static_cast<const char *>(xcb_get_property_value(reply)), size);
      if (!receiving.incremental || size == 0)
        receiving.done = true;
    }
    free(reply);
    xcb_flush(connection);
  }
  std::optional<QByteArray> fetch(xcb_atom_t target, xcb_window_t owner,
                                  xcb_timestamp_t stamp, uint32_t budget) {
    if (receiving.active || !valid() || nativeOwner != owner ||
        nativeStamp != stamp || client(owner) != engine)
      return {};
    auto *current = xcb_get_selection_owner_reply(
        connection, xcb_get_selection_owner(connection, clipboard), nullptr);
    const bool matched = current && current->owner == owner;
    free(current);
    if (!matched)
      return {};
    const auto epoch = lease;
    receiving = Receiving{};
    receiving.active = true;
    receiving.owner = owner;
    receiving.target = target;
    receiving.limit = budget;
    receiving.window = xcb_generate_id(connection);
    const uint32_t mask = XCB_EVENT_MASK_PROPERTY_CHANGE;
    xcb_create_window(connection, XCB_COPY_FROM_PARENT, receiving.window, root,
                      0, 0, 1, 1, 0, XCB_WINDOW_CLASS_INPUT_OUTPUT,
                      XCB_COPY_FROM_PARENT, XCB_CW_EVENT_MASK, &mask);
    xcb_convert_selection(connection, receiving.window, clipboard, target,
                          property, XCB_CURRENT_TIME);
    xcb_flush(connection);
    const auto deadline = clock.elapsed() + 5000;
    while (valid() && !receiving.done && clock.elapsed() < deadline) {
      pump();
      QCoreApplication::processEvents(QEventLoop::ExcludeUserInputEvents, 2);
      if (!receiving.done)
        QThread::msleep(1);
    }
    auto *after = xcb_get_selection_owner_reply(
        connection, xcb_get_selection_owner(connection, clipboard), nullptr);
    const bool same = after && after->owner == owner;
    free(after);
    pump();
    const bool ok = same && valid() && epoch == lease && receiving.done &&
                    !receiving.failed && nativeOwner == owner &&
                    nativeStamp == stamp;
    auto result = ok ? std::optional<QByteArray>(std::move(receiving.bytes))
                     : std::nullopt;
    xcb_destroy_window(connection, receiving.window);
    xcb_flush(connection);
    receiving = Receiving{};
    return result;
  }
  bool captureScheduled = false;
  xcb_window_t captureOwner = 0;
  xcb_timestamp_t captureStamp = 0;
  void scheduleCapture(xcb_window_t owner, xcb_timestamp_t stamp,
                       int delay = 0) {
    captureOwner = owner;
    captureStamp = stamp;
    if (captureScheduled || !valid())
      return;
    captureScheduled = true;
    auto weak = weak_from_this();
    QTimer::singleShot(delay, this, [weak] {
      if (auto self = weak.lock()) {
        self->captureScheduled = false;
        self->capture(self->captureOwner, self->captureStamp);
      }
    });
  }
  void capture(xcb_window_t owner, xcb_timestamp_t stamp) {
    if (!valid() || nativeOwner != owner || nativeStamp != stamp)
      return;
    if (receiving.active) {
      scheduleCapture(owner, stamp, 10);
      return;
    }
    const auto available = fetch(targets, owner, stamp, 512);
    if (!available || available->isEmpty() || available->size() % 4 ||
        available->size() > 512)
      return;
    QVector<xcb_atom_t> supported;
    for (qsizetype at = 0; at < available->size(); at += 4) {
      xcb_atom_t value;
      memcpy(&value, available->constData() + at, 4);
      supported.append(value);
    }
    QHash<QString, xcb_atom_t> mapping;
    if (supported.contains(utf8))
      mapping.insert("text/plain", utf8);
    else if (supported.contains(text))
      mapping.insert("text/plain", text);
    else if (supported.contains(plain))
      mapping.insert("text/plain", plain);
    if (supported.contains(html))
      mapping.insert("text/html", html);
    if (supported.contains(png)) {
      mapping.insert("image/png", png);
      mapping.insert("application/x-qt-image", png);
    }
    if (mapping.isEmpty())
      return;
    QHash<xcb_atom_t, QByteArray> bodies;
    qsizetype retained = 0;
    for (auto target : mapping) {
      if (bodies.contains(target))
        continue;
      auto body = fetch(target, owner, stamp, uint32_t(limit - retained));
      if (!body)
        return;
      retained += body->size();
      bodies.insert(target, std::move(*body));
    }
    if (!valid() || nativeOwner != owner || nativeStamp != stamp)
      return;
    publishing = true;
    publish(id,
            new CurrentClipboardMime(std::move(mapping), std::move(bodies)));
    publishing = false;
  }
  void scheduleQueue() {
    if (queuedScheduled || reading || queued.isEmpty() || !valid())
      return;
    queuedScheduled = true;
    auto weak = weak_from_this();
    QTimer::singleShot(0, this, [weak] {
      if (auto self = weak.lock()) {
        self->queuedScheduled = false;
        if (self->valid() && !self->reading && !self->queued.isEmpty())
          self->selection(self->queued.dequeue(), false);
        self->scheduleQueue();
      }
    });
  }
  void selection(const xcb_selection_request_event_t &request, bool synthetic) {
    if (!valid() || synthetic || request.selection != clipboard ||
        request.owner != owners[ownerIndex] ||
        client(request.requestor) != engine) {
      notify(request, XCB_ATOM_NONE);
      return;
    }
    if (reading) {
      if (queued.size() + pending.size() >= 7)
        notify(request, XCB_ATOM_NONE);
      else
        queued.enqueue(request);
      return;
    }
    auto *current = xcb_get_selection_owner_reply(
        connection, xcb_get_selection_owner(connection, clipboard), nullptr);
    const bool stillOwner = current && current->owner == request.owner;
    free(current);
    if (!stillOwner) {
      notify(request, XCB_ATOM_NONE);
      return;
    }
    const auto result =
        request.property == XCB_ATOM_NONE ? request.target : request.property;
    const auto epoch = lease;
    if (request.target == timestamp) {
      xcb_change_property(connection, XCB_PROP_MODE_REPLACE, request.requestor,
                          result, XCB_ATOM_INTEGER, 32, 1, &selectedAt);
      notify(request, result);
      return;
    }
    if (request.target != targets && request.target != utf8 &&
        request.target != text && request.target != plain &&
        request.target != png && request.target != html) {
      notify(request, XCB_ATOM_NONE);
      return;
    }
    const auto formats = offers->formats();
    qsizetype metadata = 0;
    for (const auto &format : formats)
      metadata += format.toUtf8().size();
    if (epoch != lease || !valid() || formats.size() > 128 ||
        metadata > 65536 || !offers->metadataAllowed()) {
      notify(request, XCB_ATOM_NONE);
      return;
    }
    if (request.target == targets) {
      QVector<xcb_atom_t> values{targets, timestamp};
      if (formats.contains("text/plain") ||
          formats.contains("text/plain;charset=utf-8"))
        values << utf8 << text << plain;
      if (formats.contains("image/png"))
        values << png;
      if (formats.contains("text/html"))
        values << html;
      xcb_change_property(connection, XCB_PROP_MODE_REPLACE, request.requestor,
                          result, XCB_ATOM_ATOM, 32, uint32_t(values.size()),
                          values.data());
      notify(request, result);
      return;
    }
    const auto mime = request.target == png    ? QString("image/png")
                      : request.target == html ? QString("text/html")
                                               : QString("text/plain");
    reading = true;
    auto body =
        offers->read(mime, limit, [&] { return valid() && epoch == lease; });
    reading = false;
    scheduleQueue();
    if (!body || !valid() || epoch != lease) {
      notify(request, XCB_ATOM_NONE);
      return;
    }
    qsizetype retained = body->size();
    for (const auto &transfer : pending)
      retained += transfer.bytes.size();
    const quint64 key = (quint64(request.requestor) << 32) | result;
    if (body->size() > 65536) {
      if (pending.size() >= 8 || retained > limit || pending.contains(key)) {
        notify(request, XCB_ATOM_NONE);
        return;
      }
      const uint32_t mask =
          XCB_EVENT_MASK_PROPERTY_CHANGE | XCB_EVENT_MASK_STRUCTURE_NOTIFY;
      xcb_change_window_attributes(connection, request.requestor,
                                   XCB_CW_EVENT_MASK, &mask);
      const uint32_t length = uint32_t(body->size());
      xcb_change_property(connection, XCB_PROP_MODE_REPLACE, request.requestor,
                          result, incr, 32, 1, &length);
      pending.insert(key, {request.target, std::move(*body), 0,
                           clock.elapsed() + 5000, epoch});
      notify(request, result);
    } else {
      xcb_change_property(connection, XCB_PROP_MODE_REPLACE, request.requestor,
                          result, request.target, 8, uint32_t(body->size()),
                          body->constData());
      notify(request, result);
    }
  }
  void pump() {
    if (!valid())
      return;
    while (auto *raw = xcb_poll_for_event(connection)) {
      const auto type = raw->response_type & 0x7f;
      const bool synthetic = raw->response_type & 0x80;
      if (type == eventBase + XCB_XFIXES_SELECTION_NOTIFY && !synthetic) {
        const auto *event =
            reinterpret_cast<xcb_xfixes_selection_notify_event_t *>(raw);
        if (event->owner == owners[ownerIndex])
          selectedAt = event->selection_timestamp;
        if (receiving.active && event->owner != receiving.owner)
          receiving.failed = receiving.done = true;
        if (event->owner && event->owner != owners[0] &&
            event->owner != owners[1]) {
          if (client(event->owner) == engine) {
            nativeOwner = event->owner;
            nativeStamp = event->selection_timestamp;
            scheduleCapture(nativeOwner, nativeStamp);
          } else {
            nativeOwner = 0;
            announce();
          }
        }
      } else if (type == XCB_SELECTION_REQUEST)
        selection(*reinterpret_cast<xcb_selection_request_event_t *>(raw),
                  synthetic);
      else if (type == XCB_SELECTION_NOTIFY && receiving.active &&
               !receiving.done) {
        const auto *event =
            reinterpret_cast<xcb_selection_notify_event_t *>(raw);
        if (event->requestor == receiving.window &&
            event->selection == clipboard &&
            event->target == receiving.target) {
          if (event->property == XCB_ATOM_NONE)
            receiving.failed = receiving.done = true;
          else if (event->property == property)
            received();
        }
      } else if (type == XCB_PROPERTY_NOTIFY && !synthetic) {
        const auto *event =
            reinterpret_cast<xcb_property_notify_event_t *>(raw);
        if (receiving.active && receiving.incremental &&
            event->window == receiving.window && event->atom == property &&
            event->state == XCB_PROPERTY_NEW_VALUE)
          received();
        const quint64 key = (quint64(event->window) << 32) | event->atom;
        auto transfer = pending.find(key);
        if (event->state == XCB_PROPERTY_DELETE && transfer != pending.end()) {
          if (transfer->lease != lease || transfer->deadline < clock.elapsed())
            pending.erase(transfer);
          else {
            const auto size = qMin<qsizetype>(65536, transfer->bytes.size() -
                                                         transfer->offset);
            xcb_change_property(connection, XCB_PROP_MODE_REPLACE,
                                event->window, event->atom, transfer->type, 8,
                                uint32_t(size),
                                transfer->bytes.constData() + transfer->offset);
            transfer->offset += size;
            if (size == 0)
              pending.erase(transfer);
            xcb_flush(connection);
          }
        }
      } else if (type == XCB_DESTROY_NOTIFY && !synthetic) {
        const auto *event = reinterpret_cast<xcb_destroy_notify_event_t *>(raw);
        for (auto transfer = pending.begin(); transfer != pending.end();)
          if (xcb_window_t(transfer.key() >> 32) == event->window)
            transfer = pending.erase(transfer);
          else
            ++transfer;
      }
      free(raw);
      if (!valid())
        break;
    }
    if (xcb_connection_has_error(connection))
      retire();
  }
};
ClipboardCohort::ClipboardCohort(std::shared_ptr<State> value)
    : state(std::move(value)) {}
ClipboardCohort::~ClipboardCohort() { state->retire(); }
std::shared_ptr<ClipboardCohort> ClipboardCohort::create(
    QString id, int socket, int authority, int pidfd, qint64 engine,
    std::shared_ptr<NativeWaylandOffers> offers, Publish publish) {
  auto state = std::make_shared<State>(std::move(id), pidfd, quint32(engine),
                                       std::move(offers), std::move(publish));
  if (!state->initialize(socket, authority) || !state->valid())
    return {};
  return std::shared_ptr<ClipboardCohort>(
      new ClipboardCohort(std::move(state)));
}
void ClipboardCohort::retire() { state->retire(); }
void ClipboardCohort::offerChanged(const QString &source) {
  state->offerChanged(source);
}

bool ClipboardCohort::alive() const { return state->valid(); }
