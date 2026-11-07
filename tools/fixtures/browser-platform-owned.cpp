// Exercise the actual QML plugin with real child credentials and pidfds.
#include <QGuiApplication>
#include <QQmlEngine>
#include <QQmlComponent>
#include <QProcess>
#include <QProcessEnvironment>
#include <QElapsedTimer>
#include <QThread>
#include <QJsonDocument>
#include <QJsonObject>
#include <cstdio>
#include <unistd.h>
static bool waitFor(const std::function<bool()> &ready){QElapsedTimer clock;clock.start();while(!ready()&&clock.elapsed()<4000){QCoreApplication::processEvents();QThread::msleep(2);}return ready();}
int main(int argc,char **argv){QGuiApplication app(argc,argv);QQmlEngine engine;engine.addImportPath("/fixture/qml");QQmlComponent component(&engine);component.setData("import O3.Browser 1.0\nClipboardPlatform {}",QUrl("file:///fixture/member.qml"));if(!waitFor([&]{return component.status()!=QQmlComponent::Loading;})){fprintf(stderr,"Component remained loading status=%d errors=%s\n",int(component.status()),qPrintable(component.errorString()));return 2;}auto *member=component.create();if(!member){fprintf(stderr,"Component creation failed status=%d errors=%s\n",int(component.status()),qPrintable(component.errorString()));return 2;}
 auto check=[](bool value){if(!value)qFatal("Owned browser GUI admission assertion failed");};check(member->property("available").toBool());const auto endpoint=member->property("endpoint").toString();check(!endpoint.isEmpty());
 auto bind=[&](qint64 pid){QString binding;check(QMetaObject::invokeMethod(member,"bindWorker",Q_RETURN_ARG(QString,binding),Q_ARG(qint64,pid)));return binding;};auto retire=[&](const QString &binding){check(QMetaObject::invokeMethod(member,"retireWorker",Q_ARG(QString,binding)));};auto admitted=[&]{return member->property("workerAdmitted").toBool();};
 check(bind(-1).isEmpty());check(bind(getpid()).isEmpty());
 auto launch=[&](QProcess &peer){peer.setProgram("/usr/bin/python3");peer.setArguments({"/fixture/browser-platform-peer.py",endpoint});peer.start();check(peer.waitForStarted(2000));};auto response=[&](QProcess &peer){check(waitFor([&]{return peer.canReadLine();}));return peer.readLine().trimmed();};
 QProcess first;launch(first);const auto old=bind(first.processId());check(!old.isEmpty());check(response(first)=="admitted");check(waitFor(admitted));
 QProcess foreign;launch(foreign);check(response(foreign)=="rejected");check(foreign.waitForFinished(2000));check(admitted());
 first.write("transfer\n");check(response(first)=="closed");check(!admitted());QProcess other;launch(other);check(response(other)=="rejected");check(other.waitForFinished(2000));check(!admitted());first.write("quit\n");check(first.waitForFinished(2000));check(waitFor([&]{return !admitted();}));
 for(const auto command:{"oversized","rights","quit"}){QProcess peer;launch(peer);const auto current=bind(peer.processId());check(!current.isEmpty()&&current!=old);check(response(peer)=="admitted");check(admitted());retire(old);check(admitted());
  if(QByteArray(command)!="quit"){peer.write(QByteArray(command)+"\n");check(response(peer)=="closed");check(!admitted());}peer.write("quit\n");check(peer.waitForFinished(2000));check(waitFor([&]{return !admitted();}));
 }
 QProcess actual;auto environment=QProcessEnvironment::systemEnvironment();environment.insert("O3_GUI_CLIPBOARD_ENDPOINT",endpoint);actual.setProcessEnvironment(environment);actual.setProgram("/fixture/worker-probe");actual.start();check(actual.waitForStarted(2000));const auto actualBinding=bind(actual.processId());check(!actualBinding.isEmpty());check(response(actual)=="admitted");check(admitted());retire(old);check(admitted());retire(actualBinding);check(!admitted());actual.write("alive\n");check(response(actual)=="false");actual.write("quit\n");check(actual.waitForFinished(2000)&&actual.exitCode()==0);
 delete member;puts("BROWSER_GUI_PLATFORM_PASS exact-worker foreign-peer transferred-fd oversized rights stale-binding pidfd-death");return 0;}
