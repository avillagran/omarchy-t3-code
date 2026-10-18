import QtQuick 6.11
import qs.Commons

Item {
  id: root
  property bool active: false
  property color tint: Color.accent
  property int step: 0
  implicitWidth: Style.space(12)
  implicitHeight: implicitWidth
  Rectangle {
    anchors.centerIn: parent
    width: parent.width - 2; height: width; radius: width / 2
    color: "transparent"; border.width: 1; border.color: Qt.rgba(root.tint.r, root.tint.g, root.tint.b, 0.30)
    rotation: root.step * 45
    Rectangle { anchors.horizontalCenter: parent.horizontalCenter; y: -1; width: Style.space(4); height: width; radius: width / 2; color: root.tint }
  }
  Timer { interval: 200; running: root.active && root.visible; repeat: true; onTriggered: root.step = (root.step + 1) % 8 }
}
