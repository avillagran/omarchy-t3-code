#include "terminalsurface.h"
#include <QQmlExtensionPlugin>
#include <qqml.h>
class TerminalPlugin final : public QQmlExtensionPlugin {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID "org.qt-project.Qt.QQmlExtensionInterface/1.0")
public:
    void registerTypes(const char *uri) override { qmlRegisterType<TerminalSurface>(uri, 1, 0, "TerminalSurface"); }
};
#include "plugin.moc"
