import QtQuick
import QtQuick.Controls as Controls
import qs.Commons
Rectangle {
  id: root
  property string name: "server"
  property string tooltipText: ""
  property color tint: Color.popups.text
  signal clicked()
  activeFocusOnTab: true
  Accessible.role: Accessible.Button
  Accessible.name: tooltipText || name
  Keys.onReturnPressed: if (enabled) clicked()
  Keys.onEnterPressed: if (enabled) clicked()
  Keys.onSpacePressed: if (enabled) clicked()
  border.width: activeFocus ? 1 : 0
  border.color: Color.accent
  implicitWidth: Style.space(28); implicitHeight: implicitWidth
  radius: Style.space(6)
  color: mouse.containsMouse && enabled ? Qt.rgba(Color.popups.text.r, Color.popups.text.g, Color.popups.text.b, 0.07) : "transparent"
  opacity: enabled ? 1 : 0.3
  Behavior on color { ColorAnimation { duration: 130 } }
  T3Icon { anchors.centerIn: parent; name: root.name; tint: root.tint }
  MouseArea { id: mouse; anchors.fill: parent; hoverEnabled: true; cursorShape: root.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor; onClicked: root.clicked() }
  Controls.ToolTip.visible: mouse.containsMouse && root.tooltipText.length > 0
  Controls.ToolTip.text: tooltipText
  Controls.ToolTip.delay: 500
}
