import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
ColumnLayout {
 id:root
 required property var bridge
 required property string environment
 readonly property var store:bridge.connectionAccess || ({})
 readonly property bool scoped:store.environment===environment
 readonly property var accessData:scoped&&store.data?store.data:({})
 readonly property var session:accessData.session || ({})
 readonly property var granted:session.permissions || session.scopes || []
 readonly property bool busy:scoped&&!!store.busy
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 property bool expanded:false
 property var selectedScopes:[]
 property var pending:null
 readonly property var options:[
  {scope:"orchestration:read",title:"View environment"},{scope:"orchestration:operate",title:"Operate tasks"},
  {scope:"settings:write",title:"Change environment settings"},{scope:"providers:manage",title:"Manage providers"},
  {scope:"environment:maintain",title:"Maintain environment"},{scope:"preview:operate",title:"Control previews"},
  {scope:"diagnostics:read",title:"View diagnostics and usage"},{scope:"terminal:read",title:"View terminals"},
  {scope:"terminal:operate",title:"Use terminals"},{scope:"source-control:write",title:"Change source control"},
  {scope:"filesystem:read",title:"Read files"},{scope:"filesystem:write",title:"Write files"},
  {scope:"access:read",title:"View access"},{scope:"access:write",title:"Manage access"},
  {scope:"relay:read",title:"View relay"},{scope:"relay:write",title:"Manage relay"}]
 function can(scope){return granted.indexOf(scope)>=0}
 function request(action,values){bridge.requestConnectionAccess(environment,action,values || {})}
 function load(){if(environment)request("read")}
 function beginPairing(){
  selectedScopes=granted.filter(function(s){return s!=="access:read"&&s!=="access:write"&&s!=="relay:write"&&root.options.some(function(o){return o.scope===s})})
  pairingLabel.text="";pairingEndpoint.text=accessData.endpoint || "";pairing.open()
 }
 function toggleScope(scope,checked){
  var next=selectedScopes.filter(function(s){return s!==scope})
  if(checked){next.push(scope);if(scope==="terminal:operate"&&next.indexOf("terminal:read")<0)next.push("terminal:read")}
  else if(scope==="terminal:read")next=next.filter(function(s){return s!=="terminal:operate"})
  selectedScopes=next
 }
 function revoke(action,record){pending={action:action,record:record,environment:environment};confirmation.open()}
 function confirmRevoke(){if(!pending||pending.environment!==environment)return;var p=pending;request(p.action,{id:p.record.id || "",sessionId:p.record.sessionId || "",confirmed:true})}
 function labelFor(record){return record.label || (record.client || {}).label || record.subject || record.id || record.sessionId || ""}
 function date(value){var d=new Date(value);return isNaN(d.getTime())?String(value || ""):d.toLocaleString(Qt.locale(),Locale.ShortFormat)}
 function summary(record){return (record.permissions || record.scopes || []).join(" · ")}
 function preferences(publish,hold){request("connect-preferences",{publishAgentActivity:publish,holdWebhooksWhileOffline:hold})}
 Component.onCompleted:load()
 onEnvironmentChanged:{pairing.close();confirmation.close();pending=null;expanded=false;if(bridge.clearConnectionCredential)bridge.clearConnectionCredential();load()}
 Component.onDestruction:if(bridge.dismissConnectionAccess)bridge.dismissConnectionAccess();else if(bridge.clearConnectionCredential)bridge.clearConnectionCredential()
 spacing:Style.space(20)
 Text{Layout.fillWidth:true;visible:root.busy;text:T3I18n.tr("Loading…");color:root.muted;font.pixelSize:Style.font.caption}
 Text{Layout.fillWidth:true;visible:scoped&&!!store.error;text:T3I18n.error(store.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
 T3PreferenceSection{Layout.fillWidth:true;title:root.accessData.descriptor?root.accessData.descriptor.label:"This machine";icon:"server"
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(12)
 T3LocalConnection{Layout.fillWidth:true;Layout.margins:Style.space(12);visible:root.environment==="local";bridge:root.bridge}
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(14)
    ColumnLayout{Layout.fillWidth:true
     Text{text:T3I18n.tr("Version");color:root.fg;font.pixelSize:Style.font.body}
     Text{text:(root.accessData.descriptor || {}).serverVersion || "—";color:root.muted;font.pixelSize:Style.font.caption}
    }
    T3IconButton{objectName:"connectionAccessRefresh";name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");enabled:!root.busy;onClicked:root.load()}
   }
   Rectangle{Layout.fillWidth:true;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)}
   Loader{Layout.fillWidth:true;Layout.margins:Style.space(14);active:root.environment==="local";visible:active;sourceComponent:Component{T3ConnectionNetwork{bridge:root.bridge}}}
   RowLayout{visible:root.environment!=="local";Layout.fillWidth:true;Layout.margins:Style.space(14)
    ColumnLayout{Layout.fillWidth:true
     Text{text:T3I18n.tr("Network access");color:root.fg;font.pixelSize:Style.font.body}
     Text{Layout.fillWidth:true;text:T3I18n.tr(root.session.auth&&root.session.auth.policy==="remote-reachable"?"Remote access is already configured. Change network exposure where the server starts.":"Only this machine can connect. Restart with a non-loopback host for remote pairing.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
     T3PickerField{Layout.fillWidth:true;text:root.accessData.endpoint || "";readOnly:true;selectByMouse:true;font.pixelSize:Style.font.caption;Accessible.name:T3I18n.tr("Endpoint")}
    }
    T3Switch{checked:!!root.session.auth&&root.session.auth.policy==="remote-reachable";enabled:false}
   }
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(14);visible:!!root.accessData.descriptor&&!!root.accessData.descriptor.capabilities&&root.accessData.descriptor.capabilities.environmentIcon===true
    Text{Layout.fillWidth:true;text:T3I18n.tr("Environment icon");color:root.fg;font.pixelSize:Style.font.body}
    T3Select{objectName:"connectionEnvironmentIcon";Layout.preferredWidth:Style.space(170);popupAbove:false;model:["Automatic","server","cloud","linux","desktop","laptop","mac-mini","mac-studio"];currentIndex:Math.max(0,model.indexOf(root.accessData.environmentIcon || "Automatic"));enabled:!root.busy&&root.can("settings:write")&&!root.accessData.iconError;onActivated:function(i){root.request("environment-icon",{icon:i===0?null:model[i]})}}
   }
   Text{Layout.fillWidth:true;Layout.margins:Style.space(14);visible:!!root.accessData.descriptorError;text:T3I18n.error(root.accessData.descriptorError || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  }}
 }
 T3PreferenceSection{Layout.fillWidth:true;title:"T3 Connect";icon:"cloud";visible:!!root.accessData.connect
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(12)
   Text{Layout.fillWidth:true;Layout.margins:Style.space(14);text:T3I18n.tr(root.accessData.connect&&root.accessData.connect.linked?"Connected":"Offline");color:root.muted;font.pixelSize:Style.font.caption}
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(14)
    Text{Layout.fillWidth:true;text:T3I18n.tr("Publish agent activity");color:root.fg;font.pixelSize:Style.font.body}
    T3Switch{objectName:"connectionPublishActivity";checked:!!root.accessData.connect&&!!root.accessData.connect.publishAgentActivity;enabled:!root.busy&&root.can("relay:write")&&!!root.accessData.connect&&!!root.accessData.connect.linked;onToggled:root.preferences(checked,!!root.accessData.connect.holdWebhooksWhileOffline)}
   }
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(14);visible:!!root.accessData.connect&&!!root.accessData.connect.managedTunnelActive
    Text{Layout.fillWidth:true;text:T3I18n.tr("Hold webhooks while offline");color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
    T3Switch{objectName:"connectionHoldWebhooks";checked:!!root.accessData.connect&&!!root.accessData.connect.holdWebhooksWhileOffline;enabled:!root.busy&&root.can("relay:write");onToggled:root.preferences(!!root.accessData.connect.publishAgentActivity,checked)}
   }
  }}
 }
 T3PreferenceSection{Layout.fillWidth:true;title:"Authorized clients";icon:"shield-check"
  body:Component{ColumnLayout{width:parent.width;spacing:Style.space(10)
   RowLayout{Layout.fillWidth:true;Layout.margins:Style.space(12)
    Controls.AbstractButton{objectName:"connectionAccessExpand";Layout.fillWidth:true;implicitHeight:Style.space(32);onClicked:root.expanded=!root.expanded
     contentItem:RowLayout{T3Icon{name:root.expanded?"chevron-down":"chevron-right";tint:root.muted}Text{Layout.fillWidth:true;text:T3I18n.tr("Authorized clients")+" · "+(root.accessData.clients || []).length;color:root.fg;font.pixelSize:Style.font.body}}
    }
    T3PickerButton{primary:false;objectName:"connectionCreateLink";text:T3I18n.tr("Create pairing link");enabled:!root.busy&&root.can("access:write");onClicked:root.beginPairing()}
   }
   ColumnLayout{Layout.fillWidth:true;Layout.margins:Style.space(14);visible:root.expanded;spacing:Style.space(12)
    Text{Layout.fillWidth:true;visible:!root.can("access:read");text:T3I18n.tr("This connection cannot view authorized clients.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
    Text{Layout.fillWidth:true;visible:!!root.accessData.accessError;text:T3I18n.error(root.accessData.accessError || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
    Text{visible:root.can("access:read")&&root.accessData.clients!==null&&!(root.accessData.clients || []).length;text:T3I18n.tr("No authorized clients");color:root.muted;font.pixelSize:Style.font.caption}
    Repeater{model:root.accessData.clients || []
     delegate:ColumnLayout{id:client;required property var modelData;Layout.fillWidth:true
      RowLayout{Layout.fillWidth:true
       T3Icon{name:client.modelData.client&&client.modelData.client.deviceType==="mobile"?"smartphone":"monitor";tint:client.modelData.connected?T3Palette.done:root.muted}
       ColumnLayout{Layout.fillWidth:true
        Text{Layout.fillWidth:true;text:root.labelFor(client.modelData)+(client.modelData.current?" · "+T3I18n.tr("Current session"):"");color:root.fg;elide:Text.ElideRight;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
        Text{Layout.fillWidth:true;text:[(client.modelData.client || {}).os,(client.modelData.client || {}).browser,client.modelData.connected?T3I18n.tr("Connected"):root.date(client.modelData.lastConnectedAt || client.modelData.issuedAt)].filter(Boolean).join(" · ");color:root.muted;elide:Text.ElideRight;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
       }
       T3IconButton{objectName:"connectionRevokeClient_"+client.modelData.sessionId;name:"trash-2";tooltipText:T3I18n.tr("Revoke access");enabled:!root.busy&&root.can("access:write")&&!client.modelData.current;onClicked:root.revoke("revoke-client",client.modelData)}
      }
      Text{Layout.fillWidth:true;Layout.leftMargin:Style.space(25);text:root.summary(client.modelData);color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
     }
    }
    Text{text:T3I18n.tr("Pairing links");color:root.fg;font.pixelSize:Style.font.body;visible:root.can("access:read")}
    Text{visible:root.can("access:read")&&root.accessData.pairingLinks!==null&&!(root.accessData.pairingLinks || []).length;text:T3I18n.tr("No pairing links");color:root.muted;font.pixelSize:Style.font.caption}
    Repeater{model:root.accessData.pairingLinks || []
     delegate:ColumnLayout{id:link;required property var modelData;Layout.fillWidth:true
      RowLayout{Layout.fillWidth:true
       T3Icon{name:"link";tint:root.muted}
       ColumnLayout{Layout.fillWidth:true
        Text{Layout.fillWidth:true;text:root.labelFor(link.modelData);color:root.fg;elide:Text.ElideRight;font.pixelSize:Style.font.body;textFormat:Text.PlainText}
        Text{Layout.fillWidth:true;text:T3I18n.tr("Expires")+" · "+root.date(link.modelData.expiresAt);color:root.muted;elide:Text.ElideRight;font.pixelSize:Style.font.caption}
       }
       T3IconButton{objectName:"connectionRevokeLink_"+link.modelData.id;name:"trash-2";tooltipText:T3I18n.tr("Revoke access");enabled:!root.busy&&root.can("access:write");onClicked:root.revoke("revoke-link",link.modelData)}
      }
      Text{Layout.fillWidth:true;Layout.leftMargin:Style.space(25);text:root.summary(link.modelData);color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
     }
    }
    T3PickerButton{primary:false;objectName:"connectionRevokeOthers";text:T3I18n.tr("Revoke other clients");enabled:!root.busy&&root.can("access:write")&&(root.accessData.clients===null||(root.accessData.clients || []).some(function(c){return !c.current}));onClicked:root.revoke("revoke-others",{})}
   }
  }}
 }
 Controls.Popup{id:pairing;parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(540),parent?parent.width-Style.space(32):Style.space(540));padding:Style.space(20);modal:true;closePolicy:root.busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
  onClosed:{if(root.bridge.clearConnectionCredential)root.bridge.clearConnectionCredential();if(root.scoped&&!root.busy)root.load()}
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
  contentItem:ColumnLayout{spacing:Style.space(12)
   RowLayout{Layout.fillWidth:true;Text{Layout.fillWidth:true;text:T3I18n.tr("Create pairing link");color:root.fg;font.bold:true;font.pixelSize:Style.font.body}T3IconButton{name:"x";enabled:!root.busy;tooltipText:T3I18n.tr("Close");onClicked:pairing.close()}}
   T3PickerField{id:pairingLabel;objectName:"connectionNewLinkLabel";Layout.fillWidth:true;placeholderText:T3I18n.tr("Name");enabled:!root.busy&&!root.store.created;selectByMouse:true}
   T3PickerField{id:pairingEndpoint;objectName:"connectionPairingEndpoint";Layout.fillWidth:true;placeholderText:T3I18n.tr("Endpoint");text:root.accessData.endpoint || "";selectByMouse:true;enabled:!root.busy&&!root.store.created}
   Controls.ScrollView{Layout.fillWidth:true;Layout.preferredHeight:Style.space(280);visible:!root.store.created;contentWidth:availableWidth;clip:true
    Column{width:parent.width;Repeater{model:root.options.filter(function(o){return root.can(o.scope)})
     Controls.CheckBox{required property var modelData;width:parent.width;text:T3I18n.tr(modelData.title);palette.windowText:root.fg;palette.text:root.fg;checked:root.selectedScopes.indexOf(modelData.scope)>=0;enabled:!root.busy;onToggled:root.toggleScope(modelData.scope,checked)}
    }}
   }
   Image{objectName:"connectionPairingQr";Layout.alignment:Qt.AlignHCenter;Layout.preferredWidth:Style.space(180);Layout.preferredHeight:visible?Style.space(180):0;visible:root.scoped&&!!root.store.created&&!!root.store.created.qrImage;source:root.scoped&&root.store.created?root.store.created.qrImage || "":"";fillMode:Image.PreserveAspectFit}
   Text{Layout.fillWidth:true;visible:root.scoped&&!!root.store.created;text:T3I18n.tr("Pairing code");color:root.fg;font.pixelSize:Style.font.body}
   T3PickerField{objectName:"connectionCreatedCode";Layout.fillWidth:true;visible:root.scoped&&!!root.store.created;text:root.store.created?root.store.created.credential || "":"";readOnly:true;selectByMouse:true;Accessible.name:T3I18n.tr("Pairing code")}
   T3PickerField{objectName:"connectionCreatedUrl";Layout.fillWidth:true;visible:root.scoped&&!!root.store.created;text:root.store.created?root.store.created.pairingUrl || "":"";readOnly:true;selectByMouse:true;Accessible.name:T3I18n.tr("Pairing link")}
   Text{Layout.fillWidth:true;visible:!!root.store.created;text:T3I18n.tr("Expires")+" · "+root.date(root.store.created?root.store.created.expiresAt:"");color:root.muted;font.pixelSize:Style.font.caption}
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:T3I18n.error(root.store.error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   T3PickerButton{primary:false;objectName:"connectionIssueLink";Layout.alignment:Qt.AlignRight;visible:!root.store.created;text:T3I18n.tr("Create pairing link");enabled:!root.busy&&root.selectedScopes.length>0;onClicked:root.request("create-link",{label:pairingLabel.text,scopes:root.selectedScopes,endpoint:pairingEndpoint.text})}
   T3PickerButton{primary:false;objectName:"connectionCopyCode";Layout.alignment:Qt.AlignRight;visible:!!root.store.created;text:T3I18n.tr("Copy pairing link");onClicked:root.bridge.copyText(root.store.created.pairingUrl || root.store.created.credential)}
  }
 }
 Controls.Popup{id:confirmation;parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(450),parent?parent.width-Style.space(32):Style.space(450));padding:Style.space(20);modal:true
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Color.urgent}
  contentItem:ColumnLayout{spacing:Style.space(14)
   Text{Layout.fillWidth:true;text:T3I18n.tr("Revoke access")+" · "+(root.pending?root.labelFor(root.pending.record):"");color:root.fg;font.bold:true;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;text:T3I18n.tr("This client will need a new pairing link to connect again.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:T3I18n.error(root.store.error || "");color:Color.urgent;wrapMode:Text.Wrap}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}T3PickerButton{primary:false;text:T3I18n.tr("Cancel");enabled:!root.busy;onClicked:confirmation.close()}T3PickerButton{primary:false;objectName:"connectionConfirmRevoke";text:T3I18n.tr("Revoke access");enabled:!root.busy;onClicked:root.confirmRevoke()}}
  }
 }
 Connections{target:root.bridge;ignoreUnknownSignals:true
  function onConnectionCloudFinished(action,error){if(!error&&action!=="read"&&root.environment==="local")root.load()}
  function onConnectionNetworkFinished(action,error){if(!error&&action==="apply"&&root.environment==="local")root.load()}
 }
 Connections{target:root.bridge;function onConnectionAccessFinished(action,error){if(error)return;if(action==="revoke-client"||action==="revoke-link"||action==="revoke-others"){confirmation.close();root.load()}else if(action==="connect-preferences"||action==="environment-icon")root.load()}}
}
