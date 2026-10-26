import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
ColumnLayout {
 id:root
 required property var bridge
 readonly property var share:bridge.guestShare||null
 readonly property var operation:bridge.nightlyState?bridge.nightlyState("guest-create",bridge.selectedEnvironment,bridge.selectedThread):({})
 readonly property var closeOperation:bridge.nightlyState?bridge.nightlyState("guest-close",bridge.selectedEnvironment,bridge.selectedThread):({})
 readonly property var pending:share?(share.messages||[]).filter(function(m){return m.status==="pending"}):[]
 readonly property string url:operation.data&&operation.data.share&&share&&operation.data.share.id===share.id?operation.data.url||"":""
 visible:!!share||!!operation.busy||!!operation.error
 spacing:Style.space(6)
 RowLayout {
  Layout.fillWidth:true
  Text{Layout.fillWidth:true;text:T3I18n.tr("Guest links")+(root.share&&root.share.guestName?" · "+root.share.guestName:"");color:Color.popups.text;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
  T3PickerButton{objectName:"guestCopyLink";visible:!!root.url;primary:false;text:T3I18n.tr("Copy");onClicked:root.bridge.copyText(root.url)}
  T3PickerButton{objectName:"guestCloseLink";visible:!!root.share;primary:false;text:T3I18n.tr("Close");enabled:!!root.bridge.nightlyState&&(root.bridge.catalog||{}).canWrite!==false&&!root.closeOperation.busy;onClicked:root.bridge.requestNightly("guest-close",{})}
 }
 Text{Layout.fillWidth:true;visible:!!root.operation.error||!!root.closeOperation.error;text:root.operation.error||root.closeOperation.error||"";color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
 Repeater {
  model:root.pending
  delegate:ColumnLayout {
   required property var modelData
   Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:modelData.text||"";textFormat:Text.PlainText;wrapMode:Text.Wrap;color:Color.popups.text;font.pixelSize:Style.font.body}
   Repeater{model:modelData.attachments||[];delegate:Text{required property var modelData;Layout.fillWidth:true;text:modelData.name||"";textFormat:Text.PlainText;wrapMode:Text.Wrap;color:Color.popups.text;font.pixelSize:Style.font.caption}}
   RowLayout{
    readonly property var op:root.bridge.nightlyState("guest-send",root.bridge.selectedEnvironment,root.bridge.selectedThread)
    readonly property var dismiss:root.bridge.nightlyState("guest-dismiss",root.bridge.selectedEnvironment,root.bridge.selectedThread)
    T3PickerButton{objectName:"guestApprove_"+modelData.id;text:T3I18n.tr("Send message");enabled:root.bridge.catalog.canWrite!==false&&!parent.op.busy&&!parent.dismiss.busy;onClicked:root.bridge.requestNightly("guest-send",{guestMessageId:modelData.id})}
    T3PickerButton{objectName:"guestDismiss_"+modelData.id;primary:false;text:T3I18n.tr("Dismiss");enabled:root.bridge.catalog.canWrite!==false&&!parent.op.busy&&!parent.dismiss.busy;onClicked:root.bridge.requestNightly("guest-dismiss",{guestMessageId:modelData.id})}
    Text{Layout.fillWidth:true;text:parent.op.error||parent.dismiss.error||"";color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
   }
  }
 }
}
