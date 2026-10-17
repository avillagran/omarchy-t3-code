import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// Parent supplies native projection references, authenticated gates and durable outbox.
// Signals are local integration events, NOT new native RPC methods.
Item {
 id:root
 objectName:"t3CheckpointRevert"
 required property var bridge
 required property Item popupHost
 property bool closeAfterConfirmation:false
 property bool retainingRequest:false
 property bool reconciled:false
 signal recoverRequested(var request)
 signal reconcileRequested(var request)
 property string environment:""
 property string threadId:""
 property bool canOperate:false
 property bool connected:false
 property bool supportsRollback:false
 property bool working:false
 property bool sendBusy:false
 property bool connecting:false
 property bool attachmentsPending:false
 property bool parentPending:false
 // True only when a worktree is present; server remains authoritative for isolation.
 property bool canRestoreFiles:false
 property string stage:"idle"
 property string error:""
 property var reference:({})
 property var request:({})
 property int outcomeTimeoutMs:120000
 readonly property bool opened:dialog.opened
 readonly property bool busy:stage==="sending"||stage==="waiting"||stage==="recovering"
 readonly property bool unresolved:busy||stage==="uncertain"||stage==="superseded"||stage==="recovery_failed"
 readonly property bool baseReady:canOperate&&connected&&supportsRollback&&!working&&!sendBusy&&!connecting&&!attachmentsPending
 readonly property var referenceKeys:["threadId","messageId","messageRunId","messageCheckpointId","checkpointId","checkpointRunId","scopeId","providerThreadId","providerInstanceId","turnCount","priorRollbackRequestId"]
 readonly property color fg:Color.popups.text
 signal requested(var request)
 signal completed(var request)
 signal reviewRequested(string messageId)
 signal closed()

 function clone(v){return JSON.parse(JSON.stringify(v))}
 function validReference(r){
  if(!r||r.threadId!==threadId||!environment)return false
  for(var i=0;i<referenceKeys.length;i++){
   var k=referenceKeys[i],v=r[k]
   if(k==="turnCount"){if(typeof v!=="number"||!isFinite(v)||v<0||Math.floor(v)!==v)return false}
   else if((k==="checkpointRunId"||k==="priorRollbackRequestId")&&v===null)continue
   else if(typeof v!=="string"||!v.length||v.trim()!==v||v.length>4096||/[\x00-\x1f\x7f]/.test(v))return false
  }
  return true
 }
 function sameReference(a,b){
  if(!a||!b)return false
  for(var i=0;i<referenceKeys.length;i++){var k=referenceKeys[i];if(a[k]!==b[k])return false}
  return true
 }
 function gateError(){
  if(!canOperate)return "revert_operate_required"
  if(!connected)return "revert_disconnected"
  if(!supportsRollback)return "revert_unsupported"
  if(working||sendBusy||connecting)return "revert_interrupt_first"
  if(attachmentsPending)return "revert_attachments_pending"
  return ""
 }
 function begin(r){
  if((unresolved&&opened)||parentPending)return false
  error=gateError()
  if(error)return false
  if(!validReference(r)){error="revert_reference_changed";return false}
  deadline.stop();retainingRequest=false;reference=clone(r);request={};stage="confirm";error="";dialog.open();return true
 }
 function choose(restoreFiles){
  if(stage!=="confirm"||parentPending||!validReference(reference)||typeof restoreFiles!=="boolean")return false
  error=gateError()
  if(error)return false
  if(restoreFiles&&!canRestoreFiles){error="revert_files_require_worktree";return false}
  var id=bridge.uuid()
  if(typeof id!=="string"||!/^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(id)){error="invalid_request_id";return false}
  request={requestId:id,environment:environment,thread:threadId,reference:clone(reference),confirmed:true,restoreFiles:restoreFiles}
  stage="sending";error="";deadline.restart();requested(clone(request));if(closeAfterConfirmation){retainingRequest=true;dialog.close()}return true
 }
 // Explicit identical retry only while outcome is UNKNOWN; never restart a terminal
 // failed native effect with the same deduplicated command id.
 function retry(){
  if(stage!=="uncertain"||!baseReady||!request.requestId)return false
  stage="sending";error="";deadline.restart();var replay=clone(request);replay.retry=true;requested(replay);return true
 }
 function markUncertain(){
  if(!busy&&stage!=="uncertain")return false
  deadline.stop();stage="uncertain";return true
 }
 function matchesReceipt(r){
  return !!r&&r.requestId===request.requestId&&r.environment===environment&&r.thread===threadId&&r.restoreFiles===request.restoreFiles&&sameReference(r.reference,request.reference)
 }
 // accepted = native dispatch receipt ONLY; complete/failed come from Rust observation.
 // rejected is reserved for a definitive pre-admission rejection (not a network error).
 function accept(r){
  if(!opened||!unresolved||!matchesReceipt(r)||(stage==="superseded"&&r.state!=="reviewed"&&r.state!=="superseded"))return false
  if(r.state==="recovering"){deadline.stop();stage="recovering";error="";return true}
  if(r.state==="recovery_failed"){deadline.stop();stage="recovery_failed";error=String(r.error || "");return true}
  if(r.state==="reviewed"){dismiss();return true}
  if((stage==="recovering"||stage==="recovery_failed")&&(r.state==="accepted"||r.state==="pending"))return false
  if(r.state==="accepted"){
   if(stage!=="uncertain"){stage="waiting";deadline.restart()}
   return true
  }
  if(r.state==="pending")return true
  if(r.state==="uncertain"){error=String(r.error||"");return markUncertain()}
  if(r.state==="superseded"){deadline.stop();stage="superseded";error=String(r.error||"revert_superseded");return true}
  if(r.state==="failed"||r.state==="rejected"){
   deadline.stop();stage="failed";error=String(r.error||"revert_failed");return true
  }
  if(r.state==="complete"){
   deadline.stop();stage="done";error="";completed(clone(request));return true
  }
  return false
 }
 // Rehydrate parent's persisted operation on reopen/reconnect. Never sends by itself.
 function resume(saved,status){
  if((unresolved&&opened)||!saved||saved.environment!==environment||saved.thread!==threadId||!validReference(saved.reference)||saved.confirmed!==true||typeof saved.restoreFiles!=="boolean"||typeof saved.requestId!=="string"||!/^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(saved.requestId))return false
  if(status!=="waiting"&&status!=="uncertain"&&status!=="superseded"&&status!=="recovery_failed"&&status!=="recovering")return false
  deadline.stop();retainingRequest=false;request=clone(saved);reference=clone(saved.reference);stage=status;error=status==="superseded"?"revert_superseded":"";dialog.open()
  if(status==="waiting")deadline.restart()
  return true
 }
 function review(){
  if(stage!=="failed")return false
  var messageId=reference.messageId
  dismiss();reviewRequested(messageId);return true
 }
 // Close only detaches this view. The parent MUST retain unresolved durable work,
 // its global per-thread guard and draft-recovery snapshot across navigation/restart.
 function dismiss(){
  retainingRequest=false;deadline.stop();dialog.close();stage="idle";error="";request={};reference={};reconciled=false
 }
 function errorText(code){
  code=String(code||"")
  if(code.indexOf("invalid_revert_")===0||code==="invalid_request_id"||code==="revert_confirmation_required"||code==="revert_file_choice_required")return T3I18n.tr("Invalid rewind request. Refresh the thread and try again.")
  if(code==="invalid_attachments"||code==="image_total_limit_80MiB")return T3I18n.tr("Make room for this message's attachments in the composer, then retry.")
  if(code==="draft_too_large")return T3I18n.tr("Shorten the draft, then retry restoring the message.")
  if(code==="revert_attachment_unavailable"||code==="revert_attachment_changed"||code==="upload_failed"||code==="invalid_asset_url"||code.indexOf("upload_http_")===0)return T3I18n.tr("Could not prepare this message's attachments. Refresh the thread and try again.")
  switch(code){
  case "revert_operate_required":return T3I18n.tr("You do not have permission to rewind this thread.")
  case "revert_disconnected":return T3I18n.tr("Reconnect this environment before rewinding.")
  case "revert_unsupported":return T3I18n.tr("This provider does not support reverting conversation history. Start a new thread instead.")
  case "revert_interrupt_first":return T3I18n.tr("Interrupt the current turn before reverting checkpoints.")
  case "revert_attachments_pending":return T3I18n.tr("Wait for attachments to finish preparing before rewinding.")
  case "revert_files_require_worktree":return T3I18n.tr("File restore requires an isolated worktree. Rewind the conversation without restoring files instead.")
  case "revert_superseded":return T3I18n.tr("Another rewind replaced this operation. Refresh the thread before continuing.")
  case "revert_already_pending":return T3I18n.tr("A rewind is already pending for this thread.")
  case "revert_reference_changed":
  case "revert_checkpoint_unavailable":
  case "revert_message_unavailable":
  case "revert_message_not_turn_start":
  case "revert_message_invalid":
  case "revert_scope_unavailable":
  case "revert_scope_mismatch":
  case "revert_run_unavailable":
  case "revert_provider_unavailable":
  case "revert_provider_changed":return T3I18n.tr("The checkpoint is no longer available. Refresh the thread and try again.")
  case "revert_failed":return T3I18n.tr("The thread could not be rewound.")
  default:return T3I18n.error(code)
  }
 }
 onEnvironmentChanged:dismiss()
 onThreadIdChanged:dismiss()
 Timer{id:deadline;interval:root.outcomeTimeoutMs;onTriggered:root.markUncertain()}
 Controls.Popup {
  id:dialog
  objectName:"checkpointRevertDialog"
  parent:root.popupHost
  anchors.centerIn:parent
  width:Math.min(Style.space(520),Math.max(Style.space(240),parent.width-Style.space(32)))
  padding:Style.space(20);modal:true;focus:true
  closePolicy:Controls.Popup.CloseOnEscape
  onClosed:{if(root.retainingRequest){root.retainingRequest=false;return}deadline.stop();root.stage="idle";root.error="";root.request={};root.reference={};root.closed()}
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Color.accent}
  contentItem:ColumnLayout {
   spacing:Style.space(12)
   RowLayout {
    Layout.fillWidth:true
    T3Icon{name:"undo-2";tint:Color.accent}
    Text{Layout.fillWidth:true;text:T3I18n.tr("Edit from here?");color:root.fg;font.pixelSize:Style.font.body;font.bold:true;wrapMode:Text.Wrap}
    T3IconButton{objectName:"checkpointRevertClose";name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.dismiss()}
   }
   Text{Layout.fillWidth:true;text:T3I18n.tr("Rewind chat to before this message. Your prompt and attachments return to the composer.");color:root.fg;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
   Text{Layout.fillWidth:true;visible:root.stage==="confirm"&&!root.canRestoreFiles;text:root.errorText("revert_files_require_worktree");color:Color.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   RowLayout {
    Layout.fillWidth:true
    WorkIndicator{visible:root.busy;active:root.busy}
    Text {
     objectName:"checkpointRevertStatus";Layout.fillWidth:true
     text:root.stage==="recovering"?T3I18n.tr("Restoring draft…"):root.stage==="recovery_failed"?T3I18n.tr("Draft could not be saved/restored"):root.stage==="sending"?T3I18n.tr("Sending rewind request…"):root.stage==="waiting"?T3I18n.tr("Waiting for the thread to rewind…"):root.stage==="uncertain"?T3I18n.tr("Outcome unknown. Refresh to check before retrying the same action."):root.stage==="done"?T3I18n.tr("Thread rewound."):root.stage==="failed"?T3I18n.tr("The thread could not be rewound."):""
     color:root.stage==="uncertain"||root.stage==="failed"?Color.urgent:Color.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap
    }
   }
   Text{objectName:"checkpointRevertError";Layout.fillWidth:true;visible:!!root.error;text:root.errorText(root.error);textFormat:Text.PlainText;color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   ColumnLayout{visible:root.stage==="uncertain"||root.stage==="superseded";Layout.fillWidth:true
    Controls.CheckBox{Layout.fillWidth:true;text:T3I18n.tr("I verified this operation has finished in native T3.");checked:root.reconciled;onToggled:root.reconciled=checked}
    T3PickerButton{primary:false;text:T3I18n.tr("Clear resolved operation");enabled:root.reconciled&&!root.busy;onClicked:{root.reconciled=false;root.reconcileRequested(root.clone(root.request))}}
   }
   Flow {
    Layout.fillWidth:true;spacing:Style.space(8)
    T3PickerButton{objectName:"checkpointRevertCancel";leftPadding:Style.space(10);rightPadding:leftPadding;visible:root.stage==="confirm";primary:false;text:T3I18n.tr("Cancel");onClicked:root.dismiss()}
    T3PickerButton{objectName:"checkpointRevertFiles";leftPadding:Style.space(10);rightPadding:leftPadding;visible:root.stage==="confirm";enabled:root.baseReady&&!root.parentPending&&root.canRestoreFiles;text:T3I18n.tr("Revert files too");background:Rectangle{color:Color.urgent;radius:Style.space(7)}onClicked:root.choose(true)}
    T3PickerButton{objectName:"checkpointRevertKeep";leftPadding:Style.space(10);rightPadding:leftPadding;visible:root.stage==="confirm";enabled:root.baseReady&&!root.parentPending;text:T3I18n.tr("Revert and keep changes");primary:true;onClicked:root.choose(false)}
    T3PickerButton{objectName:"checkpointRevertRefresh";visible:root.stage==="uncertain"||root.stage==="superseded";primary:false;text:T3I18n.tr("Refresh thread");enabled:root.connected;onClicked:root.refreshRequested(root.clone(root.request))}
    T3PickerButton{objectName:"checkpointRevertRetry";visible:root.stage==="uncertain";primary:false;enabled:root.baseReady;text:T3I18n.tr("Retry same action");onClicked:root.retry()}
    T3PickerButton{objectName:"checkpointRevertReview";visible:root.stage==="failed";enabled:root.baseReady&&!root.parentPending;text:T3I18n.tr("Review and retry");onClicked:root.review()}
    T3PickerButton{objectName:"checkpointRevertRecover";visible:root.stage==="recovery_failed";primary:true;text:T3I18n.tr("Retry");enabled:!root.attachmentsPending&&!root.sendBusy;onClicked:root.recoverRequested(root.clone(root.request))}
    T3PickerButton{objectName:"checkpointRevertDone";visible:root.stage==="done";text:T3I18n.tr("Done");primary:true;onClicked:root.dismiss()}
   }
  }
 }
 signal refreshRequested(var request)
}
