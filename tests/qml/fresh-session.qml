import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var fixture:null
 function check(ok,message){if(!ok){console.error("FRESH_SESSION_QML_FAIL "+message);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:200;repeat:true;running:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One",activeRunId:"running",projectId:"p",modelSelection:{instanceId:"codex",model:"m"}},{id:"two",key:"fixture/two",environmentId:"fixture",title:"Two",projectId:"p"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){
   if(b.draft.loading)return
   b.catalogs={fixture:{canWrite:true,freshContext:false,models:[],providers:[]}}
   b.transcript=[{id:"old",role:"user",text:"Earlier message",createdAt:1}]
   check(!b.freshSessionAvailable,"legacy server gate")
   b.catalogs={fixture:{canWrite:true,freshContext:true,models:[],providers:[]}}
   check(b.freshSessionCommandAvailable,"capability and history enable clear")
   b.requestSuggestions("command","cl","codex")
   b.action({op:"suggest",environment:"fixture",thread:"one",requestId:b.suggestionRequest,result:{items:[]}})
   check(b.suggestionItems.length===1&&b.suggestionItems[0].label==="/clear","built-in command appears in completion")
   b.edit("/cl");panel.composerView.completion={kind:"command",query:"cl",start:0,end:3}
   check(panel.composerView.chooseCompletion(0),"clear completion is selectable")
   check(b.freshSessionArmed&&!b.draft.text&&!b.draft.attempt,"selecting clear arms directly without sending")
   b.disarmFreshSession(b.selectedKey)
   b.edit("/CLEAR ");b.submit()
   check(b.freshSessionArmed&&!b.draft.text&&!b.draft.attempt,"clear prepares without dispatch")
   check(b.transcript.length===1,"history stays visible")
   check(keys.findChild(panel,"t3FreshSessionBanner").visible,"banner visible")
   check(b.dispatchMode(false)==="queue"&&b.dispatchMode(true)==="queue","fresh message never steers")
   var arm=b.freshSessionArms[b.selectedKey];b.disarmFreshSession(b.selectedKey,"stale")
   check(b.freshSessionArms[b.selectedKey]===arm,"stale acknowledgement cannot consume intent")
   keys.findChild(panel,"t3KeepCurrentSession").clicked()
   check(!b.freshSessionArmed,"undo keeps current session")
   b.edit("/clear");b.submit();b.edit("New task");b.submit()
   var attempt=b.draft.attempt
   check(attempt&&attempt.freshContext&&attempt.delivery==="queue","next send carries fresh context and queues")
   b.action({op:"send",environment:"fixture",thread:"one",requestId:attempt.requestId,error:"rpc_outcome_unknown"})
   check(b.draft.attempt.requestId===attempt.requestId&&b.draft.attempt.freshContext&&b.freshSessionArmed,"uncertain send retains same intent")
   b.armFreshSession();var newer=b.freshSessionArms[b.selectedKey]
   b.action({op:"send",environment:"fixture",thread:"one",requestId:attempt.requestId,result:{}})
   check(b.freshSessionArms[b.selectedKey]===newer,"late acceptance preserves a newer arm")
   b.edit("Retry task");b.submit();attempt=b.draft.attempt
   b.action({op:"send",environment:"fixture",thread:"one",requestId:attempt.requestId,error:"disconnected"})
   check(!b.draft.attempt&&b.freshSessionArmed&&b.draft.text==="Retry task","failed send preserves text and arm")
   b.submit();attempt=b.draft.attempt
   b.action({op:"send",environment:"fixture",thread:"one",requestId:attempt.requestId,result:{}})
   check(!b.freshSessionArmed&&!b.draft.text,"acceptance consumes matching arm")
   b.armFreshSession();b.selectThread("fixture","two");phase=2
  }else if(phase===2){
   if(b.draft.loading)return
   check(!b.freshSessionArms[b.selectedKey],"intent is thread scoped")
   b.selectThread("fixture","one");phase=3
  }else if(phase===3){
   if(b.draft.loading)return
   b.transcript=[{id:"old",role:"user",text:"Earlier message"}]
   check(b.freshSessionArmed,"switching back restores intent")
   b.approvals=[{id:"pending"}];check(!b.freshSessionCommandAvailable,"approval owns composer");b.approvals=[]
   b.catalogs={fixture:{canWrite:true,freshContext:false,models:[],providers:[]}}
   check(!b.freshSessionArmed&&!keys.findChild(panel,"t3FreshSessionBanner").visible,"capability loss hides unsupported intent")
   b.catalogs={fixture:{canWrite:true,freshContext:true,models:[],providers:[]}};b.transcript=[]
   check(!b.freshSessionAvailable,"no clear before first message")
   panel.close();console.log("FRESH_SESSION_QML_PASS: capability, history, banner, undo, queue, retries, arm races and thread scope");Qt.quit()
  }
 }}
 Timer{interval:10000;running:true;onTriggered:{console.error("FRESH_SESSION_QML_FAIL timeout at "+root.phase);Qt.exit(1)}}
}
