import QtQuick 6.11
import QtQuick.Controls as Controls
import QtQuick.Layouts
import qs.Commons
// The environments this app talks to, from the sidebar's machine button: each
// with its state and the one fix that applies (reconnect a remote host,
// restart the local backend when ot3 runs it). A host stuck "connected" in T3
// but not answering is reconnected from here without opening Preferences.
Controls.Popup {
 id:root
 objectName:"t3HostsPopup"
 required property var bridge
 signal settingsRequested()
 readonly property var environments:(bridge.state.environments || [])
 readonly property color fg:Color.popups.text
 readonly property color faint:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 // Reconnect requests in flight, by environment, until the state changes.
 property var reconnecting:({})
 // Restart (kill and start the server on its host): asked to confirm first, in the row.
 property string confirmRestart:""
 readonly property var store:bridge.connectionSettings || ({})
 readonly property string restarting:store.busy&&bridge.connectionSettingsCommand&&bridge.connectionSettingsCommand.action==="restart-server"?bridge.connectionSettingsCommand.environmentId || "":""
 property string restartError:""
 property string restartErrorFor:""
 function ssh(id){return (store.environments || []).some(function(e){return e.id===id&&(e.routes || []).some(function(r){return r.kind==="SshConnectionTarget"})})}
 function restart(id){confirmRestart="";restartError="";restartErrorFor=id;bridge.manageConnections("restart-server",{environmentId:id,confirmed:true})}
 Connections{target:root.bridge;function onConnectionsManaged(action,error){if(action==="restart-server"){root.restartError=error || "";if(!error){root.bridge.refresh()}}}}
 onOpened:{confirmRestart="";if(!store.busy)bridge.manageConnections("read")}
 function reconnect(id){
  var next=Object.assign({},reconnecting);next[id]=environments.find(function(e){return e.id===id}).state;reconnecting=next
  bridge.manageConnections("reconnect",{environmentId:id});bridge.refresh()
 }
 Connections{target:root.bridge;function onStateChanged(){
  var next={},changed=false
  Object.keys(root.reconnecting).forEach(function(id){var e=root.environments.find(function(x){return x.id===id});if(e&&e.state===root.reconnecting[id]&&e.state!=="ready")next[id]=root.reconnecting[id];else changed=true})
  if(changed)root.reconnecting=next
 }}
 // A reconnect that never changes the state stops spinning after a while.
 Timer{interval:45000;running:Object.keys(root.reconnecting).length>0;onTriggered:root.reconnecting=({})}
 width:Style.space(330)
 padding:Style.space(8)
 modal:false;focus:true
 closePolicy:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
 background:Rectangle{color:Color.popups.background;radius:Style.space(9);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.15)}
 contentItem:ColumnLayout {
  spacing:Style.space(2)
  Text{Layout.fillWidth:true;Layout.margins:Style.space(6);text:T3I18n.tr("Environments");color:root.faint;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  Repeater {
   model:root.environments
   RowLayout {
    id:row
    required property var modelData
    readonly property bool local:modelData.id==="local"
    readonly property bool ready:modelData.state==="ready"
    readonly property bool busy:root.reconnecting[modelData.id]!==undefined||root.restarting===modelData.id
    readonly property bool confirming:root.confirmRestart===modelData.id
    readonly property string version:root.bridge.serverVersion?root.bridge.serverVersion(modelData.id):""
    readonly property bool newer:!!version&&root.bridge.t3VersionNewer(version)
    objectName:"t3Host_"+modelData.id
    Layout.fillWidth:true;Layout.leftMargin:Style.space(6);Layout.rightMargin:Style.space(2);spacing:Style.space(10)
    T3EnvironmentIcon{kind:((root.bridge.catalogs || {})[row.modelData.id] || {}).machine || (row.local?"desktop":"server");implicitWidth:Style.space(16);tint:root.fg;opacity:0.75}
    ColumnLayout {
     Layout.fillWidth:true;spacing:Style.space(1)
     Text{Layout.fillWidth:true;text:row.modelData.name || row.modelData.id;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
     // The server's T3 version; newer than the one ot3 is validated with is marked.
     Text{objectName:"t3HostVersion_"+row.modelData.id;Layout.fillWidth:true;visible:!!row.version;text:"T3 "+row.version+(row.newer?" · "+T3I18n.tr("newer than ot3 knows"):"");color:row.newer?T3Palette.waiting:root.faint;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText
      HoverHandler{id:versionHover}
      Controls.ToolTip.visible:versionHover.hovered&&row.newer;Controls.ToolTip.delay:300
      Controls.ToolTip.text:T3I18n.tr("ot3 is validated with T3 Code {version}. Features added since then may be missing or behave differently.",{version:(root.bridge.validatedT3 || {}).tag || ""})}
     RowLayout{spacing:Style.space(5)
      Rectangle{width:Style.space(7);height:width;radius:width/2;color:row.ready?T3Palette.done:row.busy?T3Palette.working:Color.urgent}
      Text{Layout.fillWidth:true;text:root.restarting===row.modelData.id?T3I18n.tr("Restarting server…"):row.confirming?T3I18n.tr("Kill and restart T3 on this host? Runs there stop."):root.restartErrorFor===row.modelData.id&&root.restartError?T3I18n.error(root.restartError):row.busy?T3I18n.tr("Reconnecting…"):row.ready?T3I18n.tr("Connected"):T3I18n.error(row.modelData.state);color:row.confirming||(root.restartErrorFor===row.modelData.id&&root.restartError)?Color.urgent:row.ready||row.busy?root.faint:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:row.confirming?Text.Wrap:Text.NoWrap;elide:row.confirming?Text.ElideNone:Text.ElideRight;textFormat:Text.PlainText}
     }
    }
    WorkIndicator{visible:row.busy;active:visible&&root.visible;implicitWidth:Style.space(14);tint:root.faint}
    T3PickerButton{objectName:"t3HostRestartCancel_"+row.modelData.id;visible:row.confirming;primary:false;implicitHeight:Style.space(24);leftPadding:Style.space(8);pixelSize:Style.font.caption;text:T3I18n.tr("Cancel");onClicked:root.confirmRestart=""}
    T3PickerButton{objectName:"t3HostRestartConfirm_"+row.modelData.id;visible:row.confirming;primary:true;implicitHeight:Style.space(24);leftPadding:Style.space(8);pixelSize:Style.font.caption;text:T3I18n.tr("Restart");onClicked:root.restart(row.modelData.id)}
    T3IconButton{objectName:"t3HostRestart_"+row.modelData.id;visible:!row.local&&!row.busy&&!row.confirming&&root.ssh(row.modelData.id);name:"power";tooltipText:T3I18n.tr("Restart server: kill T3 on this host and start it again");enabled:!root.store.busy;onClicked:root.confirmRestart=row.modelData.id}
    T3IconButton{objectName:"t3HostReconnect_"+row.modelData.id;visible:!row.local&&!row.busy&&!row.confirming;name:"refresh-cw";tooltipText:T3I18n.tr("Reconnect");enabled:!root.bridge.connectionSettings.busy;onClicked:root.reconnect(row.modelData.id)}
    T3IconButton{objectName:"t3HostRestartLocal";visible:row.local&&root.bridge.localBackend.owner!=="gui"&&root.bridge.localBackend.running;name:"power";tooltipText:T3I18n.tr("Restart local backend");enabled:root.bridge.localBackend.canControl&&!root.bridge.backendStart.busy;onClicked:root.bridge.controlBackend("restart")}
    T3Icon{visible:row.local&&root.bridge.localBackend.owner==="gui";name:"monitor";implicitWidth:Style.space(14);tint:root.faint
     HoverHandler{id:guiHover}
     Controls.ToolTip.visible:guiHover.hovered;Controls.ToolTip.text:T3I18n.tr("Managed by T3 Code GUI")}
   }
  }
  Rectangle{Layout.fillWidth:true;Layout.topMargin:Style.space(4);implicitHeight:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.1)}
  Controls.ItemDelegate {
   objectName:"t3HostsSettings"
   Layout.fillWidth:true;implicitHeight:Style.space(32)
   background:Rectangle{radius:Style.space(6);color:parent.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.07):"transparent"}
   contentItem:RowLayout{spacing:Style.space(10)
    T3Icon{name:"settings";implicitWidth:Style.space(14);tint:root.faint}
    Text{Layout.fillWidth:true;text:T3I18n.tr("Connections");color:root.fg;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
   }
   onClicked:{root.close();root.settingsRequested()}
  }
 }
}
