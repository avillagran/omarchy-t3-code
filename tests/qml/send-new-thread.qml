import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property var attempt:null
 function check(ok,msg){if(!ok){console.error("SEND_NEW_THREAD_QML_FAIL "+msg);Qt.exit(1)}}
 function saved(b,error){b.receive(JSON.stringify({type:"draft-saved",data:{key:b.selectedKey,revision:b.draft.saveRevision,error:error}}))}
 function preference(b,mode,error){b.action({op:"settings",requestId:b.workspaceOperation.requestId,result:{mode:mode,startFromOrigin:true},error:error})}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,c=panel.composerView,e=c.inputField
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[{key:"fixture/one",id:"one",projectId:"p",projectKey:"fixture/p",environmentId:"fixture",title:"Source",modelSelection:{instanceId:"codex",model:"fixture"},runtimeMode:"approval-required",branch:"main",activeRunId:"running-source"}],projects:[{key:"fixture/p",id:"p",environmentId:"fixture",title:"Repo"}],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.patchDraft(b.selectedKey,{text:"Keep this prompt",busy:false,attempt:null,loading:false,saveState:"saved"});e.forceActiveFocus();phase=2
  }else if(phase===2){
   var button=keys.findChild(c,"t3SendAndNewThread");check(button.enabled&&button.tooltipText.indexOf("Ctrl+Shift+Enter")>=0,"accessible shortcut");keys.mouseClick(button);check(b.workspaceOperation.busy&&!b.draft.attempt,"reads native default first");preference(b,null);phase=3
  }else if(phase===3){
   var dialog=keys.findChild(panel,"t3WorkspaceConfirmation");check(dialog.visible&&!b.draft.attempt&&!b.managementRequest,"confirmation precedes every mutation")
   keys.mouseClick(keys.findChild(panel,"workspaceConfirmCancel"));check(!dialog.visible&&b.draft.text==="Keep this prompt"&&!b.draft.attempt,"cancel preserves prompt without creating a thread")
   keys.mouseClick(keys.findChild(c,"t3SendAndNewThread"));preference(b,null,"rpc_outcome_unknown");check(!b.draft.attempt&&b.draft.text==="Keep this prompt","failed preference read cannot launch or overwrite settings");keys.mouseClick(keys.findChild(panel,"workspacePreferenceRetry"));preference(b,null);phase=4
  }else if(phase===4){
   keys.mouseClick(keys.findChild(panel,"workspaceConfirmIsolated"));check(b.workspaceOperation.action==="write"&&!b.draft.attempt,"approval saves native preference before launch")
   preference(b,null,"rpc_outcome_unknown");check(!b.draft.attempt&&b.draft.text==="Keep this prompt","failed preference save preserves prompt")
   keys.mouseClick(keys.findChild(panel,"workspacePreferenceRetry"));preference(b,"worktree");var a=b.draft.attempt;root.attempt=a
   check(a&&a.op==="manage"&&a.action==="new-thread"&&a.launchWithMessage===true,"one native launch carries initial prompt")
   check(a.workspaceStrategy.type==="worktree"&&a.workspaceStrategy.baseRef==="main"&&!a.workspaceStrategy.branch,"native worktree generates its own branch")
   check(a.delivery==="auto"&&a.text==="Keep this prompt","source run cannot queue this prompt")
   saved(b,"disk_full");check(!b.managementRequest&&!b.draft.busy&&b.draft.text==="Keep this prompt","failed save never launches")
   b.submit(true);check(b.draft.attempt.requestId===a.requestId,"save retry keeps identity");saved(b)
   check(b.managementRequest&&b.managementRequest.requestId===a.requestId,"launch only after durable save")
   b.action({op:"manage",environment:"fixture",requestId:a.requestId,error:"native_launch_failed"})
   check(b.draft.attempt.requestId===a.requestId&&b.managementRequest.requestId===a.requestId,"native partial failures retain recoverable launch identity")
   b.submit(true);saved(b)
   b.action({op:"manage",environment:"fixture",requestId:a.requestId,error:"rpc_outcome_unknown"})
   check(b.draft.text==="Keep this prompt"&&b.draft.attempt.requestId===a.requestId,"uncertain launch retains exact prompt and strategy")
   b.submit(true);saved(b);check(b.managementRequest.requestId===a.requestId,"retry bypasses dialog and reuses native launch ID")
   b.action({op:"manage",environment:"fixture",requestId:a.requestId,result:{action:"new-thread",threadId:"two"}})
   check(b.selectedKey==="fixture/two"&&b.drafts["fixture/one"].text===""&&!b.drafts["fixture/one"].attempt,"native acknowledgement clears source and opens destination")
   check(!b.draft.attempt,"no separate send can race native workspace preparation")
   b.selectThread("fixture","one");b.patchDraft(b.selectedKey,{text:"Keyboard prompt",busy:false,attempt:null});e.forceActiveFocus();phase=5
  }else if(phase===5){keys.keyClick(Qt.Key_Return,Qt.ControlModifier|Qt.ShiftModifier);preference(b,"worktree");phase=6
  }else if(phase===6){
   check(!keys.findChild(panel,"t3WorkspaceConfirmation").visible&&b.draft.attempt.workspaceStrategy.type==="worktree","next keyboard launch uses saved native default without another dialog")
   check(b.draft.attempt.workspaceStrategy.startFromOrigin===true,"native origin default applied")
   saved(b,"disk_full");b.patchDraft(b.selectedKey,{attempt:null,busy:false});b.managementRequest=null;b.managementBusy=false
   b.requestNewThread({action:"new-thread",environment:"fixture",projectId:"p",title:"Header thread",modelSelection:{instanceId:"codex",model:"fixture"},branch:"main",worktreePath:"/existing"});preference(b,"worktree")
   check(!keys.findChild(panel,"t3WorkspaceConfirmation").visible&&b.managementRequest.workspaceStrategy.type==="worktree","header also skips confirmation for saved default")
   b.managementRequest=null;b.managementBusy=false
   b.requestNewThread({action:"new-thread",environment:"fixture",projectId:"p",title:"Override",modelSelection:{instanceId:"codex",model:"fixture"},branch:"main",worktreePath:"/existing"});preference(b,"local")
   check(b.managementRequest.workspaceStrategy.type==="existing_worktree"&&!keys.findChild(panel,"t3WorkspaceConfirmation").visible,"native project override is honored without asking")
   b.managementRequest=null;b.managementBusy=false
   b.requestNewThread({action:"new-thread",environment:"fixture",projectId:"p",title:"Explicit local",modelSelection:{instanceId:"codex",model:"fixture"},workspaceStrategy:{type:"root"},automatic:true})
   check(!b.workspaceOperation.busy&&b.managementRequest.workspaceStrategy.type==="root"&&!b.managementRequest.automatic&&!keys.findChild(panel,"t3WorkspaceConfirmation").visible,"native New Local explicitly uses root without changing saved worktree preference")
   b.managementRequest=null;b.managementBusy=false;panel.close();console.log("SEND_NEW_THREAD_QML_PASS: one-time native preference approval/cancel/failure, automatic subsequent worktrees, overrides and exact launch retries");Qt.quit()
  }
 }}
}
