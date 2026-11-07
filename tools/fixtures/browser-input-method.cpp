// Actual GUI item and QInputMethodEvent production; no simulated engine
// context.
#include <QElapsedTimer>
#include <QGuiApplication>
#include <QInputMethodEvent>
#include <QImage>
#include <QJSValue>
#include <QJsonArray>
#include <QJsonObject>
#include <QProcess>
#include <QProcessEnvironment>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QThread>
#include <cstdio>
#include <cstdlib>
#include <QDir>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <unistd.h>
#include <fcntl.h>
#include <poll.h>
#include <cstring>
#include <cerrno>
#include <sys/stat.h>
#include <atomic>
#define CHECK(value)                                                           \
  do {                                                                         \
    if (!(value)) {                                                            \
      fprintf(stderr, "BROWSER_IME_ASSERT line=%d\n", __LINE__);               \
      std::exit(2);                                                            \
    }                                                                          \
  } while (0)
static bool waitFor(const std::function<bool()> &ready) {
  QElapsedTimer clock;
  clock.start();
  while (!ready() && clock.elapsed() < 5000) {
    QCoreApplication::processEvents();
    QThread::msleep(2);
  }
  return ready();
}
struct OwnDescriptor {int fd;dev_t device;ino_t inode;QString kind;};
static QList<OwnDescriptor> ownDescriptors() {
  QList<OwnDescriptor> result;
  for(const auto &entry:QDir("/proc/self/fd").entryList(QDir::AllEntries|QDir::NoDotAndDotDot)) {
    bool valid=false;const int fd=entry.toInt(&valid);struct stat status{};
    if(!valid || fstat(fd,&status))continue;
    char target[256]{};const auto path=QByteArray("/proc/self/fd/")+entry.toLatin1();
    const auto length=readlink(path.constData(),target,sizeof target);
    const auto name=length>0?QByteArray(target,int(length)):QByteArray();
    QString kind=S_ISSOCK(status.st_mode)?"socket":S_ISFIFO(status.st_mode)?"pipe":
      name.contains("pidfd")?"pidfd":name.contains("memfd")?"memory-file":
      name=="anon_inode:[eventfd]"?"eventfd":name=="anon_inode:[eventpoll]"?"eventpoll":
      name=="anon_inode:[timerfd]"?"timerfd":name.startsWith("anon_inode:")?"anonymous-other":"regular-or-device";
    result.append({fd,status.st_dev,status.st_ino,kind});
  }
  return result;
}
static bool sameIdentity(const OwnDescriptor &a,const OwnDescriptor &b) {
  return a.device==b.device && a.inode==b.inode && a.kind==b.kind;
}
static void descriptorDelta(const QList<OwnDescriptor> &before,const QList<OwnDescriptor> &after) {
  for(const auto &a:before) {
    bool found=false;for(const auto &b:after)if(sameIdentity(a,b))found=true;
    if(!found)fprintf(stderr,"OWN_FD_REMOVED fd=%d kind=%s dev=%llu inode=%llu\n",a.fd,qPrintable(a.kind),
      (unsigned long long)a.device,(unsigned long long)a.inode);
  }
  for(const auto &a:after) {
    bool found=false;for(const auto &b:before)if(sameIdentity(a,b))found=true;
    if(!found)fprintf(stderr,"OWN_FD_ADDED fd=%d kind=%s dev=%llu inode=%llu\n",a.fd,qPrintable(a.kind),
      (unsigned long long)a.device,(unsigned long long)a.inode);
  }
}
static void descriptorSnapshot(const char *stage,const QList<OwnDescriptor> &values,int total) {
  fprintf(stderr,"OWN_FD_SNAPSHOT stage=%s enumerated=%lld total=%d\n",stage,(long long)values.size(),total);
  CHECK(values.size()<=128);
  for(const auto &value:values)fprintf(stderr,"OWN_FD_ID stage=%s fd=%d kind=%s dev=%llu inode=%llu\n",
    stage,value.fd,qPrintable(value.kind),(unsigned long long)value.device,(unsigned long long)value.inode);
  const auto threads=QDir("/proc/self/task").entryList(QDir::Dirs|QDir::NoDotAndDotDot);
  CHECK(threads.size()<=64);
  for(const auto &thread:threads) {
    QFile info(QString("/proc/self/task/%1/comm").arg(thread));
    if(info.open(QIODevice::ReadOnly)) {
      const auto name=info.read(64).trimmed();
      fprintf(stderr,"OWN_THREAD stage=%s tid=%s name=%s\n",stage,qPrintable(thread),name.constData());
    }
  }
}
int main(int argc, char **argv) {
  QGuiApplication app(argc, argv);
  QQmlEngine engine;
  engine.addImportPath("/fixture/qml");
  QQmlComponent component(&engine);
  component.setData(R"(import QtQuick
import O3.Browser 1.0
BrowserInputItem {
 width:320;height:240;inputEnabled:true;scopeToken:"first"
 property var records:[]
 property var rejections:[]
 property var queries:[]
 onInput:function(token,event){records=records.concat([{token:token,event:event}]);if(event.text==="rotate")scopeToken="replacement"}
 onInputRejected:function(reason){rejections=rejections.concat([reason])}
 onContextRequested:function(token){queries=queries.concat([token])}
})",
                    QUrl("file:///fixture/input.qml"));
  CHECK(waitFor([&] { return component.status() != QQmlComponent::Loading; }));
  auto *item = qobject_cast<QQuickItem *>(component.create());
  if (!item) {
    fprintf(stderr, "%s\n", qPrintable(component.errorString()));
    return 2;
  }
  QQuickWindow window;
  window.setTitle("Owned Qt native IME producer");
  window.resize(320, 240);
  item->setParentItem(window.contentItem());
  window.show();
  item->forceActiveFocus();
  window.requestActivate();
  CHECK(waitFor([&] { return window.isActive() && item->hasActiveFocus(); }));
  auto records = [&] {
    return QJsonArray::fromVariantList(
        item->property("records").value<QJSValue>().toVariant().toList());
  };
  auto rejections = [&] {
    return QJsonArray::fromVariantList(
        item->property("rejections").value<QJSValue>().toVariant().toList());
  };
  CHECK(!item->property("contextAvailable").toBool());
  QInputMethodQueryEvent query(Qt::ImEnabled | Qt::ImSurroundingText |
                               Qt::ImCursorRectangle | Qt::ImCursorPosition |
                               Qt::ImAnchorPosition | Qt::ImHints);
  QCoreApplication::sendEvent(item, &query);
  CHECK(query.value(Qt::ImEnabled).toBool());
  for (auto kind : {Qt::ImSurroundingText, Qt::ImCursorRectangle,
                    Qt::ImCursorPosition, Qt::ImAnchorPosition, Qt::ImHints})
    CHECK(!query.value(kind).isValid());
  QInputMethodEvent commit;
  commit.setCommitString(QString::fromUtf8("拼音🙂"));
  QCoreApplication::sendEvent(item, &commit);
  CHECK(commit.isAccepted());
  CHECK(records().size() == 1);
  CHECK(records()[0].toObject()["token"] == "first");
  CHECK(records()[0].toObject()["event"].toObject()["text"] ==
        QString::fromUtf8("拼音🙂"));
  QInputMethodEvent preedit(QString::fromUtf8("にほん"),
                            {{QInputMethodEvent::Cursor, 1, 1, {}}});
  QCoreApplication::sendEvent(item, &preedit);
  CHECK(records().size() == 2);
  CHECK(records()[1].toObject()["event"].toObject()["selectionStart"] == 1);
  QInputMethodEvent next(QString::fromUtf8("ご"), {});
  next.setCommitString(QString::fromUtf8("日本"));
  QCoreApplication::sendEvent(item, &next);
  CHECK(records().size() == 4);
  CHECK(records()[2].toObject()["event"].toObject()["type"] == "text");
  CHECK(records()[3].toObject()["event"].toObject()["type"] == "composition");
  QInputMethodEvent cancel;
  QCoreApplication::sendEvent(item, &cancel);
  CHECK(records().size() == 5);
  CHECK(records()[4].toObject()["event"].toObject()["preedit"] == "");
  QInputMethodEvent replace;
  replace.setCommitString("x", -1, 1);
  QCoreApplication::sendEvent(item, &replace);
  CHECK(!replace.isAccepted());
  CHECK(records().size() == 5);
  CHECK(rejections().size() == 1);
  QInputMethodEvent selection("", {{QInputMethodEvent::Selection, 1, 2, {}}});
  QCoreApplication::sendEvent(item, &selection);
  CHECK(!selection.isAccepted());
  CHECK(records().size() == 5);
  CHECK(rejections().size() == 2);
  QInputMethodEvent oversized;
  oversized.setCommitString(QString(4097, QChar('x')));
  QCoreApplication::sendEvent(item, &oversized);
  CHECK(!oversized.isAccepted());
  CHECK(records().size() == 5);
  CHECK(rejections().size() == 3);
  QInputMethodEvent rotate("new-preedit", {});
  rotate.setCommitString("rotate");
  QCoreApplication::sendEvent(item, &rotate);
  CHECK(records().size() == 6);
  CHECK(records()[5].toObject()["token"] == "first");
  CHECK(item->property("scopeToken") == "replacement");
  {
    QQuickWindow foreign;
    foreign.setTitle("Owned inactive IME peer");
    foreign.resize(120, 120);
    foreign.show();
    foreign.requestActivate();
    CHECK(waitFor([&] { return foreign.isActive() && !window.isActive(); }));
    QInputMethodEvent inactive;
    inactive.setCommitString("discard-inactive-window");
    QCoreApplication::sendEvent(item, &inactive);
    CHECK(!inactive.isAccepted());
    CHECK(records().size() == 6);
    foreign.hide();
  }
  window.requestActivate();
  item->forceActiveFocus();
  CHECK(waitFor([&] { return window.isActive() && item->hasActiveFocus(); }));
  item->setProperty("inputEnabled", false);
  QInputMethodEvent disabled;
  disabled.setCommitString("discard");
  QCoreApplication::sendEvent(item, &disabled);
  CHECK(!disabled.isAccepted());
  CHECK(records().size() == 6);
  item->setParentItem(nullptr);
  delete item;
  QImage ownedFrame(16,16,QImage::Format_ARGB32_Premultiplied);
  ownedFrame.fill(Qt::white);
  CHECK(ownedFrame.save("/tmp/owned-browser-ime-frame.png"));
  QQmlComponent pageComponent(&engine);
  pageComponent.setData(R"(import QtQuick
import "page" as Product
Product.T3BrowserPage {
 width:320;height:240;interactive:true
 page:({requestId:"page-first",nativeInput:true,
   nativeInputBinding:{connection:"fixture-connection",surface:"fixture-surface",tab:"fixture-tab",frame:"fixture-frame"},
   nativeInputContent:{x:0,y:0,width:16,height:16}})
 function showFixtureFrame(){page=Object.assign({},page,{url:"file:///tmp/owned-browser-ime-frame.png",width:16,height:16})}
 function admitInputContext(cohort){page=Object.assign({},page,{nativeInputBinding:{connection:cohort+":owned-wayland-socket",surface:"owned-surface",tab:"owned-tab",frame:"owned-frame"}})}
 property var records:[]
 onInput:function(token,event){records=records.concat([{token:token,event:event}]);if(event.text==="rotate")page=Object.assign({},page,{requestId:"page-replacement"})}
})",
                        QUrl("file:///fixture/proof.qml"));
  CHECK(waitFor(
      [&] { return pageComponent.status() != QQmlComponent::Loading; }));
  auto *page = qobject_cast<QQuickItem *>(pageComponent.create());
  if (!page) {
    fprintf(stderr, "%s\n", qPrintable(pageComponent.errorString()));
    return 2;
  }
  page->setParentItem(window.contentItem());
  QQuickItem *native = nullptr;
  const bool mounted = waitFor([&] {
    native = qvariant_cast<QQuickItem *>(page->property("nativeInputItem"));
    return native;
  });
  if (!mounted) {
    fprintf(stderr,
            "INPUT_MOUNT_DIAGNOSTIC propertyType=%s native=%p enabled=%d "
            "pageFlag=%d request=%s\n",
            page->property("nativeInputItem").typeName(),
            page->property("nativeInputItem").value<QObject *>(),
            page->property("interactive").toBool(),
            page->property("page")
                .value<QJSValue>()
                .property("nativeInput")
                .toBool(),
            qPrintable(page->property("requestId").toString()));
    for (auto *child : page->findChildren<QObject *>()) {
      if (auto *failed = qobject_cast<QQmlComponent *>(child))
        fprintf(stderr, "LOADER_COMPONENT_ERROR %s\n",
                qPrintable(failed->errorString()));
      fprintf(
          stderr,
          "INPUT_CHILD type=%s name=%s active=%d source=%s status=%d item=%p\n",
          child->metaObject()->className(), qPrintable(child->objectName()),
          child->property("active").toBool(),
          qPrintable(child->property("source").toUrl().toString()),
          child->property("status").toInt(),
          static_cast<void *>(child->property("item").value<QObject *>()));
    }
  }
  CHECK(mounted);
  CHECK(!native->property("inputEnabled").toBool());
  CHECK(native->property("nativeBinding").toMap().isEmpty());
  CHECK(QMetaObject::invokeMethod(page,"showFixtureFrame"));
  CHECK(waitFor([&] { return page->property("ready").toBool() && native->property("inputEnabled").toBool(); }));
  CHECK(native->property("nativeBinding").toMap().value("connection") == "fixture-connection");
  CHECK(native->property("nativeContentRect").toRectF() == QRectF(0,0,16,16));
  CHECK(native->property("framePresentation").toRectF() == QRectF(40,0,240,240));
  native->forceActiveFocus();
  CHECK(waitFor([&] { return native->hasActiveFocus(); }));
  QInputMethodEvent pagePreedit(QString::fromUtf8("かな"), {});
  QCoreApplication::sendEvent(native, &pagePreedit);
  CHECK(pagePreedit.isAccepted());
  QInputMethodEvent pageCommit;
  pageCommit.setCommitString(QString::fromUtf8("仮名🙂"));
  QCoreApplication::sendEvent(native, &pageCommit);
  CHECK(pageCommit.isAccepted());
  auto pageRecords = [&] {
    return QJsonArray::fromVariantList(
        page->property("records").value<QJSValue>().toVariant().toList());
  };
  CHECK(pageRecords().size() == 2);
  CHECK(pageRecords()[0].toObject()["token"] == "page-first");
  CHECK(pageRecords()[1].toObject()["event"].toObject()["text"] ==
        QString::fromUtf8("仮名🙂"));
  QInputMethodEvent pageRotate("discard-new-composition", {});
  pageRotate.setCommitString("rotate");
  QCoreApplication::sendEvent(native, &pageRotate);
  CHECK(pageRecords().size() == 3);
  CHECK(page->property("requestId") == "page-replacement");
  page->setProperty("interactive", false);
  QInputMethodEvent pageDisabled;
  pageDisabled.setCommitString("discard");
  QCoreApplication::sendEvent(native, &pageDisabled);
  CHECK(!pageDisabled.isAccepted());
  CHECK(pageRecords().size() == 3);
  if (qEnvironmentVariableIsSet("O3_TEST_COMPOSITOR_IME")) {
    page->setProperty("interactive", true);
    native->forceActiveFocus();
    CHECK(
        waitFor([&] { return native->hasActiveFocus() && window.isActive(); }));
    QProcess method;
    method.setProgram("/fixture/input-method-peer");
    method.start();
    CHECK(method.waitForStarted(2000));
    bool ready = false, active = false;
    auto drain = [&] {
      while (method.canReadLine()) {
        const auto line = method.readLine().trimmed();
        if (line == "ready")
          ready = true;
        else if (line.startsWith("active "))
          active = true;
        else if (line.startsWith("inactive "))
          active = false;
      }
    };
    const bool activated = waitFor([&] {
      drain();
      return ready && active;
    });
    if (!activated)
      fprintf(stderr, "COMPOSITOR_IME_NOT_ACTIVE state=%d error=%s\n",
              int(method.state()), method.readAllStandardError().constData());
    CHECK(activated);
    const auto before = pageRecords().size();
    method.write("preedit\n");
    CHECK(method.waitForBytesWritten(1000));
    CHECK(waitFor([&] {
      drain();
      return pageRecords().size() > before;
    }));
    CHECK(pageRecords().last().toObject()["event"].toObject()["preedit"] ==
          QString::fromUtf8("にほん"));
    const auto composition =
        pageRecords().last().toObject()["event"].toObject();
    CHECK(composition["preedit"].toString().size() == 3);
    CHECK(composition["selectionStart"].toInt(-1) == 3);
    CHECK(composition["selectionEnd"].toInt(-1) == 3);
    CHECK(!composition.contains("replacementStart"));
    CHECK(!composition.contains("replacementEnd"));
    method.write("commit\n");
    CHECK(method.waitForBytesWritten(1000));
    CHECK(waitFor([&] {
      drain();
      for (const auto &record : pageRecords())
        if (record.toObject()["event"].toObject()["text"] ==
            QString::fromUtf8("日本"))
          return true;
      return false;
    }));
    CHECK(pageRecords().last().toObject()["token"] == "page-replacement");
    method.write("quit\n");
    CHECK(method.waitForBytesWritten(1000));
    CHECK(method.waitForFinished(2000) && method.exitCode() == 0);
    puts("BROWSER_COMPOSITOR_IME_PASS input-method-v2 text-input-v3 "
         "actual-T3BrowserPage unicode-preedit-commit");
  }
  QQmlComponent platformComponent(&engine);
  platformComponent.setData(R"(import O3.Browser 1.0
ClipboardPlatform {
 property bool everAdmitted:false
 onWorkerAdmittedChanged:if(workerAdmitted)everAdmitted=true
})",QUrl("file:///fixture/reset.qml"));
  CHECK(waitFor([&]{return platformComponent.status()!=QQmlComponent::Loading;}));
  auto *platform=platformComponent.create();
  CHECK(platform && waitFor([&]{return platform->property("available").toBool();}));
  CHECK(page->property("ready").toBool());
  std::atomic<bool> synchronized{false},presented{false};
  const auto sync=QObject::connect(&window,&QQuickWindow::afterSynchronizing,&window,
      [&]{synchronized.store(true);},Qt::DirectConnection);
  const auto swap=QObject::connect(&window,&QQuickWindow::frameSwapped,&window,
      [&]{if(synchronized.load())presented.store(true);},Qt::DirectConnection);
  window.update();
  CHECK(waitFor([&]{return presented.load();}));
  QObject::disconnect(sync);QObject::disconnect(swap);
  puts("OWN_RENDER_BASELINE_BARRIER image-ready after-synchronizing frame-swapped");
  const auto endpoint=platform->property("endpoint").toString().toUtf8();
  int canary[2];CHECK(pipe2(canary,O_CLOEXEC)==0);
  const auto baselineDescriptors=ownDescriptors();
  const auto baseline=QDir("/proc/self/fd").entryList(QDir::AllEntries|QDir::NoDotAndDotDot).size();
  descriptorSnapshot("baseline",baselineDescriptors,int(baseline));
  int gate[2];CHECK(socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0,gate)==0);
  const auto worker=fork();CHECK(worker>=0);
  if(worker==0) {
    ::close(gate[0]);
    char command=0;
    if(recv(gate[1],&command,1,0)!=1)_exit(11);
    const int peer=socket(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0);
    sockaddr_un address{};address.sun_family=AF_UNIX;
    memcpy(address.sun_path+1,endpoint.constData(),size_t(endpoint.size()));
    if(peer<0 || ::connect(peer,reinterpret_cast<sockaddr *>(&address),
       socklen_t(offsetof(sockaddr_un,sun_path)+1+endpoint.size())))_exit(12);
    const char hello[]="{\"op\":\"hello\",\"version\":1}";
    if(send(peer,hello,sizeof hello-1,MSG_NOSIGNAL)!=sizeof hello-1)_exit(13);
    pollfd reply{peer,POLLIN,0};
    if(poll(&reply,1,3000)!=1)_exit(14);
    if(send(gate[1],"a",1,MSG_NOSIGNAL)!=1)_exit(17);
    if(recv(gate[1],&command,1,0)!=1)_exit(18);
    // Leave the admitted reply unread: Linux reports an actual peer reset.
    ::close(peer);
    if(send(gate[1],"c",1,MSG_NOSIGNAL)!=1)_exit(15);
    if(recv(gate[1],&command,1,0)!=1)_exit(16);
    _exit(0);
  }
  ::close(gate[1]);
  QString binding;
  CHECK(QMetaObject::invokeMethod(platform,"bindWorker",Q_RETURN_ARG(QString,binding),Q_ARG(qint64,qint64(worker))));
  CHECK(!binding.isEmpty());
  CHECK(send(gate[0],"g",1,MSG_NOSIGNAL)==1);
  CHECK(waitFor([&]{return platform->property("workerAdmitted").toBool();}));
  const auto admittedDescriptors=ownDescriptors();
  QList<OwnDescriptor> registrationDescriptors;
  for(const auto &entry:admittedDescriptors) {
    bool own=false;
    if(entry.kind=="socket") {
      ucred peer{};socklen_t size=sizeof peer;
      own=getsockopt(entry.fd,SOL_SOCKET,SO_PEERCRED,&peer,&size)==0 && peer.pid==worker && peer.uid==getuid();
    } else if(entry.kind=="pidfd") {
      QFile info(QString("/proc/self/fdinfo/%1").arg(entry.fd));
      if(info.open(QIODevice::ReadOnly))
        for(const auto &line:info.read(4096).split('\n'))
          if(line.startsWith("Pid:"))own=line.mid(4).trimmed().toLongLong()==worker;
    }
    if(own)registrationDescriptors.append(entry);
  }
  CHECK(registrationDescriptors.size()==2);
  CHECK(send(gate[0],"c",1,MSG_NOSIGNAL)==1);
  CHECK(waitFor([&]{return platform->property("everAdmitted").toBool() &&
      !platform->property("workerAdmitted").toBool() &&
      platform->property("_o3RegistrationReceiveError").toInt()==ECONNRESET;}));
  CHECK(kill(worker,0)==0);
  CHECK(send(gate[0],"q",1,MSG_NOSIGNAL)==1);
  int status=0;CHECK(waitpid(worker,&status,0)==worker && WIFEXITED(status) && WEXITSTATUS(status)==0);
  ::close(gate[0]);
  CHECK(QMetaObject::invokeMethod(platform,"retireWorker",Q_ARG(QString,binding)));
  const auto retiredDescriptors=ownDescriptors();
  const auto retiredCount=QDir("/proc/self/fd").entryList(QDir::AllEntries|QDir::NoDotAndDotDot).size();
  descriptorSnapshot("retired",retiredDescriptors,int(retiredCount));
  descriptorDelta(baselineDescriptors,retiredDescriptors);
  for(const auto &owned:registrationDescriptors)
    for(const auto &remaining:retiredDescriptors)CHECK(!sameIdentity(owned,remaining));
  CHECK(fcntl(canary[0],F_GETFD)>=0 && fcntl(canary[1],F_GETFD)>=0);
  CHECK(retiredCount==baseline);
  ::close(canary[0]);::close(canary[1]);
  if (qEnvironmentVariableIsSet("O3_INPUT_CONTEXT_WORKER")) {
    page->setProperty("interactive",true);
    native->forceActiveFocus();
    QProcess sender;
    auto environment=QProcessEnvironment::systemEnvironment();
    environment.insert("O3_GUI_CLIPBOARD_ENDPOINT",QString::fromUtf8(endpoint));
    sender.setProcessEnvironment(environment);
    sender.start(QString::fromLocal8Bit(qgetenv("O3_INPUT_CONTEXT_WORKER")),{});
    CHECK(sender.waitForStarted(3000));
    QString owner;
    CHECK(QMetaObject::invokeMethod(platform,"bindWorker",Q_RETURN_ARG(QString,owner),Q_ARG(qint64,sender.processId())));
    CHECK(!owner.isEmpty());
    sender.write("go\n");
    QByteArray metadata;
    QJsonArray events;
    auto phase=[&](const QString &kind) {
      return waitFor([&] {
        metadata+=sender.readAllStandardOutput();
        CHECK(metadata.size()<=4096);
        while(metadata.contains('\n')) {
          const auto at=metadata.indexOf('\n');
          events.append(QJsonDocument::fromJson(metadata.left(at)).object());
          metadata.remove(0,at+1);
          CHECK(events.size()<=16);
        }
        for(const auto &value:events)if(value.toObject()["kind"]==kind)return true;
        return false;
      });
    };
    CHECK(phase("input-context-registered"));
    const auto cohort=events.first().toObject()["cohort"].toString();
    CHECK(!cohort.isEmpty());
    CHECK(QMetaObject::invokeMethod(page,"admitInputContext",Q_ARG(QVariant,QVariant(cohort))));
    native->forceActiveFocus();
    CHECK(waitFor([&]{return native->hasActiveFocus() && native->property("inputEnabled").toBool();}));
    sender.write("publish\n");
    CHECK(phase("input-state-published"));
    CHECK(waitFor([&]{return native->property("contextAvailable").toBool();}));
    QInputMethodQueryEvent actual(Qt::ImSurroundingText|Qt::ImCursorPosition|Qt::ImCursorRectangle);
    QCoreApplication::sendEvent(native,&actual);
    CHECK(actual.value(Qt::ImSurroundingText).toString().size()==7);
    CHECK(actual.value(Qt::ImCursorPosition).toInt()==7);
    CHECK(actual.value(Qt::ImCursorRectangle).toRectF().width()==0);
    sender.write("invalidate\n");
    CHECK(phase("input-state-invalidated"));
    CHECK(waitFor([&]{return !native->property("contextAvailable").toBool();}));
    sender.write("retire\n");
    CHECK(phase("input-state-republished"));
    CHECK(waitFor([&]{return native->property("contextAvailable").toBool();}));
    sender.write("invalid-state\n");
    CHECK(phase("input-invalid-state-refused"));
    CHECK(waitFor([&]{return !native->property("contextAvailable").toBool();}));
    sender.write("restore\n");
    CHECK(phase("input-state-restored"));
    CHECK(waitFor([&]{return native->property("contextAvailable").toBool();}));
    sender.write("finish\n");
    CHECK(phase("input-channel-retired"));
    CHECK(sender.state()==QProcess::Running);
    CHECK(!native->property("contextAvailable").toBool());
    sender.write("exit\n");
    CHECK(waitFor([&]{return sender.state()==QProcess::NotRunning;}));
    CHECK(sender.exitCode()==0);
    puts("BROWSER_RUST_CONTEXT_REGISTRATION_PASS actual-worker engine-ancestry pidfd scoped-GUI-request private-state invalidation retirement");
  }
  delete platform;
  puts("BROWSER_REGISTRATION_REAL_RESET_PASS live-worker unread-reply exact-FD-count");
  page->setParentItem(nullptr);
  delete page;
  window.hide();
  puts("BROWSER_QT_INPUT_METHOD_PASS real-Qt-events actual-T3BrowserPage "
       "unicode-preedit-commit-cancel unavailable-context replacement-refusal "
       "stale-scope disabled bound");
  return 0;
}
