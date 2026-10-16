import QtQuick
import QtQuick.Window
import qs.Commons
import "../js/IconAssets.js" as Icons
Item {
  id: root
  property string name: "server"
  property color tint: Color.popups.text
  implicitWidth: Style.space(16)
  implicitHeight: implicitWidth
  Image {
    anchors.fill: parent
    opacity: root.tint.a
    source: "data:image/svg+xml," + encodeURIComponent(Icons.svg(root.name).replace(/currentColor/g, String(Qt.rgba(root.tint.r, root.tint.g, root.tint.b, 1))))
    sourceSize.width: Math.round(root.width * Screen.devicePixelRatio)
    sourceSize.height: Math.round(root.height * Screen.devicePixelRatio)
    fillMode: Image.PreserveAspectFit
    cache: true
  }
}
