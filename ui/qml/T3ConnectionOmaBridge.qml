import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
// OmaBridge transport for SSH and pairing-link connections. Pairing and grants
// are managed by OmaBridge itself. By default every granting peer is tried
// automatically; this view can switch that off or pin one peer per environment.
ColumnLayout {
 id:root
 required property var bridge
 property var environments:[]
 readonly property var store:bridge.connectionOmaBridge || ({})
 readonly property var value:store.data || ({})
 readonly property var routes:value.routes || ({})
 readonly property var active:value.active || ({})
 readonly property bool automatic:value.default!=="off"
 // Automatic sharing: peers that share T3 Code with this device join T3's connections.
 readonly property var sharing:value.sharing || ({})
 readonly property bool sharingOn:sharing.enabled!==false
 readonly property var sharingPeers:(value.peers || []).filter(function(p){return (p.remoteServices || []).indexOf("t3")>=0 || !!(sharing.peers || {})[p.id]})
 // Pairing links travel over a peer's "t3" service, SSH profiles over its "ssh" service.
 function servicesFor(environment){var kinds=(environment.routes || []).map(function(r){return r.kind});var out=[];if(kinds.indexOf("BearerConnectionTarget")>=0)out.push("t3");if(kinds.indexOf("SshConnectionTarget")>=0)out.push("ssh");return out}
 function optionsFor(environment){var wanted=servicesFor(environment),out=[];(value.peers || []).forEach(function(p){wanted.forEach(function(s){if((p.remoteServices || []).indexOf(s)>=0)out.push({peer:p.id,service:s,label:root.peerName(p.id)+" · "+(s==="ssh"?"SSH":"T3")})})});return out}
 readonly property bool anyCandidate:(environments || []).some(function(e){return root.optionsFor(e).length>0})
 readonly property var linkable:(environments || []).filter(function(e){return root.servicesFor(e).length>0})
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 property var pending:null
 // Rebuilding rows after a cancelled change restores selectors to the saved state.
 property int epoch:0
 function request(action,values){bridge.requestConnectionOmaBridge(action,values || {})}
 function peerName(id){var p=(value.peers || []).find(function(c){return c.id===id});return p?p.name+" · "+id.slice(0,8):id.slice(0,8)}
 function propose(environment,option,only){
  pending=option==="auto"?{action:"clear",environmentId:environment.id,label:environment.name,confirmed:true}
   :option==="off"?{action:"set",environmentId:environment.id,label:environment.name,mode:"off",confirmed:true}
   :{action:"set",environmentId:environment.id,label:environment.name,peer:option.peer,service:option.service,mode:only?"only":"prefer",confirmed:true}
  confirmation.open()
 }
 function proposeSharing(on){pending={action:"share-enabled",enabled:on,label:"",confirmed:true};confirmation.open()}
 function shareText(peer){
  var row=(sharing.peers || {})[peer.id] || {}
  if(row.state==="shared")return T3I18n.tr("In your connections · {address}",{address:"127.0.0.1:"+row.port})
  if(row.state==="skipped")return T3I18n.tr("Removed by you")
  if(row.state==="error")return row.error==="omabridge_t3_pair_refused"?T3I18n.tr("That device refused the pairing. Check that it still shares T3 with this one.")
   :row.error==="omabridge_t3_pair_unavailable"||row.error==="omabridge_peer_unreachable"?T3I18n.tr("That device can't be reached over OmaBridge, or its OmaBridge needs an update. Trying again in a few minutes.")
   :row.error==="share_is_this_device"?T3I18n.tr("This is this device's own T3 Code")
   :T3I18n.tr("Could not add it ({code}). Trying again in a few minutes.",{code:row.error || "unknown"})
  return sharingOn?T3I18n.tr("Adding to your connections…"):T3I18n.tr("Not added")
 }
 function proposeDefault(on){pending={action:"default",value:on?"auto":"off",label:"",confirmed:true};confirmation.open()}
 // Selector entries: 0 automatic, 1 saved route only, then pinned peers.
 function selectedIndex(current,options){if(!current)return 0;if(current.mode==="off")return 1;return Math.max(0,options.findIndex(function(o){return o.peer===current.peer&&o.service===current.service})+2)}
 function statusText(environment,current){
  var live=active[environment.id];if(live)return T3I18n.tr("Connected through OmaBridge")+" · "+peerName(live.peer)
  if(current&&current.mode==="only")return T3I18n.tr("Only through OmaBridge")
  if(current&&current.mode!=="off")return T3I18n.tr("OmaBridge first, then the saved route")
  if(!current&&automatic)return T3I18n.tr("Automatic · saved route in use")
  return T3I18n.tr("Saved route")
 }
 function pendingTitle(){
  if(!pending)return ""
  if(pending.action==="default")return pending.value==="auto"?T3I18n.tr("Use OmaBridge for every connection?"):T3I18n.tr("Stop using OmaBridge by default?")
  if(pending.action==="share-enabled")return pending.enabled?T3I18n.tr("Share T3 automatically?"):T3I18n.tr("Stop sharing T3 automatically?")
  var title=pending.action==="clear"?T3I18n.tr("Return this connection to automatic routing?"):pending.mode==="off"?T3I18n.tr("Stop using OmaBridge for this connection?"):T3I18n.tr("Route this connection through OmaBridge?")
  return title+" · "+pending.label
 }
 function pendingDetail(){
  if(!pending)return ""
  if(pending.action==="default")return pending.value==="auto"?T3I18n.tr("Paired devices are tried first, and each must prove it serves the connection. The saved route stays as fallback."):T3I18n.tr("The saved T3 route is used again.")
  if(pending.action==="share-enabled")return pending.enabled?T3I18n.tr("Paired devices that share T3 Code with this one are added to your connections, for O3 and T3's desktop app, with T3's own pairing."):T3I18n.tr("Devices already added stay in your connections.")
  if(pending.action==="clear")return T3I18n.tr("Paired devices are tried first, and each must prove it serves the connection. The saved route stays as fallback.")
  if(pending.mode==="off")return T3I18n.tr("The saved T3 route is used again.")
  return peerName(pending.peer)+" · "+(pending.service==="ssh"?"SSH":"T3")+" · "+(pending.mode==="only"?T3I18n.tr("Only through OmaBridge"):T3I18n.tr("OmaBridge first, then the saved route"))
 }
 function confirm(){if(pending){var values=Object.assign({},pending);delete values.label;delete values.action;request(pending.action,values)}}
 Component.onCompleted:request("read")
 spacing:Style.space(10)
 RowLayout{Layout.fillWidth:true
  ColumnLayout{Layout.fillWidth:true
   Text{text:"OmaBridge";color:root.fg;font.pixelSize:Style.font.body}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Carry T3 connections through your paired OmaBridge devices, without a VPN. T3 still authenticates every session.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  }
  T3IconButton{objectName:"omabridgeRefresh";name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");enabled:!root.store.busy;onClicked:root.request("read")}
 }
 RowLayout{Layout.fillWidth:true;visible:!!root.value.identity
  ColumnLayout{Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Use OmaBridge for every connection");color:root.fg;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Paired devices are tried first, and each must prove it serves the connection. The saved route stays as fallback.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  }
  T3Switch{id:defaultSwitch;objectName:"omabridgeDefault";checked:root.automatic;enabled:!root.store.busy;Accessible.name:T3I18n.tr("Use OmaBridge for every connection");onToggled:root.proposeDefault(checked)}
 }
 RowLayout{Layout.fillWidth:true;visible:!!root.value.identity
  ColumnLayout{Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Share T3 automatically");color:root.fg;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;text:T3I18n.tr("Paired devices that share T3 Code with this one are added to your connections, for O3 and T3's desktop app, with T3's own pairing.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  }
  T3Switch{id:shareSwitch;objectName:"omabridgeShare";checked:root.sharingOn;enabled:!root.store.busy;Accessible.name:T3I18n.tr("Share T3 automatically");onToggled:root.proposeSharing(checked)}
 }
 Repeater{model:root.value.identity?root.sharingPeers:[]
  delegate:RowLayout{
   id:shareRow;required property var modelData
   readonly property var state:(root.sharing.peers || {})[modelData.id] || ({})
   Layout.fillWidth:true
   T3Icon{name:"monitor";tint:shareRow.state.state==="shared"?T3Palette.done:root.muted}
   ColumnLayout{Layout.fillWidth:true
    Text{Layout.fillWidth:true;text:root.peerName(shareRow.modelData.id);color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    Text{objectName:"omabridgeShareStatus_"+shareRow.modelData.id;Layout.fillWidth:true;text:root.shareText(shareRow.modelData);color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   }
   T3PickerButton{objectName:"omabridgeShareRetry_"+shareRow.modelData.id;primary:false;visible:shareRow.state.state==="skipped"||shareRow.state.state==="error";text:T3I18n.tr("Add again");enabled:!root.store.busy;onClicked:root.request("share-retry",{peer:shareRow.modelData.id,confirmed:true})}
  }
 }
 Text{Layout.fillWidth:true;visible:!!root.store.data&&!root.value.available;text:T3I18n.tr("OmaBridge is not installed. Rerun the installer to add it.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Text{Layout.fillWidth:true;visible:!!root.value.available&&!root.value.identity;text:T3I18n.tr("No OmaBridge identity yet. Create one below to pair this device.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Text{Layout.fillWidth:true;visible:!!root.value.identity&&!root.anyCandidate;text:T3I18n.tr("No paired device shares T3 Code with this one yet.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Text{Layout.fillWidth:true;visible:!!root.store.data&&root.linkable.length===0;text:T3I18n.tr("Add a pairing-link or SSH connection to route it through OmaBridge.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Repeater{model:root.epoch>=0?root.linkable:[]
  delegate:RowLayout{
   id:row;required property var modelData
   readonly property var current:root.routes[modelData.id] || null
   readonly property var options:root.optionsFor(modelData)
   Layout.fillWidth:true
   T3Icon{name:"globe";tint:root.active[row.modelData.id]?T3Palette.done:root.muted}
   ColumnLayout{Layout.fillWidth:true
    Text{Layout.fillWidth:true;text:row.modelData.name;color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    Text{objectName:"omabridgeStatus_"+row.modelData.id;Layout.fillWidth:true;text:root.statusText(row.modelData,row.current);color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
   }
   T3Select{
    objectName:"omabridgePeer_"+row.modelData.id;Layout.preferredWidth:Style.space(220);popupAbove:false
    model:[T3I18n.tr("Automatic"),T3I18n.tr("Saved route only")].concat(row.options.map(function(o){return o.label}))
    currentIndex:root.selectedIndex(row.current,row.options)
    enabled:!root.store.busy&&!!root.value.identity
    onActivated:function(index){if(index===root.selectedIndex(row.current,row.options))return;root.propose(row.modelData,index===0?"auto":index===1?"off":row.options[index-2],!!row.current&&row.current.mode==="only")}
   }
   T3Switch{
    objectName:"omabridgeOnly_"+row.modelData.id;checked:!!row.current&&row.current.mode==="only";enabled:!root.store.busy&&!!row.current&&row.current.mode!=="off"
    Accessible.name:T3I18n.tr("Only through OmaBridge")
    onToggled:root.propose(row.modelData,{peer:row.current.peer,service:row.current.service},checked)
   }
  }
 }
 Text{Layout.fillWidth:true;visible:!!root.store.error || !!root.value.error;text:T3I18n.error(root.store.error || root.value.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Text{Layout.fillWidth:true;visible:!!root.store.busy;text:T3I18n.tr("Loading…");color:root.muted;font.pixelSize:Style.font.caption}
 Controls.Popup{id:confirmation;parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(480),parent?parent.width-Style.space(32):Style.space(480));padding:Style.space(20);modal:true;closePolicy:root.store.busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
  onClosed:{root.pending=null;root.epoch++;defaultSwitch.checked=Qt.binding(function(){return root.automatic});shareSwitch.checked=Qt.binding(function(){return root.sharingOn})}
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
  contentItem:ColumnLayout{spacing:Style.space(14)
   Text{Layout.fillWidth:true;text:root.pendingTitle();color:root.fg;font.bold:true;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;text:root.pendingDetail();color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;visible:!root.pending||root.pending.action!=="share-enabled";text:root.pending&&root.pending.action==="default"?T3I18n.tr("Every connection reconnects. Threads and drafts are kept."):T3I18n.tr("This environment reconnects. Threads and drafts are kept.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:T3I18n.error(root.store.error || "");color:Color.urgent;wrapMode:Text.Wrap}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}T3PickerButton{primary:false;text:T3I18n.tr("Cancel");enabled:!root.store.busy;onClicked:confirmation.close()}T3PickerButton{objectName:"omabridgeConfirm";text:T3I18n.tr("Apply");enabled:!root.store.busy;onClicked:root.confirm()}}
  }
 }
 Connections{target:root.bridge;function onConnectionOmaBridgeFinished(action,error){if(action!=="read"&&!error)confirmation.close();root.epoch++}}
}
