import QtQuick 6.11
import QtQuick.Controls as Controls
import QtQuick.Layouts
import qs.Commons
import "../js/ConnectionStatus.js" as Status
import "../js/HostStyle.js" as HostStyle
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
 // The host whose look (icon, colour, name on threads) is being edited.
 property string styling:""
 readonly property var hostStyles:bridge.viewState.hostStyles || ({})
 function restyle(id,patch){bridge.rememberView({hostStyles:HostStyle.update(hostStyles,id,patch)})}
 property string restartErrorFor:""
 function ssh(id){return (store.environments || []).some(function(e){return e.id===id&&(e.routes || []).some(function(r){return r.kind==="SshConnectionTarget"})})}
 function restart(id){confirmRestart="";restartError="";restartErrorFor=id;bridge.manageConnections("restart-server",{environmentId:id,confirmed:true})}
 Connections{target:root.bridge;function onConnectionsManaged(action,error){if(action==="restart-server"){root.restartError=error || "";if(!error){root.bridge.refresh()}}}}
 onOpened:{confirmRestart="";styling="";if(!store.busy)bridge.manageConnections("read")}
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
 width:Style.space(390)
 padding:Style.space(8)
 modal:false;focus:true
 closePolicy:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
 background:Rectangle{color:Color.popups.background;radius:Style.space(9);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.15)}
 contentItem:ColumnLayout {
  spacing:Style.space(2)
  Text{Layout.fillWidth:true;Layout.margins:Style.space(6);text:T3I18n.tr("Environments");color:root.faint;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  Repeater {
   model:root.environments
   ColumnLayout {
   id:hostBlock
   required property var modelData
   readonly property var look:HostStyle.of(root.hostStyles,modelData.id,((root.bridge.catalogs || {})[modelData.id] || {}).machine)
   Layout.fillWidth:true;spacing:Style.space(4)
   RowLayout {
    id:row
    readonly property var modelData:hostBlock.modelData
    readonly property bool local:modelData.id==="local"
    readonly property bool ready:modelData.state==="ready"
    readonly property bool busy:root.reconnecting[modelData.id]!==undefined||root.restarting===modelData.id
    readonly property bool confirming:root.confirmRestart===modelData.id
    readonly property string version:root.bridge.serverVersion?root.bridge.serverVersion(modelData.id):""
    readonly property bool newer:!!version&&root.bridge.t3VersionNewer(version)
    objectName:"t3Host_"+modelData.id
    Layout.fillWidth:true;Layout.leftMargin:Style.space(6);Layout.rightMargin:Style.space(2);spacing:Style.space(10)
    // The icon is the button that edits how this host is marked on threads.
    Controls.AbstractButton {
     objectName:"t3HostStyle_"+row.modelData.id
     implicitWidth:Style.space(28);implicitHeight:Style.space(28)
     hoverEnabled:true;Accessible.name:T3I18n.tr("Icon and colour for {host}",{host:row.modelData.name || row.modelData.id})
     Controls.ToolTip.visible:hovered;Controls.ToolTip.delay:400;Controls.ToolTip.text:Accessible.name
     onClicked:root.styling=root.styling===row.modelData.id?"":row.modelData.id
     background:Rectangle{radius:Style.space(7);color:hostBlock.look.color?Qt.rgba(Qt.color(hostBlock.look.color).r,Qt.color(hostBlock.look.color).g,Qt.color(hostBlock.look.color).b,0.16):parent.hovered||root.styling===row.modelData.id?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08):"transparent";border.width:root.styling===row.modelData.id?1:0;border.color:Color.accent}
     contentItem:Item{T3Icon{anchors.centerIn:parent;name:hostBlock.look.icon;implicitWidth:Style.space(16);tint:hostBlock.look.color || root.fg;opacity:hostBlock.look.color?1:0.75}}
    }
    ColumnLayout {
     Layout.fillWidth:true;spacing:Style.space(1)
     Text{Layout.fillWidth:true;text:row.modelData.name || row.modelData.id;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
     // The server's T3 version; newer than the one ot3 is validated with is marked.
     Text{objectName:"t3HostVersion_"+row.modelData.id;Layout.fillWidth:true;visible:!!row.version;text:"T3 "+row.version+(row.newer?" · "+T3I18n.tr("newer than O3 knows"):"");color:row.newer?T3Palette.waiting:root.faint;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText
      HoverHandler{id:versionHover}
      Controls.ToolTip.visible:versionHover.hovered&&row.newer;Controls.ToolTip.delay:300
      Controls.ToolTip.text:T3I18n.tr("O3 is validated with T3 Code {version}. Features added since then may be missing or behave differently.",{version:(root.bridge.validatedT3 || {}).tag || ""})}
     RowLayout{spacing:Style.space(5)
      Rectangle{width:Style.space(7);height:width;radius:width/2;color:row.ready?T3Palette.done:row.busy?T3Palette.working:Color.urgent}
      Text{Layout.fillWidth:true;text:root.restarting===row.modelData.id?T3I18n.tr("Restarting server…"):row.confirming?T3I18n.tr("Kill and restart T3 on this host? Runs there stop."):root.restartErrorFor===row.modelData.id&&root.restartError?T3I18n.error(root.restartError):row.busy?T3I18n.tr("Reconnecting…"):row.ready?T3I18n.tr("Connected"):T3I18n.error(row.modelData.state);color:row.confirming||(root.restartErrorFor===row.modelData.id&&root.restartError)?Color.urgent:row.ready||row.busy?root.faint:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:row.confirming?Text.Wrap:Text.NoWrap;elide:row.confirming?Text.ElideNone:Text.ElideRight;textFormat:Text.PlainText}
     }
     Text{objectName:"t3HostFailure_"+row.modelData.id;Layout.fillWidth:true;visible:!row.ready;text:(Status.reason(row.modelData.state)?T3I18n.tr(Status.reason(row.modelData.state)):T3I18n.error(row.modelData.state))+" ("+row.modelData.state+")";color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
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
   // Icon, colour (none by default) and whether threads show the host's name.
   ColumnLayout {
    objectName:"t3HostStyleEditor_"+hostBlock.modelData.id
    visible:root.styling===hostBlock.modelData.id
    Layout.fillWidth:true;Layout.leftMargin:Style.space(44);Layout.rightMargin:Style.space(8);Layout.bottomMargin:Style.space(6);spacing:Style.space(6)
    Flow {
     Layout.fillWidth:true;spacing:Style.space(4)
     Repeater {
      model:HostStyle.KINDS
      T3IconButton{required property string modelData;objectName:"t3HostIcon_"+modelData;name:HostStyle.ICONS[modelData];implicitWidth:Style.space(26);implicitHeight:Style.space(26);tint:hostBlock.look.kind===modelData?Color.accent:root.faint;tooltipText:T3I18n.tr(({server:"Server",cloud:"Cloud VM",linux:"Linux/WSL",desktop:"Desktop",laptop:"Laptop","mac-mini":"Mini PC","mac-studio":"Workstation"})[modelData]);onClicked:root.restyle(hostBlock.modelData.id,{icon:modelData})}
     }
    }
    Flow {
     Layout.fillWidth:true;spacing:Style.space(6)
     Repeater {
      model:[""].concat(HostStyle.COLORS)
      Controls.AbstractButton {
       required property string modelData
       objectName:"t3HostColor_"+(modelData || "none")
       implicitWidth:Style.space(20);implicitHeight:Style.space(20)
       Accessible.name:modelData?modelData:T3I18n.tr("No colour")
       hoverEnabled:true;Controls.ToolTip.visible:hovered&&!modelData;Controls.ToolTip.delay:400;Controls.ToolTip.text:Accessible.name
       onClicked:root.restyle(hostBlock.modelData.id,{color:modelData})
       background:Rectangle{radius:width/2;color:parent.modelData || "transparent";border.width:hostBlock.look.color===parent.modelData?2:1;border.color:hostBlock.look.color===parent.modelData?root.fg:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.3)
        Rectangle{visible:!parent.parent.modelData;anchors.centerIn:parent;width:parent.width*0.7;height:1.5;rotation:-45;color:root.faint}}
      }
     }
    }
    Controls.CheckBox {
     objectName:"t3HostShowName_"+hostBlock.modelData.id
     checked:hostBlock.look.showName
     text:T3I18n.tr("Show the host's name on threads");palette.windowText:root.fg;palette.text:root.fg
     onToggled:root.restyle(hostBlock.modelData.id,{showName:checked})
    }
   }
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
