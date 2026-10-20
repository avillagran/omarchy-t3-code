import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
ColumnLayout {
 id:root
 required property var bridge
 property string environment:bridge.selectedEnvironment || "local"
 readonly property var store:bridge.connectionSettings || ({busy:false,error:"",environments:[]})
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 property var selected:null
 property var route:null
 property string form:""
 property string kind:"ssh"
 function run(action,values){bridge.manageConnections(action,values || {})}
 function edit(environment,route){
  selected=environment || null;root.route=route || null;form="route";kind=route&&route.kind==="BearerConnectionTarget"?"bearer":"ssh"
  label.text=environment?environment.name:"";host.text=route&&route.ssh?route.ssh.alias || route.ssh.hostname || "":"";username.text=route&&route.ssh?route.ssh.username || "":"";port.text=route&&route.ssh&&route.ssh.port?String(route.ssh.port):"";endpoint.text=route?route.url || "":"";code.text="";editor.open()
 }
 function rename(environment){selected=environment;form="rename";label.text=environment.name;editor.open()}
 function remove(environment,route){selected=environment;root.route=route || null;confirm.open()}
 function save(){
  if(form==="rename"){run("rename",{environmentId:selected.id,label:label.text});return}
  var values={kind:kind,environmentId:selected?selected.id:"",connectionId:route?route.connectionId || "":"",label:label.text}
  if(kind==="ssh")values.ssh={alias:host.text.trim(),hostname:host.text.trim(),username:username.text.trim() || null,port:port.text?Number(port.text):null}
  else {values.url=endpoint.text;values.pairingCode=code.text}
  run("save-route",values)
 }
 spacing:Style.space(14)
 Component.onCompleted:run("read")
 Text{Layout.fillWidth:true;Layout.margins:Style.space(12);visible:!!text;text:root.store.error?T3I18n.error(root.store.error):root.store.busy?T3I18n.tr("Loading…"):"";color:root.store.error?Color.urgent:root.muted;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
 Loader{Layout.fillWidth:true;active:root.environment!=="local";visible:active;sourceComponent:Component{T3PreferenceSection{title:"Local environment";icon:"laptop";body:Component{T3LocalConnection{width:parent.width;bridge:root.bridge}}}}}
 T3ConnectionAccess{Layout.fillWidth:true;bridge:root.bridge;environment:root.environment}
 T3ConnectionCloud{Layout.fillWidth:true;bridge:root.bridge;environment:root.environment}
 T3PickerButton{primary:false;Layout.alignment:Qt.AlignRight;visible:root.environment==="local"&&!!((root.bridge.connectionCloud || {}).status || {}).desired&&!!(root.bridge.localBackend || {}).canControl;text:T3I18n.tr("Restart local backend");enabled:!(root.bridge.backendStart || {}).busy;onClicked:cloudRestartConfirm.open()}
 Controls.Popup{id:cloudRestartConfirm;parent:Controls.Overlay.overlay;anchors.centerIn:parent;modal:true;width:Math.min(Style.space(500),parent?parent.width-Style.space(32):Style.space(500));padding:Style.space(20)
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
  contentItem:ColumnLayout{spacing:Style.space(12)
   Text{Layout.fillWidth:true;text:T3I18n.tr("Apply and restart T3 Code?");color:root.fg;font.bold:true;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;text:T3I18n.tr("T3 Code will restart its local server to apply the network settings. Your threads and drafts are kept.");color:root.muted;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}T3PickerButton{primary:false;text:T3I18n.tr("Cancel");onClicked:cloudRestartConfirm.close()}T3PickerButton{text:T3I18n.tr("Apply and restart");onClicked:{cloudRestartConfirm.close();root.bridge.controlBackend("restart")}}}
  }
 }

 T3PreferenceSection{Layout.fillWidth:true;title:"Environments";icon:"server"
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(8)
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(12)
    Text{Layout.fillWidth:true;text:T3I18n.tr("Environments");color:root.fg;font.pixelSize:Style.font.body}
    T3IconButton{name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");enabled:!root.store.busy;onClicked:root.run("read")}
    T3PickerButton{primary:false;objectName:"connectionAdd";text:T3I18n.tr("Add connection");enabled:!root.store.busy&&!!root.store.revision;onClicked:root.edit(null,null)}
   }
 Repeater{model:root.store.environments || []
  delegate:ColumnLayout {
   id:environment;required property var modelData
   Layout.fillWidth:true;Layout.margins:Style.space(12);spacing:Style.space(8)
   readonly property var status:(root.bridge.state.environments || []).find(function(e){return e.id===environment.modelData.id}) || ({state:"offline"})
   Rectangle{Layout.fillWidth:true;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)}
   RowLayout{Layout.fillWidth:true
    T3Icon{name:"server";tint:environment.status.state==="ready"?T3Palette.done:root.muted}
    ColumnLayout{Layout.fillWidth:true
     Text{text:environment.modelData.name;Layout.fillWidth:true;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
     Text{text:!environment.modelData.enabled?T3I18n.tr("Disabled"):environment.status.state==="ready"?T3I18n.tr("Connected"):environment.status.state==="offline"?T3I18n.tr("Offline"):T3I18n.error(environment.status.error || environment.status.state);Layout.fillWidth:true;color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideRight}
    }
    T3Switch{objectName:"connectionEnabled_"+environment.modelData.id;checked:!!environment.modelData.enabled;enabled:!root.store.busy;onToggled:root.run("enabled",{environmentId:environment.modelData.id,enabled:checked})}
    T3IconButton{name:"square-pen";tooltipText:T3I18n.tr("Rename");enabled:!root.store.busy;onClicked:root.rename(environment.modelData)}
    T3IconButton{name:"refresh-cw";tooltipText:T3I18n.tr("Reconnect");enabled:!root.store.busy&&environment.modelData.enabled;onClicked:root.run("reconnect",{environmentId:environment.modelData.id})}
    T3IconButton{name:"plus";tooltipText:T3I18n.tr("Add route");enabled:!root.store.busy;onClicked:root.edit(environment.modelData,null)}
    T3IconButton{name:"trash-2";tint:Color.urgent;tooltipText:T3I18n.tr("Remove connection");enabled:!root.store.busy;onClicked:root.remove(environment.modelData,null)}
   }
   Repeater{model:environment.modelData.routes || []
    delegate:RowLayout {
     required property var modelData;required property int index
     Layout.fillWidth:true;Layout.leftMargin:Style.space(24)
     T3Icon{name:modelData.kind==="SshConnectionTarget"?"terminal":"cloud";tint:root.muted}
     Text{Layout.fillWidth:true;text:modelData.ssh?(modelData.ssh.username?modelData.ssh.username+"@":"")+(modelData.ssh.alias || modelData.ssh.hostname)+(modelData.ssh.port?":"+modelData.ssh.port:""):modelData.url || "T3 Connect";color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
     T3IconButton{name:index===0?"star-filled":"star";tooltipText:T3I18n.tr("Preferred route");enabled:!root.store.busy&&index>0;onClicked:root.run("prefer-route",{environmentId:environment.modelData.id,connectionId:modelData.connectionId})}
     T3IconButton{name:"sliders-horizontal";tooltipText:T3I18n.tr("Edit connection");visible:modelData.kind!=="RelayConnectionTarget";enabled:!root.store.busy;onClicked:root.edit(environment.modelData,modelData)}
     T3IconButton{name:"x";tooltipText:T3I18n.tr("Remove route");visible:modelData.kind!=="RelayConnectionTarget";enabled:!root.store.busy;onClicked:root.remove(environment.modelData,modelData)}
    }
   }
  }
 }
  }}
 }
 T3PreferenceSection{Layout.fillWidth:true;title:"OmaBridge";icon:"globe"
  body:Component{ColumnLayout{width:parent.width;spacing:0
   T3ConnectionOmaBridge{Layout.fillWidth:true;Layout.margins:Style.space(14);bridge:root.bridge;environments:root.store.environments || []}
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(14)
    ColumnLayout{Layout.fillWidth:true
     Text{Layout.fillWidth:true;text:T3I18n.tr("Show OmaBridge in the bar");color:root.fg;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
     Text{Layout.fillWidth:true;text:T3I18n.tr("A small indicator next to T3 Code that shows this device's state and requests waiting for your approval.");color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.58);font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
    }
    T3Switch{objectName:"omabridgeIndicatorPreference";checked:(root.bridge.viewState || {}).omabridgeIndicator===true;Accessible.name:T3I18n.tr("Show OmaBridge in the bar");onToggled:root.bridge.rememberView({omabridgeIndicator:checked})}
   }
   // The same OmaBridge page as the Omarchy Control Panel, on the same device.
   T3OmaBridgeManage{Layout.fillWidth:true;Layout.margins:Style.space(14);bridge:root.bridge}
  }}
 }
 T3ConnectionPreferences{Layout.fillWidth:true;bridge:root.bridge}
 Controls.Popup {
  id:editor;parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(520),parent?parent.width-Style.space(32):Style.space(520));padding:Style.space(20);modal:true
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.18)}
  onClosed:{code.text="";endpoint.text=""}
  contentItem:ColumnLayout{spacing:Style.space(10)
   RowLayout{Layout.fillWidth:true;Text{Layout.fillWidth:true;text:T3I18n.tr(root.form==="rename"?"Rename":root.route?"Edit connection":"Add connection");color:root.fg;font.bold:true;font.pixelSize:Style.font.body}T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");enabled:!root.store.busy;onClicked:editor.close()}}
   RowLayout{visible:root.form!=="rename"&&!root.route;Layout.fillWidth:true
    T3PickerButton{primary:false;text:"SSH";checkable:true;checked:root.kind==="ssh";enabled:!root.store.busy;onClicked:root.kind="ssh"}
    T3PickerButton{primary:false;text:T3I18n.tr("Pairing link");checkable:true;checked:root.kind==="bearer";enabled:!root.store.busy;onClicked:root.kind="bearer"}
   }
   T3PickerField{id:label;objectName:"connectionLabel";Layout.fillWidth:true;placeholderText:T3I18n.tr("Name");enabled:!root.store.busy;selectByMouse:true}
   ColumnLayout{visible:root.form!=="rename"&&root.kind==="ssh";Layout.fillWidth:true
    T3PickerField{id:host;objectName:"connectionSshHost";Layout.fillWidth:true;placeholderText:T3I18n.tr("SSH host or alias");enabled:!root.store.busy;selectByMouse:true}
    RowLayout{Layout.fillWidth:true
     T3PickerField{id:username;objectName:"connectionSshUser";Layout.fillWidth:true;placeholderText:T3I18n.tr("Username");enabled:!root.store.busy;selectByMouse:true}
     T3PickerField{id:port;objectName:"connectionSshPort";Layout.preferredWidth:Style.space(90);placeholderText:T3I18n.tr("Port");validator:IntValidator{bottom:1;top:65535} enabled:!root.store.busy}
    }
   }
   ColumnLayout{visible:root.form!=="rename"&&root.kind==="bearer";Layout.fillWidth:true
    T3PickerField{id:endpoint;objectName:"connectionUrl";Layout.fillWidth:true;placeholderText:"https://host:3773";enabled:!root.store.busy;selectByMouse:true}
    T3PickerField{id:code;objectName:"connectionPairingCode";Layout.fillWidth:true;placeholderText:T3I18n.tr("Pairing code");echoMode:TextInput.Password;enabled:!root.store.busy;selectByMouse:true}
   }
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:T3I18n.error(root.store.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   T3PickerButton{primary:false;objectName:"connectionSave";Layout.alignment:Qt.AlignRight;text:T3I18n.tr("Save");enabled:!root.store.busy&&(root.form==="rename"?!!label.text.trim():root.kind==="ssh"?!!host.text.trim():!!endpoint.text.trim());onClicked:root.save()}
  }
 }
 Controls.Popup{
  id:confirm;parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(450),parent?parent.width-Style.space(32):Style.space(450));padding:Style.space(20);modal:true
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Color.urgent}
  contentItem:ColumnLayout{spacing:Style.space(12)
   Text{Layout.fillWidth:true;text:T3I18n.tr(root.route?"Remove route":"Remove connection")+" · "+(root.selected?root.selected.name:"");color:root.fg;font.bold:true;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Forget this connection on this device? Remote projects and messages are kept.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:T3I18n.error(root.store.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}T3PickerButton{primary:false;text:T3I18n.tr("Cancel");onClicked:confirm.close()}T3PickerButton{primary:false;objectName:"connectionConfirmRemove";text:T3I18n.tr("Delete");enabled:!root.store.busy;onClicked:root.run(root.route?"remove-route":"remove",{environmentId:root.selected.id,connectionId:root.route?root.route.connectionId:"",confirmed:true})}}
  }
 }
 Connections{target:root.bridge;function onConnectionsManaged(action,error){if(!error&&action!=="read"){editor.close();confirm.close();code.text=""}}}
}
