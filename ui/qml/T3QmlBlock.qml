import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// A QML code block in a finished assistant message, shown as its preview in
// the block's place. The bar above switches to the code, copies it and opens
// the preview larger. With side-panel renders the block is a card for the
// Renders tab instead.
Rectangle {
  id: root
  objectName: "t3QmlBlock"
  required property var bridge
  required property var attachment
  property bool active: false
  property bool side: false
  property bool showCode: false
  property bool shownInSide: false
  signal codeToggled(bool show)
  signal sideRequested()
  readonly property color fg: Color.popups.text
  readonly property string code: String(attachment.source || "")
  readonly property var preview: previewLoader.item
  radius: Style.space(10)
  color: Qt.rgba(fg.r, fg.g, fg.b, 0.04)
  border.width: 1
  border.color: Qt.rgba(fg.r, fg.g, fg.b, 0.10)
  implicitHeight: column.implicitHeight + Style.space(2)
  ColumnLayout {
    id: column
    anchors { left: parent.left; right: parent.right; top: parent.top; margins: 1 }
    spacing: 0
    RowLayout {
      objectName: "t3QmlBlockBar"
      Layout.fillWidth: true; Layout.preferredHeight: Style.space(36)
      Layout.leftMargin: Style.space(12); Layout.rightMargin: Style.space(6); spacing: Style.space(4)
      T3Icon { name: "code"; tint: Color.accent; implicitWidth: Style.space(14) }
      Text {
        Layout.fillWidth: true; Layout.leftMargin: Style.space(4)
        text: T3I18n.tr("QML preview"); color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.8)
        font.pixelSize: Style.font.caption; elide: Text.ElideRight
      }
      T3PickerButton {
        objectName: "t3QmlBlockCode"; primary: false
        implicitHeight: Style.space(26); leftPadding: Style.space(10); rightPadding: Style.space(10)
        text: T3I18n.tr(root.showCode ? "Show preview" : "Show code")
        onClicked: root.codeToggled(!root.showCode)
      }
      T3CopyButton { objectName: "t3QmlBlockCopy"; bridge: root.bridge; copyText: root.code; tooltipText: T3I18n.tr("Copy code") }
      T3IconButton {
        objectName: "t3QmlBlockOpen"
        name: root.side ? "panel-right" : "external-link"
        tooltipText: T3I18n.tr(root.side ? "Show in side panel" : "Open in a window")
        onClicked: if (root.side) root.sideRequested(); else root.bridge.openReference("", root.attachment)
      }
    }
    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.08) }
    // Code, behind the button.
    Controls.TextArea {
      objectName: "t3QmlBlockSource"
      visible: root.showCode
      Layout.fillWidth: true
      text: root.code; textFormat: TextEdit.PlainText; readOnly: true; selectByMouse: true; wrapMode: TextEdit.Wrap
      color: root.fg; selectionColor: Color.accent; selectedTextColor: Color.popups.background
      font.family: "monospace"; font.pixelSize: Style.font.caption
      padding: Style.space(12)
      background: Item {}
    }
    // Side-panel renders: a card that opens the Renders tab.
    Rectangle {
      objectName: "t3QmlBlockSideCard"
      visible: !root.showCode && root.side
      Layout.fillWidth: true; Layout.preferredHeight: Style.space(44)
      color: cardHover.hovered ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.08) : "transparent"
      radius: Style.space(9)
      HoverHandler { id: cardHover; cursorShape: Qt.PointingHandCursor }
      TapHandler { onTapped: root.sideRequested() }
      RowLayout {
        anchors.fill: parent; anchors.leftMargin: Style.space(12); anchors.rightMargin: Style.space(12); spacing: Style.space(8)
        T3Icon { name: "panel-right"; tint: root.shownInSide ? Color.accent : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.7); implicitWidth: Style.space(14) }
        Text {
          Layout.fillWidth: true
          text: T3I18n.tr(root.shownInSide ? "Shown in the side panel" : "Show in side panel")
          color: root.shownInSide ? Color.accent : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.75); font.pixelSize: Style.font.caption
        }
      }
    }
    // The preview itself, in the conversation. The Loader only sets the width:
    // the page decides its height from the component's size.
    Item {
      visible: !root.showCode && !root.side
      Layout.fillWidth: true
      Layout.preferredHeight: previewLoader.item ? previewLoader.item.height : 0
      Loader {
        id: previewLoader
        width: parent.width
        active: !root.side
        sourceComponent: T3InlineHtml {
          objectName: "t3InlineQmlPreview"
          bridge: root.bridge; attachment: root.attachment; chromeVisible: false
          // Short until the renderer reports the component's height.
          pageHeight: Style.space(260)
          active: root.active && !root.showCode
        }
      }
    }
  }
}
