import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

Rectangle {
  id: root
  objectName: "t3TerminalPane_" + terminalId
  required property var bridge
  required property string terminalId
  property bool current: false
  property string requestId: ""
  property string attachedKey: ""
  property bool applyingGrid: false
  readonly property var view: bridge.terminalViews[requestId] || {status:"connecting",error:""}
  readonly property bool terminalFocused: !!terminal.item && terminal.item.activeFocus
  readonly property color fg: (Color.popups ? Color.popups.text : Color.foreground)
  color: (Color.popups ? Color.popups.background : Color.background)
  border.width: current ? 1 : 0; border.color: Color.accent
  function attach(restart){
    if (!terminal.item || !bridge.selectedKey) return
    detach();attachedKey=bridge.selectedKey;terminal.item.reset()
    requestId=bridge.attachTerminal(terminalId,terminal.item.columns,terminal.item.rows,!!restart)
    if(current)terminal.item.forceActiveFocus()
  }
  function detach(){if(requestId){bridge.detachTerminal(requestId);requestId=""}}
  function paste(){clipboard.text="";clipboard.paste();if(terminal.item&&clipboard.text)terminal.item.paste(clipboard.text);clipboard.text=""}
  onCurrentChanged: if(current&&terminal.item)terminal.item.forceActiveFocus()
  Component.onDestruction: detach()
  Connections {
    target: root.bridge; ignoreUnknownSignals: true
    function onSelectedKeyChanged(){Qt.callLater(root.attach,false)}
    function onTerminalViewEvent(request,data){
      if(request!==root.requestId || root.attachedKey!==root.bridge.selectedKey || !terminal.item)return
      if(data.kind==="event"){
        if(data.reset)terminal.item.reset()
        var event=data.event || {},grid=event.snapshot || event
        if(grid.cols>0&&grid.rows>0){root.applyingGrid=true;terminal.item.setGrid(grid.cols,grid.rows);root.applyingGrid=false}
        if(data.data)terminal.item.feed(data.data)
      }
    }
  }
  ColumnLayout {
    anchors.fill: parent; anchors.margins: 1; spacing: 0
    RowLayout {
      Layout.fillWidth: true; Layout.leftMargin: Style.space(5); spacing: Style.space(2)
      Text { Layout.fillWidth: true; text: root.view.label || T3I18n.tr("Terminal"); textFormat: Text.PlainText; elide: Text.ElideRight; color: root.fg; font.pixelSize: Style.font.caption }
      Text { visible: root.view.status!=="running"; text: root.view.status==="exited"?T3I18n.tr("Exited"):root.view.status==="reconnecting"?T3I18n.tr("Reconnecting…"):root.view.status==="error"?T3I18n.tr("Error"):T3I18n.tr("Connecting…"); color: Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.6); font.pixelSize: Style.font.caption }
      T3IconButton { objectName: "t3TerminalCopy"; name: "copy"; tooltipText: T3I18n.tr("Copy selection"); enabled: !!terminal.item && terminal.item.hasSelection; onClicked: terminal.item.copySelection() }
      T3IconButton { objectName: "t3TerminalPaste"; name: "clipboard"; tooltipText: T3I18n.tr("Paste"); enabled: !!root.requestId && root.bridge.canOpenTerminal; onClicked: root.paste() }
      T3IconButton { objectName: "t3TerminalClear"; name: "eraser"; tooltipText: T3I18n.tr("Clear terminal"); enabled: !!root.requestId && root.bridge.canOpenTerminal; onClicked: root.bridge.terminalInput(root.requestId,"clear") }
      T3IconButton { objectName: "t3TerminalRestart"; name: "rotate-cw"; tooltipText: T3I18n.tr("Restart terminal"); enabled: root.bridge.canOpenTerminal; visible: root.view.status==="exited" || root.view.status==="error"; onClicked: root.attach(true) }
      T3IconButton { objectName: "t3TerminalClose"; name: "x"; tooltipText: T3I18n.tr("Close terminal"); enabled: !!root.requestId && root.bridge.canOpenTerminal; onClicked: root.bridge.closeTerminal(root.terminalId) }
    }
    Text {
      Layout.fillWidth: true; Layout.margins: Style.space(5); visible: !!root.view.error
      text: T3I18n.error(root.view.error || ""); color: Color.urgent; wrapMode: Text.Wrap; font.pixelSize: Style.font.caption
    }
    Loader {
      id: terminal; objectName: "t3TerminalViewportLoader"
      Layout.fillWidth: true; Layout.fillHeight: true
      source: Qt.resolvedUrl("T3TerminalViewport.qml")
      onLoaded: root.attach(false)
    }
    Text {
      Layout.fillWidth: true; Layout.margins: Style.space(12); visible: terminal.status===Loader.Error
      text: T3I18n.tr("The terminal renderer is unavailable. Rebuild the desktop client.")
      color: Color.urgent; wrapMode: Text.Wrap; font.pixelSize: Style.font.caption
    }
  }
  Connections {
    target: terminal.item; ignoreUnknownSignals: true
    function onInput(data){if(root.requestId && root.attachedKey===root.bridge.selectedKey)root.bridge.terminalInput(root.requestId,"write",{data:data})}
    function onGridChanged(columns,rows){if(root.requestId&&!root.applyingGrid)root.bridge.terminalInput(root.requestId,"resize",{columns:columns,rows:rows})}
    function onCopyRequested(text){root.bridge.copyText(text)}
    function onPasteRequested(){root.paste()}
    function onLinkActivated(uri){root.bridge.openReference(uri)}
    function onActiveFocusChanged(){if(terminal.item.activeFocus)root.bridge.activateTerminal(root.terminalId)}
  }
  Controls.TextArea { id: clipboard; visible: false }
}
