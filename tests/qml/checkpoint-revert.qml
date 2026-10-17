import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test

// Author-only fixture: parent runs serially after registering qmldir/test runner.
// No backend, live thread, draft, filesystem mutation or RPC is used.
ShellRoot {
 id:root
 property int phase:0
 property int sends:0
 property int completions:0
 property int reviews:0
 property int recoveries:0
 property int nextId:0
 property var sent:({})
 property var original:({})
 property int uuidCalls:0
 property string failedId:""
 function check(ok,message){if(!ok){console.error("CHECKPOINT_REVERT_QML_FAIL "+message);Qt.exit(1)}}
 function ref(){return {threadId:"thread:one",messageId:"message:two",messageRunId:"run:two",messageCheckpointId:"checkpoint:two",checkpointId:"checkpoint:one",checkpointRunId:"run:one",scopeId:"scope:one",providerThreadId:"provider:one",providerInstanceId:"codex",turnCount:1,priorRollbackRequestId:null}}
 function receipt(state,extra){return Object.assign({},sent,{state:state,error:""},extra || {})}
 QtObject{id:bridge;function uuid(){root.uuidCalls++;return ["b3e86a43-744c-46ec-b5f8-65ae79316907","e1fc833f-4d94-407f-8e3e-037e2a66936a","cfc123b6-7f17-4086-8e9e-287b782af91a"][root.uuidCalls-1]}}
 FloatingWindow{width:900;height:650;visible:true;Item{id:host;anchors.fill:parent}}
 TestCase{id:probe;when:false}
 Test.T3CheckpointRevert {
  id:workflow
  bridge:bridge;popupHost:host;environment:"fixture";threadId:"thread:one"
  canOperate:true;connected:true;supportsRollback:true;canRestoreFiles:false
  onRequested:function(request){root.sends++;root.sent=JSON.parse(JSON.stringify(request))}
  onCompleted:function(request){root.completions++}
  onRecoverRequested:function(request){root.recoveries++}
  onReviewRequested:function(messageId){root.reviews++}
 }
 Timer {
  interval:150;running:true;repeat:true
  onTriggered:{
   if(phase===0){
    check(sends===0,"construction does not mutate")
    workflow.working=true;check(!workflow.begin(ref()),"active turn blocks confirmation")
    workflow.working=false
    check(workflow.begin(ref())&&workflow.stage==="confirm","explicit trigger shows confirmation")
    check(sends===0&&uuidCalls===0,"opening confirmation does not allocate or send")
    check(!workflow.choose(true),"file restore blocked for shared workspace")
    check(workflow.choose(false),"explicit keep-changes confirmation")
    check(sends===1&&sent.confirmed===true&&sent.restoreFiles===false,"concrete explicit command choice")
    original=JSON.parse(JSON.stringify(sent))
    check(!workflow.choose(false)&&!workflow.begin(ref())&&!workflow.retry(),"duplicate mutation guard")
    check(!workflow.accept(receipt("accepted",{environment:"other"})),"other environment receipt rejected")
    check(!workflow.accept(receipt("accepted",{thread:"other"})),"other thread receipt rejected")
    check(!workflow.accept(receipt("accepted",{reference:Object.assign({},sent.reference,{scopeId:"other"})})),"other checkpoint scope rejected")
    check(workflow.accept(receipt("accepted"))&&workflow.stage==="waiting"&&completions===0,"acknowledgement is not completion")
    check(workflow.markUncertain()&&workflow.stage==="uncertain","timeout has uncertain outcome")
    check(workflow.retry()&&sends===2&&uuidCalls===1,"explicit retry reuses exact native command id")
    var nativePayload=JSON.parse(JSON.stringify(sent));delete nativePayload.retry;check(sent.retry===true&&JSON.stringify(nativePayload)===JSON.stringify(original),"explicit local retry flag preserves native payload")
    check(workflow.accept(receipt("complete"))&&completions===1&&workflow.stage==="done","native read-back completes")
    check(!workflow.accept(receipt("accepted"))&&completions===1,"late ack never regresses success")
    phase=1
   }else if(phase===1){
    check(!!probe.findChild(workflow,"checkpointRevertStatus"),"status control exists")
    check(workflow.begin(ref()),"new confirmed operation allowed after terminal success")
    workflow.canRestoreFiles=true
    check(workflow.choose(true)&&sent.restoreFiles===true,"files choice explicit")
    failedId=sent.requestId
    check(workflow.accept(receipt("failed",{error:"<b>provider refused</b>"})),"native async failure terminal")
    check(!workflow.retry(),"failed command id cannot restart deduplicated effect")
    check(workflow.review()&&reviews===1,"failed action requires fresh review and confirmation")
    check(workflow.begin(ref())&&workflow.choose(false),"fresh confirmed operation")
    check(sent.requestId!==failedId&&uuidCalls===3,"terminal failure review uses a new native command id")
    var late=receipt("complete")
    workflow.threadId="another"
    check(!workflow.opened&&!workflow.accept(late),"navigation closes and rejects late visual results")
    workflow.threadId="thread:one";workflow.canOperate=false
    check(!workflow.begin(ref()),"operate permission fail closed")
    workflow.canOperate=true;workflow.attachmentsPending=true
    check(!workflow.begin(ref()),"attachment preflight fail closed")
    workflow.attachmentsPending=false
    check(!workflow.begin({messageId:"not-a-checkpoint"}),"incomplete reference is not guessed")
    check(workflow.resume(original,"uncertain"),"parent may restore durable unresolved operation without sending")
    check(sends===4,"resume never sends automatically")
    sent=JSON.parse(JSON.stringify(original))
    check(workflow.accept(receipt("recovering"))&&workflow.stage==="recovering","verified native completion starts only local recovery")
    check(workflow.accept(receipt("recovery_failed",{error:"invalid_attachments"}))&&!workflow.retry(),"recovery failure cannot replay rollback")
    probe.findChild(workflow,"checkpointRevertRecover").clicked();check(recoveries===1&&sends===4,"retry restores draft without another native command")
    check(workflow.accept(receipt("complete")),"local recovery completion closes durable phase")
    workflow.dismiss()
    check(!workflow.accept(Object.assign({},original,{state:"complete"})),"closed dialog rejects late result")
    check(workflow.resume(original,"uncertain"),"can reattach original unresolved operation")
    sent=JSON.parse(JSON.stringify(original))
    check(workflow.accept(receipt("superseded"))&&workflow.stage==="superseded","newer native operation blocks replay")
    check(!workflow.retry()&&!workflow.markUncertain()&&!workflow.accept(receipt("accepted")),"late ack cannot reenable superseded retry")
    check(workflow.accept(receipt("reviewed"))&&!workflow.opened,"explicit manual reconciliation detaches superseded operation")
    workflow.dismiss()
    console.log("CHECKPOINT_REVERT_QML_PASS: explicit confirmation, file choices, scope, duplicate guard, ack/readback, uncertain retry identity, async failure, navigation and durable resume")
    Qt.quit()
   }
  }
 }
 Timer{interval:7000;running:true;onTriggered:{console.error("CHECKPOINT_REVERT_QML_FAIL timeout");Qt.exit(1)}}
}
