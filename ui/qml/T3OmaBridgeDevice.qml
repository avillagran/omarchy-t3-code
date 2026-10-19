import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import QtQuick.Dialogs
import qs.Commons
// OmaBridge devices, managed here instead of omabridge-gui: this device's
// identity, one-use invitations, pairing from an invitation file, approvals,
// paired devices and revocation. Every change of trust asks for confirmation.
// Invitation files and QR codes are secrets: shown once, never kept in state.
ColumnLayout {
 id:root
 required property var bridge
 readonly property var store:bridge.connectionOmaBridge || ({})
 readonly property var value:store.data || ({})
 readonly property var device:value.device || null
 readonly property var created:store.created || null
 readonly property var published:value.published || ({})
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 property var pending:null
 function request(action,values){bridge.requestConnectionOmaBridge(action,values || {})}
 function shortId(id){return (id || "").slice(0,8)}
 function serviceLabel(s){return s==="t3"?"T3":s==="ssh"?"SSH":s}
 function servicesText(list){return (list || []).map(serviceLabel).join(", ")}
 function expiry(seconds){return seconds?new Date(seconds*1000).toLocaleTimeString(Qt.locale(),Locale.ShortFormat):""}
 function ask(action,values,title,detail){pending={action:action,values:Object.assign({confirmed:true},values || {}),title:title,detail:detail || ""};confirmation.open()}
 function errorText(code){
  if(code==="omabridge_not_published")return T3I18n.tr("Publish this device before inviting another one.")
  if(code==="omabridge_service_not_published")return T3I18n.tr("Only published services can be shared.")
  if(code==="omabridge_invite_expired")return T3I18n.tr("This invitation has expired. Ask for a new one.")
  if(code==="omabridge_invite_invalid" || code==="omabridge_invite_unreadable")return T3I18n.tr("This file is not a valid OmaBridge invitation.")
  if(code==="omabridge_ssh_unavailable")return T3I18n.tr("No SSH server is listening on this device.")
  if(code==="omabridge_unit_foreign")return T3I18n.tr("Another omarchy-t3-omabridge service exists. Remove it to publish from here.")
  if(code==="omabridge_peer_not_found")return T3I18n.tr("That device is no longer paired or waiting.")
  return T3I18n.error(code)
 }
 function publishStatus(){
  if(!published.enabled)return T3I18n.tr("Off. Publish this device to invite others to it.")
  if(published.stale)return T3I18n.tr("T3 Code moved to another port. Publish again to follow it.")
  if(!published.active)return T3I18n.tr("Published, but not running.")
  return T3I18n.tr("Published")+" · "+servicesText(published.services)
 }
 function restorePublish(){publishSwitch.checked=Qt.binding(function(){return !!root.published.enabled})}
 function pairFile(url){var path=String(url).replace(/^file:\/\//,"");ask("pair",{path:decodeURIComponent(path)},T3I18n.tr("Pair with the device in this invitation?"),T3I18n.tr("The other device must approve this one before it can be reached."))}
 spacing:Style.space(10)
 // This device
 RowLayout{Layout.fillWidth:true;visible:!!root.value.available
  T3Icon{name:"laptop";tint:root.device?Color.accent:root.muted}
  ColumnLayout{Layout.fillWidth:true
   Text{objectName:"omabridgeDeviceName";Layout.fillWidth:true;text:root.device?root.device.name:T3I18n.tr("This device has no OmaBridge identity yet.");color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;visible:!!root.device;text:T3I18n.tr("Device ID")+" · "+(root.device?root.device.id:"");color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
  }
  T3IconButton{visible:!!root.device;name:"copy";tooltipText:T3I18n.tr("Copy device ID");onClicked:root.bridge.copyText(root.device.id)}
  T3PickerButton{objectName:"omabridgeCreateIdentity";visible:!root.device;primary:false;text:T3I18n.tr("Create identity");enabled:!root.store.busy
   onClicked:root.ask("identity-create",{},T3I18n.tr("Create an OmaBridge identity for this device?"),T3I18n.tr("A private key is created on this device and never leaves it."))}
 }
 // Publishing this device runs OmaBridge as a user service; off by default.
 RowLayout{Layout.fillWidth:true;visible:!!root.value.available
  ColumnLayout{Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Publish this device");color:root.fg;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{objectName:"omabridgePublishStatus";Layout.fillWidth:true;text:root.publishStatus();color:root.published.stale?Color.urgent:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  }
  T3PickerButton{objectName:"omabridgeRepublish";visible:!!root.published.enabled&&(!!root.published.stale || !root.published.active);primary:false;text:T3I18n.tr("Publish again");enabled:!root.store.busy
   onClicked:{publishSsh.checked=(root.published.services || []).indexOf("ssh")>=0;publishDialog.open()}}
  T3Switch{id:publishSwitch;objectName:"omabridgePublish";checked:!!root.published.enabled;enabled:!root.store.busy;Accessible.name:T3I18n.tr("Publish this device")
   onToggled:{if(checked){publishSsh.checked=false;publishDialog.open()}else root.ask("unpublish",{},T3I18n.tr("Stop publishing this device?"),T3I18n.tr("Paired devices can no longer reach it. Pairings are kept."))}}
 }
 RowLayout{Layout.fillWidth:true;visible:!!root.value.available;spacing:Style.space(8)
  Item{Layout.fillWidth:true}
  T3PickerButton{objectName:"omabridgePairFile";primary:false;text:T3I18n.tr("Pair from invitation file…");enabled:!root.store.busy;onClicked:inviteFile.open()}
  T3PickerButton{objectName:"omabridgeInvite";primary:false;text:T3I18n.tr("Invite a device…");enabled:!root.store.busy&&!!root.device&&!!root.published.active;onClicked:{root.bridge.clearOmaBridgeInvite();inviteDialog.open()}}
 }
 // Pairings sent from here that the other device has not approved yet.
 Repeater{model:root.value.awaiting || []
  delegate:RowLayout{required property var modelData;Layout.fillWidth:true
   T3Icon{name:"circle-dashed";tint:root.muted}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Waiting for {name} to approve this device",{name:modelData.name || "?"})+" · "+T3I18n.tr("Expires")+" "+root.expiry(modelData.expires);color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   T3PickerButton{objectName:"omabridgeRetry_"+modelData.id;primary:false;text:T3I18n.tr("Check again");enabled:!root.store.busy;onClicked:root.request("pair-retry",{id:modelData.id,confirmed:true})}
   T3IconButton{name:"x";tooltipText:T3I18n.tr("Cancel");enabled:!root.store.busy;onClicked:root.request("pair-discard",{id:modelData.id,confirmed:true})}
  }
 }
 // Requests to reach this device.
 Text{Layout.fillWidth:true;visible:(root.value.pending || []).length>0;text:T3I18n.tr("Waiting for your approval");color:root.fg;font.pixelSize:Style.font.caption;font.bold:true}
 Repeater{model:root.value.pending || []
  delegate:RowLayout{required property var modelData;Layout.fillWidth:true
   T3Icon{name:"circle-alert";tint:Color.accent}
   ColumnLayout{Layout.fillWidth:true
    Text{Layout.fillWidth:true;text:modelData.name+" · "+root.shortId(modelData.id);color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    Text{Layout.fillWidth:true;text:T3I18n.tr("Asks to reach")+" "+root.servicesText(modelData.services);color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
   }
   T3PickerButton{objectName:"omabridgeApprove_"+modelData.id;text:T3I18n.tr("Approve…");enabled:!root.store.busy
    onClicked:root.ask("approve",{peer:modelData.id,expires:modelData.expires},T3I18n.tr("Allow {name} to reach this device?",{name:modelData.name}),root.servicesText(modelData.services)+" · "+modelData.id)}
  }
 }
 // Paired devices.
 Text{Layout.fillWidth:true;visible:(root.value.peers || []).length>0;text:T3I18n.tr("Paired devices");color:root.fg;font.pixelSize:Style.font.caption;font.bold:true}
 Repeater{model:root.value.peers || []
  delegate:RowLayout{required property var modelData;Layout.fillWidth:true
   T3Icon{name:"plug";tint:root.muted}
   ColumnLayout{Layout.fillWidth:true
    Text{Layout.fillWidth:true;text:modelData.name+" · "+root.shortId(modelData.id);color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    Text{objectName:"omabridgePeerServices_"+modelData.id;Layout.fillWidth:true
     text:[(modelData.remoteServices || []).length?T3I18n.tr("You can reach")+" "+root.servicesText(modelData.remoteServices):"",(modelData.services || []).length?T3I18n.tr("It can reach")+" "+root.servicesText(modelData.services):""].filter(function(s){return s}).join(" · ")
     color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
   }
   T3PickerButton{objectName:"omabridgeRevoke_"+modelData.id;primary:false;text:T3I18n.tr("Revoke…");enabled:!root.store.busy
    onClicked:root.ask("revoke",{peer:modelData.id},T3I18n.tr("Revoke {name}?",{name:modelData.name}),T3I18n.tr("Both directions stop working until the devices are paired again."))}
  }
 }
 Text{Layout.fillWidth:true;visible:!!root.store.paired&&root.store.paired.status==="approved";text:T3I18n.tr("Paired with {name}.",{name:root.store.paired?root.store.paired.name || "":""});color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 FileDialog{id:inviteFile;title:T3I18n.tr("Choose an OmaBridge invitation");fileMode:FileDialog.OpenFile;nameFilters:[T3I18n.tr("OmaBridge invitation")+" (*.json)","* (*)"];onAccepted:root.pairFile(selectedFile)}
 Controls.Popup{id:inviteDialog;objectName:"omabridgeInviteDialog";parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(480),parent?parent.width-Style.space(32):Style.space(480));padding:Style.space(20);modal:true
  closePolicy:root.store.busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape
  // The QR and path are secrets: forget them as soon as the dialog closes.
  onClosed:root.bridge.clearOmaBridgeInvite()
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
  contentItem:ColumnLayout{spacing:Style.space(12)
   Text{Layout.fillWidth:true;text:T3I18n.tr("Invite a device");color:root.fg;font.bold:true;font.pixelSize:Style.font.body}
   Text{Layout.fillWidth:true;text:T3I18n.tr("The invitation works once and expires in 10 minutes. Anyone who has it can ask to pair, so share it privately. You approve each request here.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   T3Switch{id:inviteT3;objectName:"omabridgeInviteT3";visible:!root.created&&(root.published.services || []).indexOf("t3")>=0;text:T3I18n.tr("Share T3 Code");checked:true}
   T3Switch{id:inviteSsh;objectName:"omabridgeInviteSsh";visible:!root.created&&(root.published.services || []).indexOf("ssh")>=0;text:T3I18n.tr("Share SSH");checked:false}
   T3Switch{id:inviteQr;objectName:"omabridgeInviteQr";visible:!root.created;text:T3I18n.tr("Show a QR code");checked:false}
   Image{objectName:"omabridgeInviteQrImage";Layout.alignment:Qt.AlignHCenter;Layout.preferredWidth:Style.space(220);Layout.preferredHeight:visible?Style.space(220):0;visible:!!root.created&&!!root.created.qrImage;source:root.created?root.created.qrImage || "":"";fillMode:Image.PreserveAspectFit;smooth:false;cache:false}
   Text{Layout.fillWidth:true;visible:!!root.created;text:T3I18n.tr("Invitation file");color:root.fg;font.pixelSize:Style.font.caption}
   T3PickerField{objectName:"omabridgeInvitePath";Layout.fillWidth:true;visible:!!root.created;text:root.created?root.created.path:"";readOnly:true;selectByMouse:true}
   Text{Layout.fillWidth:true;visible:!!root.created;text:T3I18n.tr("Expires")+" "+root.expiry(root.created?root.created.expires:0)+" · "+root.servicesText(root.created?root.created.services:[]);color:root.muted;font.pixelSize:Style.font.caption}
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:root.errorText(root.store.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   RowLayout{Layout.fillWidth:true
    T3PickerButton{objectName:"omabridgeInviteDiscard";visible:!!root.created;primary:false;text:T3I18n.tr("Delete invitation file");enabled:!root.store.busy;onClicked:root.request("invite-discard",{path:root.created.path,confirmed:true})}
    Item{Layout.fillWidth:true}
    T3PickerButton{primary:false;text:root.created?T3I18n.tr("Done"):T3I18n.tr("Cancel");enabled:!root.store.busy;onClicked:inviteDialog.close()}
    T3PickerButton{objectName:"omabridgeInviteCreate";visible:!root.created;text:T3I18n.tr("Create invitation");enabled:!root.store.busy&&((inviteT3.checked&&inviteT3.visible) || (inviteSsh.checked&&inviteSsh.visible))
     onClicked:root.request("invite",{services:[inviteT3.checked&&inviteT3.visible?"t3":"",inviteSsh.checked&&inviteSsh.visible?"ssh":""].filter(function(s){return s}),qr:inviteQr.checked,confirmed:true})}
   }
  }
 }
 Controls.Popup{id:publishDialog;objectName:"omabridgePublishDialog";parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(480),parent?parent.width-Style.space(32):Style.space(480));padding:Style.space(20);modal:true
  closePolicy:root.store.busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
  onClosed:root.restorePublish()
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
  contentItem:ColumnLayout{spacing:Style.space(14)
   Text{Layout.fillWidth:true;text:T3I18n.tr("Publish this device?");color:root.fg;font.bold:true;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;text:T3I18n.tr("OmaBridge runs in the background as a user service. Only devices you approve can reach T3 Code here, and T3 still asks for its own sign-in. Nothing is opened to anyone else.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   T3Switch{id:publishSsh;objectName:"omabridgePublishSsh";text:T3I18n.tr("Also share SSH");enabled:!!root.published.sshAvailable;checked:false}
   Text{Layout.fillWidth:true;visible:!root.published.sshAvailable;text:T3I18n.tr("No SSH server is listening on this device.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:root.errorText(root.store.error || "");color:Color.urgent;wrapMode:Text.Wrap}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}T3PickerButton{primary:false;text:T3I18n.tr("Cancel");enabled:!root.store.busy;onClicked:publishDialog.close()}T3PickerButton{objectName:"omabridgePublishConfirm";text:T3I18n.tr("Publish");enabled:!root.store.busy;onClicked:root.request("publish",{ssh:publishSsh.checked&&publishSsh.enabled,confirmed:true})}}
  }
 }
 Controls.Popup{id:confirmation;parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(480),parent?parent.width-Style.space(32):Style.space(480));padding:Style.space(20);modal:true;closePolicy:root.store.busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
  onClosed:{root.pending=null;root.restorePublish()}
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
  contentItem:ColumnLayout{spacing:Style.space(14)
   Text{Layout.fillWidth:true;text:root.pending?root.pending.title:"";color:root.fg;font.bold:true;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;visible:!!text;text:root.pending?root.pending.detail:"";color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.WrapAnywhere;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:root.errorText(root.store.error || "");color:Color.urgent;wrapMode:Text.Wrap}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}T3PickerButton{objectName:"omabridgeDeviceCancel";primary:false;text:T3I18n.tr("Cancel");enabled:!root.store.busy;onClicked:confirmation.close()}T3PickerButton{objectName:"omabridgeDeviceConfirm";text:T3I18n.tr("Confirm");enabled:!root.store.busy;onClicked:root.request(root.pending.action,root.pending.values)}}
  }
 }
 Connections{target:root.bridge;function onConnectionOmaBridgeFinished(action,error){if(!error&&root.pending&&root.pending.action===action)confirmation.close();if(!error&&action==="publish")publishDialog.close()}}
}
