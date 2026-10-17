#include <QHash>
// Qt browser guest host. One process/partition, multiple tabs; Rust owns T3
// authentication and lifecycle. No WebChannel or native QObject enters a page.
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickWindow>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QTimer>
#include <QSocketNotifier>
#include <QFile>
#include <QDir>
#include <QMouseEvent>
#include <QWheelEvent>
#include <QKeyEvent>
#include <QNetworkCookie>
#include <QDateTime>
#include <QSet>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <QtWebEngineCore/QWebEngineCookieStore>
#include <atomic>
#include <cmath>
#include <QTimeZone>
#include <QRegularExpression>
#include <fcntl.h>
#include <unistd.h>
class BrowserHost:public QObject {
 Q_OBJECT
 QQuickWindow *window=nullptr;
 QString frameDirectory;
 QByteArray input;
 QTimer capture;
 std::atomic<bool> grabbing{false};
 int revision=0;
 bool ready=false;
 bool resetSessions=false;
 QQuickWebEngineProfile *profile=nullptr;
 QTimer cookieTimeout;
 QNetworkCookie expectedCookie;
 QString expectedDomain;
 QString cookieRequest;
 QHash<QString,QNetworkCookie> observedCookies;
 QString cookieKey(const QNetworkCookie &cookie,const QString &domain=QString()){return QString::fromUtf8(QJsonDocument(QJsonArray{domain.isEmpty()?cookie.domain():domain,QString::fromUtf8(cookie.name()),cookie.path()}).toJson(QJsonDocument::Compact));}
 bool matchesCookie(const QNetworkCookie &cookie)const{return cookie.name()==expectedCookie.name()&&cookie.path()==expectedCookie.path()&&cookie.domain()==expectedDomain&&cookie.value()==expectedCookie.value()&&cookie.isHttpOnly()==expectedCookie.isHttpOnly()&&cookie.isSecure()==expectedCookie.isSecure()&&cookie.isSessionCookie()==expectedCookie.isSessionCookie()&&(cookie.isSessionCookie()||cookie.expirationDate().toSecsSinceEpoch()==expectedCookie.expirationDate().toSecsSinceEpoch());}
 void cookieResult(const QString &state,const QString &error=QString()){
  if(cookieRequest.isEmpty())return;auto id=cookieRequest;cookieRequest.clear();cookieTimeout.stop();expectedCookie=QNetworkCookie();event({{"type","cookie-imported"},{"requestId",id},{"state",state},{"error",error}});
 }

public:
 BrowserHost(QString directory,bool reset):frameDirectory(directory),resetSessions(reset){
  capture.setSingleShot(true);capture.setInterval(33);
  connect(&capture,&QTimer::timeout,this,[this](){
   if(!window)return;grabbing=true;auto image=window->grabWindow();grabbing=false;if(image.isNull())return;
   auto file=QString("frame-%1.png").arg(++revision%2);auto path=QDir(frameDirectory).filePath(file),temp=path+".next";
   if(!image.save(temp,"PNG"))return;QFile::setPermissions(temp,QFileDevice::ReadOwner|QFileDevice::WriteOwner);
   if(::rename(QFile::encodeName(temp).constData(),QFile::encodeName(path).constData())!=0)return;
   event({{"type","frame"},{"file",file},{"revision",revision},{"width",image.width()},{"height",image.height()},{"surfaceId",window->property("surfaceId").toString()},{"tabKey",window->property("selected").toString()}});
  });
 }
 Q_INVOKABLE void event(const QJsonObject &value){auto bytes=QJsonDocument(value).toJson(QJsonDocument::Compact);if(bytes.size()>2*1024*1024)bytes="{\"type\":\"runtime-error\",\"error\":\"browser_output_too_large\"}";fprintf(stdout,"%s\n",bytes.constData());fflush(stdout);}
 Q_INVOKABLE QString externalProtocolPromptUrl(const QString &raw){
  static const QSet<QString> denied={"about","blob","chrome","data","devtools","file","filesystem","http","https","javascript","view-source"};
  QUrl url(raw);return url.isValid()&&!url.scheme().isEmpty()&&!denied.contains(url.scheme().toLower())?url.toString():QString();
 }
 void attachProfile(QQuickWebEngineProfile *p){profile=p;cookieTimeout.setSingleShot(true);cookieTimeout.setInterval(8000);
  connect(&cookieTimeout,&QTimer::timeout,this,[this](){cookieResult("uncertain","browser_cookie_import_timeout");});
  // Qt 6.11 omits SameSite from cookieAdded. The setter transports it;
  // persisted-store fixture checks cover that field independently.
  connect(p->cookieStore(),&QWebEngineCookieStore::cookieAdded,this,[this](const QNetworkCookie &cookie){observedCookies.insert(cookieKey(cookie),cookie);if(!cookieRequest.isEmpty()&&matchesCookie(cookie))cookieResult("complete");});
  connect(p->cookieStore(),&QWebEngineCookieStore::cookieRemoved,this,[this](const QNetworkCookie &cookie){observedCookies.remove(cookieKey(cookie));});
 }
 void importCookie(const QJsonObject &command){
  const auto id=command["requestId"].toString();if(id.isEmpty()||id.size()>128)return;
  if(!cookieRequest.isEmpty()){event({{"type","cookie-imported"},{"requestId",id},{"state","failed"},{"error","browser_cookie_import_busy"}});return;}
  const auto value=command["cookie"].toObject();QUrl url(value["url"].toString());auto name=value["name"].toString(),contents=value["value"].toString();
  if(!profile||!url.isValid()||!QSet<QString>{"http","https"}.contains(url.scheme())||url.host().isEmpty()||!value["name"].isString()||!value["value"].isString()||name.toUtf8().size()>4096||contents.toUtf8().size()>16384){event({{"type","cookie-imported"},{"requestId",id},{"state","failed"},{"error","invalid_browser_cookie"}});return;}
  QNetworkCookie cookie(name.toUtf8(),contents.toUtf8());cookie.setPath(value["path"].toString("/"));cookie.setSecure(value["secure"].toBool());cookie.setHttpOnly(value["httpOnly"].toBool());
  if(value.contains("domain"))cookie.setDomain(value["domain"].toString());
  if(value.contains("expirationDate")){double expires=value["expirationDate"].toDouble();if(!std::isfinite(expires)||expires<0||expires>253402300799.0){event({{"type","cookie-imported"},{"requestId",id},{"state","failed"},{"error","invalid_browser_cookie"}});return;}cookie.setExpirationDate(QDateTime::fromSecsSinceEpoch(qint64(expires),QTimeZone::UTC));}
  const auto sameSite=value["sameSite"].toString();if(sameSite=="strict")cookie.setSameSitePolicy(QNetworkCookie::SameSite::Strict);else if(sameSite=="lax")cookie.setSameSitePolicy(QNetworkCookie::SameSite::Lax);else if(sameSite=="no_restriction")cookie.setSameSitePolicy(QNetworkCookie::SameSite::None);
  expectedCookie=cookie;expectedDomain=cookie.domain().isEmpty()?url.host():cookie.domain();cookieRequest=id;cookieTimeout.start();profile->cookieStore()->loadAllCookies();profile->cookieStore()->setCookie(cookie,url);
  // Identical replacements need not emit cookieAdded. Presence is provisional;
  // Rust verifies persisted metadata (including SameSite) after final flush.
  auto previous=observedCookies.constFind(cookieKey(cookie,expectedDomain));if(previous!=observedCookies.constEnd()&&matchesCookie(previous.value()))cookieResult("present");
 }
 Q_INVOKABLE void requestFrame(){if(!capture.isActive())capture.start();}
 void attach(QQuickWindow *w){window=w;
  connect(w,&QQuickWindow::afterRendering,this,[this](){if(!grabbing.load())QMetaObject::invokeMethod(this,[this](){requestFrame();},Qt::QueuedConnection);},Qt::DirectConnection);
  // The private initializer has constructed an adapter. Cookie manager requests
  // share one ordered channel: clear old-session cookies before any open/import.
  profile->cookieStore()->loadAllCookies();if(resetSessions)profile->cookieStore()->deleteSessionCookies();
  event({{"type","ready"},{"frameDirectory",frameDirectory}});ready=true;requestFrame();
 }
 void command(const QJsonObject &command){
  if(!ready||!window)return;const QString type=command["type"].toString();
  if(type=="close"){QCoreApplication::quit();return;}
  if(type=="import-cookie"){importCookie(command);return;}
  if(type=="resize"){const auto w=command["width"].toInt(),h=command["height"].toInt();if(w>=240&&h>=240&&w<=3840&&h<=3840&&qint64(w)*h<=3840*2160)window->resize(w,h);}
  else if(type=="press"||type=="release"||type=="move"||type=="double"){
   QPointF p(command["x"].toDouble(),command["y"].toDouble());auto kind=type=="press"?QEvent::MouseButtonPress:type=="release"?QEvent::MouseButtonRelease:type=="double"?QEvent::MouseButtonDblClick:QEvent::MouseMove;
   QMouseEvent e(kind,p,p,p,Qt::MouseButton(command["button"].toInt()),Qt::MouseButtons(command["buttons"].toInt()),Qt::KeyboardModifiers(command["modifiers"].toInt()));QCoreApplication::sendEvent(window,&e);
  }else if(type=="wheel"){
   QPointF p(command["x"].toDouble(),command["y"].toDouble());QWheelEvent e(p,p,QPoint(command["px"].toInt(),command["py"].toInt()),QPoint(command["ax"].toInt(),command["ay"].toInt()),Qt::NoButton,Qt::KeyboardModifiers(command["modifiers"].toInt()),Qt::NoScrollPhase,false);QCoreApplication::sendEvent(window,&e);
  }else if(type=="key"||type=="keyup"){
   QKeyEvent e(type=="key"?QEvent::KeyPress:QEvent::KeyRelease,command["key"].toInt(),Qt::KeyboardModifiers(command["modifiers"].toInt()),command["text"].toString());QCoreApplication::sendEvent(window,&e);
  }else{
   QMetaObject::invokeMethod(window,"command",Q_ARG(QVariant,command.toVariantMap()));
  }
  requestFrame();
 }
 void readInput(){
  char bytes[16384];ssize_t size;while((size=::read(STDIN_FILENO,bytes,sizeof bytes))>0){input.append(bytes,size);if(input.size()>65536){QCoreApplication::quit();return;}}
  if(size==0){QCoreApplication::quit();return;}
  int end;while((end=input.indexOf('\n'))>=0){auto row=input.left(end);input.remove(0,end+1);auto parsed=QJsonDocument::fromJson(row);if(parsed.isObject())command(parsed.object());}
 }
};
extern "C" int t3_browser_main(const char *payload,const char *qml){
 int argc=1;char name[]="omarchy-t3-browser";char *argv[]={name,nullptr};QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);QtWebEngineQuick::initialize();QGuiApplication app(argc,argv);
 app.setApplicationName("omarchy-t3-browser");app.setApplicationDisplayName("T3 Code · Browser");
 qInstallMessageHandler([](QtMsgType,const QMessageLogContext&,const QString &message){fprintf(stderr,"%s\n",qPrintable(message));});
 const auto data=QJsonDocument::fromJson(payload).object();
 BrowserHost host(data["frameDirectory"].toString(),!data["restoreSessionCookies"].toBool());
 QQuickWebEngineProfile profile;
 if(data["persistent"].toBool()){
  profile.setStorageName(data["partition"].toString());profile.setOffTheRecord(false);
  profile.setPersistentStoragePath(QDir(data["profileDirectory"].toString()).filePath("storage"));profile.setCachePath(QDir(data["profileDirectory"].toString()).filePath("cache"));
  profile.setHttpCacheType(QQuickWebEngineProfile::DiskHttpCache);profile.setPersistentCookiesPolicy(QQuickWebEngineProfile::ForcePersistentCookies);
 }else{profile.setOffTheRecord(true);profile.setHttpCacheType(QQuickWebEngineProfile::MemoryHttpCache);profile.setPersistentCookiesPolicy(QQuickWebEngineProfile::NoPersistentCookies);}
 host.attachProfile(&profile);
 QQmlApplicationEngine engine;engine.rootContext()->setContextProperty("browserHost",&host);engine.rootContext()->setContextProperty("browserProfile",&profile);engine.rootContext()->setContextProperty("browserData",data.toVariantMap());
 engine.load(QUrl::fromLocalFile(qml));if(engine.rootObjects().isEmpty())return 1;
 QObject::connect(&engine,&QQmlEngine::quit,&app,&QCoreApplication::quit);QObject::connect(&engine,&QQmlEngine::exit,&app,&QCoreApplication::exit);
 host.attach(qobject_cast<QQuickWindow*>(engine.rootObjects().first()));fcntl(STDIN_FILENO,F_SETFL,fcntl(STDIN_FILENO,F_GETFL)|O_NONBLOCK);
 QSocketNotifier input(STDIN_FILENO,QSocketNotifier::Read);QObject::connect(&input,&QSocketNotifier::activated,&app,[&host](){host.readInput();});
 return app.exec();
}
#include "browser-host.moc"
