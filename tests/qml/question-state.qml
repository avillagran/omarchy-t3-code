import QtQuick
import Quickshell
import "UnderTest" as Test
ShellRoot{
 id:root;property int phase:0;property string command:"";property string revision:""
 function check(ok,msg){if(!ok){console.error("T3_QUESTION_STATE_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Timer{interval:120;repeat:true;running:true;onTriggered:{
  var b=Test.T3Backend,key="fixture/one",id="request-a"
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.selectedEnvironment="fixture";b.selectedThread="one";b.selectedKey=key;b.patchDraft(key,{loading:false,text:"Original prompt"})
   b.questions=[{requestId:id,responseCapability:"message",dismissible:true,questions:[{id:"q",options:[]}]}]
   b.updateQuestion({index:0,answers:{q:{custom:"My answer"}}});check(b.draft.text==="Original prompt","answers do not replace the prompt")
   b.respondQuestion("respond",{q:"My answer"});root.command=b.questionState.attempt.requestId;root.revision=b.draft.saveRevision
   check(b.questionOperations[key+"/"+id].busy,"response persists before RPC");phase=1
  }else if(phase===1){
   if(b.questionOperations[key+"/"+id].busy)return
   check(!!b.questionOperations[key+"/"+id].error,"failed native response remains visible");check(b.questionState.responseStatus==="failed"&&!!b.questionState.responseError,"failure is durable across reloads")
   b.respondQuestion("respond",{q:"Changed"});check(b.questionState.attempt.requestId===root.command&&b.questionState.attempt.answers.q==="My answer","retry reuses exact persisted answer and UUID");phase=2
  }else if(phase===2){
   if(b.questionOperations[key+"/"+id].busy)return
   b.questions=[{requestId:"request-b",responseCapability:"message",dismissible:true,questions:[{id:"q2",options:[]}]}]
   b.respondQuestion("dismiss",{});root.command=b.questionState.attempt.requestId;phase=3
  }else if(phase===3){
   if(b.questionOperations[key+"/request-b"].busy)return
   b.action({op:"question",environment:"fixture",thread:"one",requestId:root.command,result:{sequence:2}})
   check(b.questionOperations[key+"/request-b"].answered&&!b.questionOperations[key+"/request-a"].answered,"ACK is scoped to the exact request")
   b.patchDraft(key,{loading:true});b.send({op:"draft-load",key:key});phase=4
  }else if(phase===4){
   if(b.draft.loading)return
   check(b.draft.text==="Original prompt"&&b.draft.questionnaires[id].answers.q.custom==="My answer","persisted answers and original prompt reload together")
   b.questions=[{requestId:"request-a",responseCapability:"not_resumable",questions:[]}];var old=b.questionState.attempt.requestId;b.respondQuestion("respond",{q:"no"});check(b.questionState.attempt.requestId===old,"unresumable request is blocked")
   console.log("T3_QUESTION_STATE_QML_PASS: durable separate answers, save before RPC, exact retry, request-scoped ACK and reload");Qt.quit()
  }
 }}
}
