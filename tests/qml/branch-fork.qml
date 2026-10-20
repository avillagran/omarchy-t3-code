import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property var sent:[]
 function check(ok,msg){if(!ok){console.error("BRANCH_FORK_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1300,windowHeight:900});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One",projectId:"p",branch:"main"},{id:"parent",key:"fixture/parent",environmentId:"fixture",title:"Parent",projectId:"p"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.messagesAboutToChange();b.transcript=[{id:"u",role:"user",text:"hi",createdAt:1},{id:"a",role:"assistant",text:"done",forkRunId:"run-1",createdAt:2},{id:"bot",role:"user",text:"daily",attribution:"automation",createdAt:3}];b.messagesUpdated();phase=2
  }else if(phase===2){if(panel.restoringConversation)return
   var trigger=keys.findChild(panel,"t3BranchTrigger");check(trigger&&trigger.visible&&trigger.text==="main","composer shows the thread branch");
   var fork=keys.findChild(panel.conversationView.itemAtIndex(1),"t3ForkFromHere");check(fork&&fork.visible,"completed responses offer a fork");
   check(!keys.findChild(panel.conversationView.itemAtIndex(0),"t3ForkFromHere").visible,"user messages do not");
   var who=keys.findChild(panel.conversationView.itemAtIndex(2),"t3MessageAttribution");check(who&&who.visible&&who.text.length>0,"automation messages say who sent them");
   check(panel.composerView.promptHistory.indexOf("daily")<0&&panel.composerView.promptHistory.indexOf("hi")>=0,"prompt recall skips automation messages");
   check(!keys.findChild(panel,"t3MergeBack").visible,"merge back hidden outside forks");
   keys.findChild(panel,"t3BranchPicker").open();phase=3
  }else if(phase===3){var popup=keys.findChild(panel,"t3BranchPopup");check(popup&&popup.opened,"branch popup opens");check(!!b.branchRefs.requestId,"opening requests refs")
   b.action({op:"branch-refs",requestId:b.branchRefs.requestId,environment:"fixture",thread:"one",result:{refs:[{name:"main",current:true,isDefault:true},{name:"feature",current:false,isDefault:false},{name:"origin/remote-only",isRemote:true,remoteName:"origin",current:false,isDefault:false}]}});phase=4
  }else if(phase===4){var rows=keys.findChild(panel,"t3BranchRows");check(rows.count===3,"refs listed: "+rows.count);var search=keys.findChild(panel,"t3BranchSearch");search.text="new idea";check(keys.findChild(panel,"t3BranchCreate").visible,"typing a new name offers to create it");keys.findChild(panel,"t3BranchPicker").choose(b.branchRefs.refs[2]);phase=5
  }else if(phase===5){check(b.branchRefs.switching&&b.branchRefs.pendingBranch==="remote-only","switching a remote ref names its local branch");check(keys.findChild(panel,"t3BranchTrigger").text==="remote-only","trigger shows the pending branch")
   b.action({op:"branch-switch",requestId:b.branchRefs.switchRequestId,environment:"fixture",thread:"one",error:"git_failed"});phase=6
  }else if(phase===6){check(!b.branchRefs.switching&&b.branchRefs.error==="git_failed","failures are kept");check(keys.findChild(panel,"t3BranchTrigger").text==="main","failed switch shows the real branch")
   b.selectedWorkflow={key:b.selectedKey,target:"parent",runId:"run-9"};phase=7
  }else if(phase===7){var merge=keys.findChild(panel,"t3MergeBack");check(merge.visible&&merge.enabled,"forks offer merge back");check(b.mergeBackTitle==="Parent","merge back names its target");panel.close();console.log("BRANCH_FORK_QML_PASS: branch picker lists, creates and switches; fork and merge-back actions; sender attribution and recall");Qt.quit()}
 }}
}
