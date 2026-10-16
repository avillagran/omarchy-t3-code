import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui

ColumnLayout {
  id: root
  required property var providers
  property bool collapsed: true
  property bool showHeader: true
  readonly property color fg: Color.popups.text
  readonly property color dim: Qt.rgba(fg.r, fg.g, fg.b, 0.60)
  readonly property color track: Qt.rgba(fg.r, fg.g, fg.b, 0.10)
  readonly property color line: Qt.rgba(fg.r, fg.g, fg.b, 0.13)
  readonly property color healthy: Color.accent
  function hasWindow(w) { return !!w && typeof w.availablePercent === "number" && isFinite(w.availablePercent) && Number(w.windowSeconds) > 0 }
  readonly property var visibleProviders: providers.filter(function(p) { return root.hasWindow(p.weekly) || root.hasWindow(p.shortWindow) })
  visible: visibleProviders.length > 0 && (showHeader || !collapsed || rows.opacity > 0)
  spacing: Style.space(5)
  RowLayout {
    visible:root.showHeader
    Layout.fillWidth: true
    Text { Layout.fillWidth: true; text: T3I18n.tr("Provider usage"); color: root.dim; font.pixelSize: Style.font.caption; font.letterSpacing: 1; font.bold: true }
    T3IconButton { name: root.collapsed ? "chevron-down" : "chevron-up"; tooltipText: T3I18n.tr("Toggle usage"); onClicked: root.collapsed = !root.collapsed }
  }
  ColumnLayout {
    id: rows
    Layout.fillWidth: true
    Layout.preferredHeight: root.collapsed ? 0 : implicitHeight
    opacity: root.collapsed ? 0 : 1
    visible: opacity > 0
    clip: true
    spacing: Style.space(4)
    Behavior on opacity { NumberAnimation { duration: 140 } }
    Behavior on Layout.preferredHeight { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
    Repeater {
      model: root.visibleProviders
      Rectangle {
        id: provider
        required property var modelData
        readonly property var weekly: root.hasWindow(modelData.weekly) ? modelData.weekly : null
        readonly property var shortWindow: root.hasWindow(modelData.shortWindow) ? modelData.shortWindow : null
        readonly property real available: weekly ? Number(weekly.availablePercent || 0) : -1
        readonly property real expected: Number(modelData.expectedAvailable)
        readonly property color pace: modelData.onPace ? root.healthy : Color.urgent
        Layout.fillWidth: true
        Layout.preferredHeight: weekly && shortWindow ? Style.space(47) : Style.space(30)
        color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.035)
        radius: Style.space(5); border.width: 1; border.color: root.line
        RowLayout {
          anchors.fill: parent; anchors.margins: Style.space(7); spacing: Style.space(8)
          Text { Layout.preferredWidth: Style.space(84); text: modelData.label; color: root.fg; font.bold: true; font.pixelSize: Style.font.caption; textFormat: Text.PlainText }
          ColumnLayout {
            Layout.fillWidth: true; spacing: Style.space(3)
            RowLayout {
              Layout.fillWidth: true; visible: !!provider.weekly; spacing: Style.space(6)
              Text { Layout.preferredWidth: Math.max(Style.space(82),implicitWidth); text: T3I18n.tr("Week {percent}%",{percent:Math.round(provider.available)}); color: provider.pace; font.bold: true; font.pixelSize: Style.font.caption }
              Rectangle {
                Layout.fillWidth: true; height: Style.space(8); radius: height / 2; color: root.track
                Rectangle {
                  width: parent.width * Math.max(0, Math.min(100, provider.available)) / 100
                  height: parent.height; radius: parent.radius; color: provider.pace
                  Behavior on width { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
                }
                Rectangle {
                  visible: provider.expected >= 0
                  x: Math.max(0, Math.min(parent.width - width, parent.width * provider.expected / 100 - width / 2))
                  y: -Style.space(2); width: Math.max(2, Style.space(1)); height: parent.height + Style.space(4); radius: 1; color: root.fg
                }
              }
              Text { Layout.preferredWidth: Math.max(Style.space(68),implicitWidth); text: provider.expected >= 0 ? T3I18n.tr("pace {percent}%",{percent:Math.round(provider.expected)}) : ""; color: root.dim; font.pixelSize: Math.max(8, Style.font.caption - 1) }
            }
            RowLayout {
              objectName: "quotaShort_" + modelData.provider
              Layout.fillWidth: true; visible: !!provider.shortWindow; spacing: Style.space(6)
              Text { Layout.preferredWidth: Math.max(Style.space(82),implicitWidth); text: provider.shortWindow ? T3I18n.tr("5H {percent}%",{percent:Math.round(provider.shortWindow.availablePercent)}) : ""; color: root.dim; font.pixelSize: Style.font.caption }
              Rectangle {
                Layout.fillWidth: true; height: Style.space(6); radius: height / 2; color: root.track
                Rectangle {
                  width: parent.width * Math.max(0, Math.min(100, provider.shortWindow ? provider.shortWindow.availablePercent : 0)) / 100
                  height: parent.height; radius: parent.radius; color: Color.accent
                  Behavior on width { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
                }
              }
              Item { Layout.preferredWidth: Style.space(68); height: 1 }
            }

          }
        }
      }
    }
  }
}
