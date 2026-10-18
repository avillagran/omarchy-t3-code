import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
ColumnLayout {
 id:root
 required property var bridge
 readonly property var store:bridge.connectionNetwork || ({})
 readonly property var value:store.data || ({})
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 property var pending:null
 function request(action,values){bridge.requestConnectionNetwork(action,values || {})}
 function syncChecks(){networkToggle.checked=Qt.binding(function(){return root.value.mode==="network-accessible"});tailToggle.checked=Qt.binding(function(){return !!root.value.tailscaleServeEnabled})}
 function propose(mode,tailscale,port){pending={mode:mode,tailscaleServeEnabled:tailscale,tailscaleServePort:port,confirmed:true};syncChecks();confirmation.open()}
 function confirm(){if(pending)request("apply",pending)}
 Component.onCompleted:request("read")
 spacing:Style.space(12)
 RowLayout{Layout.fillWidth:true
  ColumnLayout{Layout.fillWidth:true
   Text{text:T3I18n.tr("Network access");color:root.fg;font.pixelSize:Style.font.body}
   Text{Layout.fillWidth:true;text:T3I18n.tr(root.value.mode==="network-accessible"?"Available to other devices on your network.":"Limited to this machine.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  }
  T3IconButton{name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");enabled:!root.store.busy;onClicked:root.request("read")}
  T3Switch{id:networkToggle;objectName:"connectionNetworkEnabled";checked:root.value.mode==="network-accessible";enabled:!root.store.busy&&!!root.value.canManage;onToggled:root.propose(checked?"network-accessible":"local-only",!!root.value.tailscaleServeEnabled,root.value.tailscaleServePort || 443)}
 }
 Repeater{model:root.value.endpoints || []
  RowLayout{required property var modelData;Layout.fillWidth:true
   Text{text:T3I18n.tr(modelData.name);color:root.muted;font.pixelSize:Style.font.caption;Layout.preferredWidth:Style.space(100)}
   T3PickerField{Layout.fillWidth:true;text:modelData.url;readOnly:true;implicitHeight:Style.space(32);font.pixelSize:Style.font.caption;Accessible.name:T3I18n.tr("Endpoint")}
   T3IconButton{name:"copy";tooltipText:T3I18n.tr("Copy");onClicked:root.bridge.copyText(modelData.url)}
  }
 }
 Rectangle{Layout.fillWidth:true;height:1;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)}
 RowLayout{Layout.fillWidth:true
  ColumnLayout{Layout.fillWidth:true
   Text{text:T3I18n.tr("Tailscale HTTPS");color:root.fg;font.pixelSize:Style.font.body}
   Text{Layout.fillWidth:true;text:root.value.tailscale&&root.value.tailscale.running?root.value.tailscale.dnsName:T3I18n.tr("Tailscale is not connected.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  }
  T3PickerField{id:port;objectName:"connectionTailscalePort";Layout.preferredWidth:Style.space(80);implicitHeight:Style.space(32);font.pixelSize:Style.font.caption;text:String(root.value.tailscaleServePort || 443);validator:IntValidator{bottom:1;top:65535} enabled:!root.store.busy&&!!root.value.canManage;Accessible.name:T3I18n.tr("Port")}
  T3PickerButton{objectName:"connectionTailscaleApplyPort";primary:false;text:T3I18n.tr("Save");visible:root.value.tailscaleServeEnabled&&Number(port.text)!==root.value.tailscaleServePort;enabled:!root.store.busy&&port.acceptableInput;onClicked:root.propose(root.value.mode,true,Number(port.text))}
  T3Switch{id:tailToggle;objectName:"connectionTailscaleEnabled";checked:!!root.value.tailscaleServeEnabled;enabled:!root.store.busy&&!!root.value.canManage&&!!root.value.tailscale&&(root.value.tailscale.running||checked)&&port.acceptableInput;onToggled:root.propose(root.value.mode,checked,Number(port.text))}
 }
 Text{Layout.fillWidth:true;visible:!!root.value.warning;text:T3I18n.error(root.value.warning || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Text{Layout.fillWidth:true;visible:!!root.store.error;text:T3I18n.error(root.store.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Text{Layout.fillWidth:true;visible:root.store.busy;text:T3I18n.tr("Loading…");color:root.muted;font.pixelSize:Style.font.caption}
 Text{Layout.fillWidth:true;visible:!!root.value.local&&!root.value.canManage;text:T3I18n.tr("Network changes are controlled by the application that started this server.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 Controls.Popup{id:confirmation;parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(500),parent?parent.width-Style.space(32):Style.space(500));padding:Style.space(20);modal:true;closePolicy:root.store.busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
  onClosed:{root.syncChecks();root.pending=null}
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.muted}
  contentItem:ColumnLayout{spacing:Style.space(14)
   Text{Layout.fillWidth:true;text:T3I18n.tr("Apply and restart T3 Code?");color:root.fg;font.bold:true;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;text:T3I18n.tr("T3 Code will restart its local server to apply the network settings. Your threads and drafts are kept.");color:root.muted;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:T3I18n.error(root.store.error || "");color:Color.urgent;wrapMode:Text.Wrap}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}T3PickerButton{primary:false;text:T3I18n.tr("Cancel");enabled:!root.store.busy;onClicked:confirmation.close()}T3PickerButton{objectName:"connectionNetworkConfirm";text:T3I18n.tr("Apply and restart");enabled:!root.store.busy;onClicked:root.confirm()}}
  }
 }
 Connections{target:root.bridge;function onConnectionNetworkFinished(action,error){if(action==="apply"&&!error)confirmation.close()}}
}
