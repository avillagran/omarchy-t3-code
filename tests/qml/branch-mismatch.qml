import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("BRANCH_MISMATCH_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1300,windowHeight:900});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One",projectId:"p",branch:"main"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.branchMismatch={key:b.selectedKey,requestId:"r1"};b.action({op:"branch-check",requestId:"r1",result:{mismatch:{threadBranch:"main",currentBranch:"dev",canRestore:true}}});phase=2
  }else if(phase===2){var bar=keys.findChild(panel,"t3BranchMismatch");check(bar&&bar.visible,"notice shows");check(keys.findChild(panel,"t3BranchRestore").visible,"local checkout can restore");keys.findChild(panel,"t3BranchAdopt").clicked();check(b.branchMismatch.busy,"adopt sends");b.action({op:"branch-resolve",requestId:b.branchMismatch.requestId,result:{mismatch:null,branch:"dev"}});phase=3
  }else if(phase===3){check(!keys.findChild(panel,"t3BranchMismatch").visible,"notice clears after adopting");b.branchMismatch={key:b.selectedKey,requestId:"r2"};b.action({op:"branch-check",requestId:"r2",result:{mismatch:{threadBranch:"t3code/x",currentBranch:"feature",canRestore:false}}});phase=4
  }else if(phase===4){check(keys.findChild(panel,"t3BranchMismatch").visible&&!keys.findChild(panel,"t3BranchRestore").visible,"worktree threads only warn");b.branchMismatch=Object.assign({},b.branchMismatch,{dismissed:true});check(!keys.findChild(panel,"t3BranchMismatch").visible,"dismiss hides");var bar=keys.findChild(panel,"t3SubagentBar");check(bar&&!bar.visible,"no subagent bar when idle");b.activeSubagents=[{id:"n1",runId:"r1",title:"Review tests",status:"running",childThreadId:null}];check(bar.visible,"running subagents show");b.stopSubagent(b.activeSubagents[0]);check(b.managementRequest&&b.managementRequest.action==="stop-subagent"&&b.managementRequest.subagentId==="n1","stop sends subagent.stop");var wt=keys.findChild(panel,"t3WorktreeSetup");check(wt&&!wt.visible,"no setup card when idle");b.receive(JSON.stringify({type:"worktree-setup",data:{environment:"fixture",thread:"one",snapshot:{phase:"running",branch:"t3/abc",stages:[{id:"fetch",status:"done",percent:null,detail:null},{id:"checkout",status:"running",percent:40,detail:null}],error:null}}}));check(wt.visible,"running setup shows progress");b.activeSubagents=[{id:"w1",runId:null,title:"Review",status:"completed",workflow:true,scriptPath:"/tmp/wf.js"}];check(keys.findChild(panel,"t3SubagentBar").visible,"finished workflows stay listed for their script");b.openWorkflowScript(b.activeSubagents[0]);check(keys.findChild(panel,"t3WorkflowScriptPopup").opened&&b.workflowScript.busy,"script viewer opens and loads");b.action({op:"workflow-script",requestId:b.workflowScript.requestId,result:{scriptPath:"/tmp/wf.js",contents:"export default 1",truncated:false}});check(keys.findChild(panel,"t3WorkflowScriptText").text==="export default 1","script shown");panel.close();console.log("BRANCH_MISMATCH_QML_PASS");Qt.quit()}
 }}
}
