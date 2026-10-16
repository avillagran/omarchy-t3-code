import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property string command:"";property string revision:""
 function check(ok,msg){if(!ok){console.error("T3_APPROVAL_STATE_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Timer{interval:120;repeat:true;running:true;onTriggered:{
  var b=Test.T3Backend,key="fixture/one",id="request-a"
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.selectedEnvironment="fixture";b.selectedThread="one";b.selectedKey=key;b.patchDraft(key,{loading:false,text:"Original prompt"});b.catalogs={fixture:{canWrite:true}}
   b.approvals=[{requestId:id,responseCapability:"live",options:[{decision:"accept",label:"Approve"},{decision:"decline",label:"Decline"}]}]
   check(b.draft.text==="Original prompt","answers do not replace the prompt")
   b.respondApproval("accept");root.command=b.approvalState.attempt.requestId;root.revision=b.draft.saveRevision
   check(b.approvalOperations[key+"/"+id].busy,"response persists before RPC");phase=1
  }else if(phase===1){
   if(b.approvalOperations[key+"/"+id].busy)return
   check(!!b.approvalOperations[key+"/"+id].error,"failed native response remains visible");check(b.approvalState.responseStatus==="failed"&&!!b.approvalState.responseError,"failure is durable across reloads")
   b.respondApproval("decline");check(b.approvalState.attempt.requestId===root.command&&b.approvalState.attempt.decision==="accept","retry reuses exact persisted answer and UUID");phase=2
  }else if(phase===2){
   if(b.approvalOperations[key+"/"+id].busy)return
   b.approvals=[{requestId:"request-b",responseCapability:"live",options:[{decision:"decline",label:"Decline"}]}]
   b.respondApproval("decline");root.command=b.approvalState.attempt.requestId;phase=3
  }else if(phase===3){
   if(b.approvalOperations[key+"/request-b"].busy)return
   b.action({op:"approval",environment:"fixture",thread:"one",requestId:root.command,result:{sequence:2}})
   check(b.approvalOperations[key+"/request-b"].answered&&!b.approvalOperations[key+"/request-a"].answered,"ACK is scoped to the exact request")
   b.patchDraft(key,{loading:true});b.send({op:"draft-load",key:key});phase=4
  }else if(phase===4){
   if(b.draft.loading)return
   check(b.draft.text==="Original prompt"&&b.draft.approvals[id].attempt.decision==="accept","persisted answers and original prompt reload together")
   b.approvals=[{requestId:"request-a",responseCapability:"not_resumable",questions:[]}];var old=b.approvalState.attempt.requestId;b.respondApproval("accept");check(b.approvalState.attempt.requestId===old,"unresumable request is blocked")
   console.log("T3_APPROVAL_STATE_QML_PASS: durable separate answers, save before RPC, exact retry, request-scoped ACK and reload");Qt.quit()
  }
 }}
}
