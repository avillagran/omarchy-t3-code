import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
// Content of the OmaBridge bar popup: state, two-step approvals, pairings still
// waiting on the other device, and a way into the full settings.
ColumnLayout {
 id:root
 required property var bridge
 signal settingsRequested()
 readonly property var store:bridge.connectionOmaBridge || ({})
 readonly property var value:store.data || ({})
 readonly property var published:value.published || ({})
 readonly property int pendingCount:(value.pending || []).length
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 // The approval being confirmed, if any.
 property var confirming:null
 function servicesText(list){return (list || []).map(function(s){return s==="t3"?"T3":s==="ssh"?"SSH":s}).join(", ")}
 function statusText(){
  if(value.available===false)return T3I18n.tr("OmaBridge is not installed. Rerun the installer to add it.")
  if(!value.device)return T3I18n.tr("This device has no OmaBridge identity yet.")
  var peers=(value.peers || []).length
  var state=published.active?T3I18n.tr("Published")+" · "+servicesText(published.services):T3I18n.tr("Not published")
  return state+" · "+T3I18n.tr("{count} paired",{count:peers})
 }
 spacing:Style.space(8)
   Text{Layout.fillWidth:true;text:"OmaBridge";color:root.fg;font.bold:true;font.pixelSize:Style.font.body}
   Text{objectName:"omabridgeIndicatorStatus";Layout.fillWidth:true;text:root.statusText();color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;visible:root.pendingCount>0;text:T3I18n.tr("Waiting for your approval");color:root.fg;font.pixelSize:Style.font.caption;font.bold:true}
   Repeater{model:root.value.pending || []
    delegate:ColumnLayout{id:entry;required property var modelData;Layout.fillWidth:true;spacing:Style.space(4)
     readonly property bool asking:!!root.confirming&&root.confirming.id===modelData.id&&root.confirming.expires===modelData.expires
     RowLayout{Layout.fillWidth:true
      Text{Layout.fillWidth:true;text:modelData.name+" · "+(modelData.id || "").slice(0,8)+" · "+root.servicesText(modelData.services);color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
      T3PickerButton{objectName:"omabridgeIndicatorApprove_"+modelData.id;visible:!entry.asking;text:T3I18n.tr("Approve…");enabled:!root.store.busy;onClicked:root.confirming=modelData}
     }
     // Inline confirmation: the full device ID and services, then an explicit second step.
     Text{Layout.fillWidth:true;visible:entry.asking;text:T3I18n.tr("Allow {name} to reach this device?",{name:modelData.name})+"\n"+modelData.id;color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.WrapAnywhere;textFormat:Text.PlainText}
     RowLayout{Layout.fillWidth:true;visible:parent.asking
      Item{Layout.fillWidth:true}
      T3PickerButton{primary:false;text:T3I18n.tr("Cancel");onClicked:root.confirming=null}
      T3PickerButton{objectName:"omabridgeIndicatorConfirm_"+modelData.id;text:T3I18n.tr("Approve");enabled:!root.store.busy;onClicked:root.bridge.requestConnectionOmaBridge("approve",{peer:modelData.id,expires:modelData.expires,confirmed:true})}
     }
    }
   }
   Repeater{model:root.value.awaiting || []
    delegate:RowLayout{required property var modelData;Layout.fillWidth:true
     Text{Layout.fillWidth:true;text:T3I18n.tr("Waiting for {name} to approve this device",{name:modelData.name || "?"});color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
     T3PickerButton{objectName:"omabridgeIndicatorRetry_"+modelData.id;primary:false;text:T3I18n.tr("Check again");enabled:!root.store.busy;onClicked:root.bridge.requestConnectionOmaBridge("pair-retry",{id:modelData.id,confirmed:true})}
    }
   }
   Text{Layout.fillWidth:true;visible:!!root.store.error;text:T3I18n.error(root.store.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   RowLayout{Layout.fillWidth:true
    Item{Layout.fillWidth:true}
    T3PickerButton{objectName:"omabridgeIndicatorSettings";primary:false;text:T3I18n.tr("OmaBridge settings…");onClicked:{root.confirming=null;root.settingsRequested()}}
   }
   Connections{target:root.bridge;function onConnectionOmaBridgeFinished(action,error){if(action==="approve"&&!error)root.confirming=null}}
}
