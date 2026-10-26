import QtQuick 6.11
import QtQuick.Controls as Controls
import qs.Commons

// Theme-coloured text field. The stock Controls style paints a white box
// that ignores the Omarchy theme, so every free-form input uses this one.
Controls.TextField {
  id: root
  readonly property color fg: Color.popups.text
  color: fg
  placeholderTextColor: Qt.rgba(fg.r, fg.g, fg.b, 0.5)
  selectionColor: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.35)
  selectedTextColor: fg
  selectByMouse: true
  font.pixelSize: Style.font.caption
  leftPadding: Style.space(10); rightPadding: Style.space(10)
  topPadding: Style.space(6); bottomPadding: Style.space(6)
  background: Rectangle {
    radius: Style.space(6)
    color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.04)
    border.width: 1
    border.color: root.activeFocus ? Color.accent : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.14)
    Behavior on border.color { enabled:T3Motion.enabled;ColorAnimation { duration: T3Motion.duration(140)} }
  }
}
