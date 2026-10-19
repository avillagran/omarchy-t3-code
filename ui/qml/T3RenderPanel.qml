import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons

// Side panel for HTML renders (Preferences > HTML renders > Side panel). Uses
// the same out-of-process Qt renderer as inline renders; one session at a
// time, reopened whenever the shown render changes.
Rectangle {
  id: root
  objectName: "t3RenderPanel"
  required property var bridge
  property var attachment: null
  signal closed()
  property string sessionId: ""
  property string sessionFor: ""
  color: Qt.rgba(Color.popups.background.r, Color.popups.background.g, Color.popups.background.b, 0.55)
  function sync() {
    var want = visible && attachment ? attachment.id : ""
    if (sessionId && sessionFor !== want) { bridge.closeInlineDocument(sessionId); sessionId = ""; sessionFor = "" }
    if (want && !sessionId) { sessionId = bridge.openInlineDocument(attachment); sessionFor = want }
  }
  onAttachmentChanged: sync()
  onVisibleChanged: sync()
  Component.onCompleted: sync()
  Component.onDestruction: if (sessionId) bridge.closeInlineDocument(sessionId)
  ColumnLayout {
    anchors.fill: parent; anchors.margins: Style.space(12); spacing: Style.space(8)
    RowLayout {
      Layout.fillWidth: true; spacing: Style.space(6)
      T3Icon { name: "file"; tint: Color.accent; implicitWidth: Style.space(14) }
      Text {
        objectName: "t3RenderPanelTitle"; Layout.fillWidth: true
        text: String((root.attachment || {}).name || "").replace(/\.html$/i, "")
        color: Color.popups.text; textFormat: Text.PlainText; elide: Text.ElideMiddle; font.pixelSize: Style.font.body
      }
      T3IconButton { objectName: "t3RenderPanelWindow"; name: "external-link"; tooltipText: T3I18n.tr("Open in a window"); onClicked: root.bridge.openReference("", root.attachment) }
      T3IconButton { objectName: "t3RenderPanelClose"; name: "x"; tooltipText: T3I18n.tr("Close"); onClicked: root.closed() }
    }
    T3DocumentView {
      Layout.fillWidth: true; Layout.fillHeight: true
      bridge: root.bridge; sessionId: root.sessionId; inlineMode: true; chromeVisible: false; ownsWheel: true
      fallbackTitle: (root.attachment || {}).name || "HTML"
      visible: !!root.sessionId
      onClosed: root.closed()
    }
  }
}
