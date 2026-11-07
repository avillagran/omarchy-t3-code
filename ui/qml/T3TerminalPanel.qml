import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Window
import QtQuick.Controls as Controls
import qs.Commons

ColumnLayout {
  id: root
  objectName: "t3TerminalPanel"
  property string metadataLeaseId: ""
  required property var bridge
  readonly property color fg: (Color.popups ? Color.popups.text : Color.foreground)
  readonly property var layout: bridge.terminalLayout
  readonly property var tab: bridge.terminalTab()
  readonly property bool terminalFocused: {var item=root.Window.activeFocusItem;while(item){if(item===root)return true;item=item.parent}return false}
  spacing: 0
  Component.onCompleted: if (!layout.tabs.length && bridge.canOpenTerminal) bridge.newTerminal(false)
  RowLayout {
    Layout.fillWidth: true; Layout.margins: Style.space(5); spacing: Style.space(3)
    Controls.ScrollView {
      Layout.fillWidth: true; Layout.preferredHeight: Style.space(30)
      contentWidth: tabButtons.implicitWidth; contentHeight: height; clip: true
      Row {
        id: tabButtons; spacing: Style.space(4)
        Repeater {
          model: root.layout.tabs
          T3PickerButton {
            required property var modelData
            required property int index
            objectName: "t3TerminalTab_" + index
            primary: root.layout.active===modelData.id
            text: T3I18n.tr("Terminal") + " " + (index+1)
            onClicked: root.bridge.activateTerminalTab(modelData.id)
          }
        }
      }
    }
    T3IconButton { objectName: "t3TerminalNew"; name: "plus"; tooltipText: T3I18n.tr("New terminal"); enabled: root.bridge.canOpenTerminal; onClicked: root.bridge.newTerminal(false) }
    T3IconButton { objectName: "t3TerminalSplit"; name: "columns-2"; tooltipText: T3I18n.tr("Split terminal"); enabled: root.bridge.canOpenTerminal && !!root.tab && root.tab.terminalIds.length<8; onClicked: root.bridge.newTerminal(true) }
    T3IconButton { objectName: "t3TerminalSplitVertical"; name: "rows-2"; tooltipText: T3I18n.tr("Split terminal vertically"); enabled: root.bridge.canOpenTerminal && !!root.tab && root.tab.terminalIds.length<8; onClicked: root.bridge.newTerminal(true,undefined,true) }
  }
  Text {
    visible: !root.tab; Layout.fillWidth: true; Layout.margins: Style.space(12)
    text: T3I18n.tr("Open a terminal to run commands in this conversation.")
    color: root.fg; wrapMode: Text.Wrap; font.pixelSize: Style.font.caption
  }
  Controls.SplitView {
    id: panes; objectName: "t3TerminalPanes"
    Layout.fillWidth: true; Layout.fillHeight: true
    orientation: root.tab && root.tab.vertical ? Qt.Vertical : Qt.Horizontal
    Repeater {
      id: terminalPanes
      model: root.tab ? root.tab.terminalIds : []
      T3TerminalPane {
        required property string modelData
        bridge: root.bridge; metadataLeaseId: root.metadataLeaseId; terminalId: modelData
        current: !!root.tab && root.tab.activeTerminalId===modelData
        Controls.SplitView.fillWidth: true; Controls.SplitView.fillHeight: true
        Controls.SplitView.minimumWidth: Style.space(60); Controls.SplitView.minimumHeight: Style.space(60)
      }
    }
  }
}
