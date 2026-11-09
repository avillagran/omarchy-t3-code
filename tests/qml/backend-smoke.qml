import QtQuick
import Quickshell
import "UnderTest/ui/qml" 1.0 as Test
ShellRoot {
  Component.onCompleted: {
    console.log("BACKEND_SMOKE",Test.T3Backend)
    if (!Test.T3Backend) {console.error("QML_FAIL backend singleton undefined");Qt.exit(1);return}
    console.log("BACKEND_SMOKE_QML_PASS",Test.T3Backend.selectedKey)
    Qt.callLater(Qt.quit)
  }
}
