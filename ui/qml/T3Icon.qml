import QtQuick 6.11
import QtQuick.Window 6.11
import qs.Commons
import "../js/IconAssets.js" as Icons
Item {
  id: root
  readonly property real displayScale: Screen.devicePixelRatio
  property real rasterScale: 1
  function updateRasterScale(){rasterScale=Math.max(0.5,displayScale || 1)}
  onDisplayScaleChanged:Qt.callLater(root.updateRasterScale)
  Component.onCompleted:Qt.callLater(root.updateRasterScale)
  property string name: "server"
  property color tint: Color.popups.text
  implicitWidth: Style.space(16)
  implicitHeight: implicitWidth
  Image {
    anchors.fill: parent
    opacity: root.tint.a
    source: "data:image/svg+xml," + encodeURIComponent(Icons.svg(root.name).replace(/currentColor/g, String(Qt.rgba(root.tint.r, root.tint.g, root.tint.b, 1))))
    sourceSize.width: Math.round(root.width * root.rasterScale)
    sourceSize.height: Math.round(root.height * root.rasterScale)
    fillMode: Image.PreserveAspectFit
    cache: true
  }
}
