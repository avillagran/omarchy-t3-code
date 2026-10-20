import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons

// Multi-purpose side panel next to the chat. Each tab is its own tool:
// the preview browser, HTML renders made in this chat and device screens.
// Only the visible tab is loaded.
Rectangle {
  id: root
  objectName: "t3SidePanel"
  required property var bridge
  required property Component browserComponent
  property string tab: "browser"
  property var renders: []
  property var renderAttachment: null
  signal tabRequested(string tab)
  signal renderRequested(var attachment)
  signal closed()
  signal integrationsRequested()
  readonly property var browserItem: tab === "browser" ? content.item : null
  readonly property color fg: Color.popups.text
  // Opaque so the narrow-window sheet covers the chat.
  color: Color.popups.background
  readonly property var tabs: [
    {id: "browser", icon: "globe", label: "Browser"},
    {id: "renders", icon: "code", label: "Renders"},
    {id: "devices", icon: "smartphone", label: "Devices"}
  ]
  ColumnLayout {
    anchors.fill: parent; spacing: 0
    RowLayout {
      objectName: "t3SidePanelTabs"
      Layout.fillWidth: true; Layout.preferredHeight: Style.space(38)
      Layout.leftMargin: Style.space(8); Layout.rightMargin: Style.space(6); spacing: Style.space(2)
      Repeater {
        model: root.tabs
        Rectangle {
          required property var modelData
          readonly property bool current: root.tab === modelData.id
          objectName: "t3SideTab_" + modelData.id
          Layout.preferredHeight: Style.space(28); Layout.preferredWidth: tabRow.implicitWidth + Style.space(16)
          radius: Style.space(6)
          color: current ? Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.14) : tabHover.hovered ? Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.06) : "transparent"
          Accessible.role: Accessible.PageTab; Accessible.name: T3I18n.tr(modelData.label)
          HoverHandler { id: tabHover }
          TapHandler { onTapped: root.tabRequested(modelData.id) }
          RowLayout {
            id: tabRow; anchors.centerIn: parent; spacing: Style.space(6)
            T3Icon { name: modelData.icon; tint: current ? Color.accent : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.65); implicitWidth: Style.space(14) }
            Text { text: T3I18n.tr(modelData.label); color: current ? Color.accent : Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.75); font.pixelSize: Style.font.caption }
          }
        }
      }
      Item { Layout.fillWidth: true }
      T3IconButton { objectName: "t3SidePanelClose"; name: "x"; tooltipText: T3I18n.tr("Close side panel"); onClicked: root.closed() }
    }
    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.08) }
    Loader {
      id: content
      objectName: "t3SidePanelContent"
      Layout.fillWidth: true; Layout.fillHeight: true
      sourceComponent: root.tab === "browser" ? root.browserComponent : root.tab === "renders" ? rendersTab : devicesTab
    }
  }
  Component {
    id: rendersTab
    ColumnLayout {
      spacing: 0
      // Renders made in this chat, newest first; the panel shows one.
      Flow {
        Layout.fillWidth: true; Layout.margins: Style.space(10); spacing: Style.space(6)
        visible: root.renders.length > 1
        Repeater {
          model: root.renders.slice().reverse()
          T3FileReference {
            required property var modelData
            objectName: "t3SideRender"
            fileName: String(modelData.name || "HTML").replace(/\.html$/i, "")
            border.width: root.renderAttachment && root.renderAttachment.id === modelData.id ? 2 : 1
            onClicked: root.renderRequested(modelData)
          }
        }
      }
      Text {
        Layout.fillWidth: true; Layout.margins: Style.space(14); wrapMode: Text.Wrap
        visible: !root.renderAttachment
        text: T3I18n.tr("Pages the agent renders in this chat appear here."); color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.6); font.pixelSize: Style.font.caption
      }
      T3RenderPanel {
        Layout.fillWidth: true; Layout.fillHeight: true
        visible: !!root.renderAttachment
        bridge: root.bridge; attachment: root.renderAttachment
        onClosed: root.closed()
      }
    }
  }
  Component {
    id: devicesTab
    T3DevicePanel { bridge: root.bridge; onIntegrationsRequested: root.integrationsRequested() }
  }
}
