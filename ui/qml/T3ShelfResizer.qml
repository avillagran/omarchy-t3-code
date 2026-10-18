import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons

// Drag handle on top of a sidebar shelf: dragging up grows the shelf below
// it, like the sidebar's own width handle. Arrows step it, double click
// restores the default.
Item {
  id: root
  property real value: 220
  property real defaultValue: 220
  property string label: ""
  signal resized(real height)
  Layout.fillWidth: true
  Layout.preferredHeight: Style.space(6)
  activeFocusOnTab: true
  Accessible.role: Accessible.Slider
  Accessible.name: T3I18n.tr(label)
  Keys.onUpPressed: root.resized(root.value + 20)
  Keys.onDownPressed: root.resized(root.value - 20)
  readonly property bool active: drag.containsMouse || drag.pressed || activeFocus
  Rectangle {
    anchors.centerIn: parent
    width: parent.width - Style.space(24)
    height: root.active ? Style.space(2) : 1
    radius: height / 2
    color: root.active ? Color.accent : "transparent"
  }
  MouseArea {
    id: drag
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.SizeVerCursor
    property real startY: 0
    property real startValue: 0
    onPressed: function(mouse) { startY = mapToGlobal(mouse.x, mouse.y).y; startValue = root.value }
    onPositionChanged: function(mouse) { if (pressed) root.resized(startValue - (mapToGlobal(mouse.x, mouse.y).y - startY) / Style.space(1)) }
    onDoubleClicked: root.resized(root.defaultValue)
  }
}
