// QML previews of agent-written components. Runs in its own process (inside a
// Landlock domain set up by Rust) with a restricted engine: only the allowed
// Qt modules are importable, every network request fails, URLs outside those
// modules are refused and opening links does nothing. No WebEngine here.
#include "document-host.h"
#include <QDesktopServices>
#include <QFileInfo>
#include <QJsonArray>
#include <QLibraryInfo>
#include <QNetworkAccessManager>
#include <QNetworkRequest>
#include <QQmlAbstractUrlInterceptor>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlNetworkAccessManagerFactory>
#include <QQuickItem>
#include <QQuickView>
#include <fcntl.h>
#include <unistd.h>

// Directories (relative to Qt's QML import path) a preview may load from.
// "Files only" entries expose a module's own files but not its sub-modules
// (QtQuick.LocalStorage, QtQuick.Dialogs, QtQml.XmlListModel…).
struct AllowedDir{const char *path;bool recursive;};
static const AllowedDir allowedDirs[]={
 {"QML",true},{"QtQml",false},{"QtQml/Models",true},{"QtQml/WorkerScript",true},
 {"QtQuick",false},{"QtQuick/Controls",true},{"QtQuick/Templates",true},{"QtQuick/Layouts",true},
 {"QtQuick/Shapes",true},{"QtQuick/Effects",true},{"QtQuick/Window",true},
};

class BlockedNetwork: public QNetworkAccessManager{
public:
 using QNetworkAccessManager::QNetworkAccessManager;
protected:
 // Every request (http, file, data…) becomes an unknown-protocol failure.
 QNetworkReply *createRequest(Operation op,const QNetworkRequest &request,QIODevice *)override{
  QNetworkRequest blocked(request);blocked.setUrl(QUrl("t3-blocked:"));return QNetworkAccessManager::createRequest(op,blocked,nullptr);
 }
};
class BlockedNetworkFactory: public QQmlNetworkAccessManagerFactory{
public:
 QNetworkAccessManager *create(QObject *parent)override{auto manager=new BlockedNetwork(parent);manager->setAutoDeleteReplies(true);return manager;}
};

class PreviewUrls: public QQmlAbstractUrlInterceptor{
 QStringList recursive,filesOnly;
 QList<QUrl> trusted;
 QUrl blocked;
public:
 // Refused URLs point at a missing local file: a remote placeholder would make
 // Qt's type loader wait on the network (and hang a synchronous createQmlObject).
 PreviewUrls(const QString &imports,const QList<QUrl> &trusted,const QUrl &blocked):trusted(trusted),blocked(blocked){
  for(const auto &d:allowedDirs){auto real=QFileInfo(imports+"/"+d.path).canonicalFilePath();if(real.isEmpty())continue;(d.recursive?recursive:filesOnly)<<real;}
 }
 bool allowedPath(const QString &file)const{
  auto real=QFileInfo(file).canonicalFilePath();if(real.isEmpty())return false;
  for(const auto &dir:recursive)if(real.startsWith(dir+"/"))return true;
  auto parent=QFileInfo(real).absolutePath();
  return filesOnly.contains(parent);
 }
 QUrl intercept(const QUrl &url,DataType type)override{
  if(trusted.contains(url))return url;
  // Qt's built-in resources and inline data (images, SVG) are inert.
  if(url.scheme()=="qrc"||url.scheme()=="data")return url;
  if(url.isLocalFile()&&allowedPath(url.toLocalFile()))return url;
  // Versioned qmldir probes (QtQuick.6.12/…) are routine lookups, not refusals worth logging.
  if(type!=QmldirFile)fprintf(stderr,"QML preview: refused %s\n",qPrintable(url.toString().left(200)));
  return blocked;
 }
};

// A private import directory holding only the allowed modules (symlinks into
// Qt's own tree), so nothing else, Quickshell included, can be resolved.
static QString mirrorImports(const QString &frames,const QString &imports){
 QDir root(frames+"/qml-imports");root.removeRecursively();QDir().mkpath(root.path());
 for(const auto &d:allowedDirs){
  QString source=imports+"/"+d.path,target=root.filePath(d.path);
  if(!QFileInfo(source).isDir())continue;
  if(d.recursive){QDir().mkpath(QFileInfo(target).absolutePath());QFile::link(source,target);continue;}
  QDir().mkpath(target);
  for(const auto &file:QDir(source).entryInfoList(QDir::Files))QFile::link(file.absoluteFilePath(),target+"/"+file.fileName());
 }
 return root.path();
}

static QString errorText(const QQmlError &e){
 QString where=e.line()>0?QString("%1:%2  ").arg(e.line()).arg(qMax(1,e.column())):QString();
 return (where+e.description()).left(600);
}

extern "C" int t3_qml_preview_main(const char *payload, const char *wrapper) {
 int argc=1; char name[]="omarchy-t3-qml-preview"; char *argv[]={name,nullptr};
 QGuiApplication app(argc,argv);
 qInstallMessageHandler([](QtMsgType,const QMessageLogContext&,const QString&message){fprintf(stderr,"%s\n",qPrintable(message));});
 app.setApplicationName("omarchy-t3-qml-preview");
 auto data=QJsonDocument::fromJson(payload).object();
 QString frames=data["frameDirectory"].toString();
 if(frames.isEmpty()||!QFileInfo(frames).isDir())return 1;
 DocumentHost host(frames);
 // Links opened from the preview go nowhere.
 for(auto scheme:{"http","https","file","mailto","ftp","sftp","ssh","tel","sms","data","about","javascript","t3-thread","ot3","xdg","smb","nfs","git"})QDesktopServices::setUrlHandler(scheme,&host,"blockedUrl");

 // Omarchy colours for `omarchy.*`; the wrapper also sets them as the palette Controls inherit.
 auto vars=data["theme"].toObject()["variables"].toObject();
 auto color=[&vars](const char *key,const char *fallback){QColor c(vars[key].toString());return c.isValid()?c:QColor(fallback);};
 QColor bg=color("--background","#1e1e2e"),fg=color("--foreground","#cdd6f4"),accent=color("--accent","#89b4fa"),surface=color("--card","#313244"),muted=color("--muted-foreground","#a6adc8");
 bool dark=data["theme"].toObject()["appearance"].toString()!="light";
 QVariantMap theme{{"background",bg},{"foreground",fg},{"accent",accent},{"surface",surface},{"muted",muted},{"dark",dark},
  {"red",color("--destructive","#f38ba8")},{"green",color("--success","#a6e3a1")},{"yellow",color("--warning","#f9e2af")},{"blue",color("--info","#89b4fa")}};

 QUrl previewUrl=QUrl::fromLocalFile(frames+"/Preview.qml"),wrapperUrl=QUrl::fromLocalFile(QString::fromLocal8Bit(wrapper));
 QString imports=QLibraryInfo::path(QLibraryInfo::QmlImportsPath);
 QQmlEngine engine;
 BlockedNetworkFactory network;engine.setNetworkAccessManagerFactory(&network);
 engine.setImportPathList({mirrorImports(frames,imports)});
 // Plugins only next to an allowed qmldir.
 engine.setPluginPathList({"."});
 PreviewUrls urls(imports,{previewUrl,wrapperUrl},QUrl::fromLocalFile(frames+"/refused/refused"));engine.addUrlInterceptor(&urls);
 engine.rootContext()->setContextProperty("omarchy",theme);

 QQuickView view(&engine,nullptr);
 view.setColor(bg);view.setResizeMode(QQuickView::SizeRootObjectToView);
 view.resize(qBound(240,data["width"].toInt(900),2560),qBound(180,data["height"].toInt(620),1800));
 view.setSource(wrapperUrl);
 auto root=view.rootObject();
 if(!root){for(const auto &e:view.errors())fprintf(stderr,"%s\n",qPrintable(e.toString()));return 1;}
 auto labels=data["labels"].toObject();
 root->setProperty("title",labels["errors"].toString("QML errors"));
 auto stage=root->findChild<QQuickItem*>("stage");
 QStringList messages;
 auto show=[&](const QList<QQmlError> &errors,bool fatal){
  for(const auto &e:errors){if(e.url().isValid()&&e.url()!=previewUrl)continue;if(messages.size()<12)messages<<errorText(e);}
  root->setProperty("messages",messages);if(fatal)root->setProperty("failed",true);host.requestFrame();
 };
 // Runtime errors in the component (bad bindings, exceptions) join the list.
 QObject::connect(&engine,&QQmlEngine::warnings,&host,[&](const QList<QQmlError> &w){QList<QQmlError> own;for(const auto &e:w)if(e.url()==previewUrl)own<<e;if(!own.isEmpty())show(own,false);});

 // The component gets its own context: it sees `omarchy`, not the wrapper.
 QQmlContext context(engine.rootContext());
 QQmlComponent component(&engine);
 component.setData(data["contents"].toString().toUtf8(),previewUrl);
 QQuickItem *item=nullptr;
 if(component.isError())show(component.errors(),true);
 else{
  QObject *object=component.beginCreate(&context);
  item=qobject_cast<QQuickItem*>(object);
  if(!object)show(component.errors(),true);
  else if(!item){delete object;QQmlError e;e.setDescription(labels["rootItem"].toString("The root object must be an Item (not a Window)."));show({e},true);}
  else{item->setParentItem(stage);component.completeCreate();if(component.isError())show(component.errors(),false);}
 }
 // Fixed-size components are centred near the top; unsized ones fill the view.
 bool fill=item&&(item->width()<=0||item->height()<=0);
 auto layout=[&](){
  if(!item||!stage)return;
  if(fill){item->setSize(stage->size());}
  else if(item->width()!=stage->width()||item->height()!=stage->height()){item->setX(qMax(0.0,(stage->width()-item->width())/2));item->setY(16);}
  int extra=messages.isEmpty()?0:qMin(240,40+20*int(messages.size()));
  host.contentHeight(fill?qMax(360,int(item->implicitHeight())+extra):int(item->height())+32+extra);
 };
 if(item){
  QObject::connect(stage,&QQuickItem::widthChanged,&host,layout);QObject::connect(stage,&QQuickItem::heightChanged,&host,layout);
  if(!fill){QObject::connect(item,&QQuickItem::widthChanged,&host,layout);QObject::connect(item,&QQuickItem::heightChanged,&host,layout);}
  layout();
 }else host.contentHeight(qMin(480,80+24*int(messages.size())));

 view.show();
 host.attach(&view);
 fcntl(STDIN_FILENO,F_SETFL,fcntl(STDIN_FILENO,F_GETFL)|O_NONBLOCK);
 QSocketNotifier reader(STDIN_FILENO,QSocketNotifier::Read);
 QObject::connect(&reader,&QSocketNotifier::activated,&app,[&host](){host.readInput();});
 fprintf(stdout,"T3_DOCUMENT_READY\n");fflush(stdout);
 int code=app.exec();
 delete item;
 return code;
}

#include "document-host.moc"
