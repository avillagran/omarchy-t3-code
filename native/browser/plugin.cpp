#include "clipboardplatform.h"
#include "browserinputitem.h"
#include <QQmlExtensionPlugin>
#include <qqml.h>
class BrowserPlatformPlugin final:public QQmlExtensionPlugin {
 Q_OBJECT
 Q_PLUGIN_METADATA(IID "org.qt-project.Qt.QQmlExtensionInterface/1.0")
public:
 void registerTypes(const char *uri)override{qmlRegisterType<ClipboardPlatform>(uri,1,0,"ClipboardPlatform");qmlRegisterType<BrowserInputItem>(uri,1,0,"BrowserInputItem");}
};
#include "plugin.moc"
