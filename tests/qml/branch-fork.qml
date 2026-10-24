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
  }else if(phase===1){if(b.draft.loading)return;b.messagesAboutToChange();b.transcript=[{id:"u",role:"user",text:"hi",createdAt:1},{id:"a",role:"assistant",text:"done",forkRunId:"run-1",createdAt:2},{id:"bot",role:"user",text:"daily",attribution:"automation",createdAt:3},{id:"peer",role:"user",text:"\nReview the parser\nthen report",attribution:"agent",senderThreadId:"parent",createdAt:4}];b.messagesUpdated();phase=2
  }else if(phase===2){if(panel.restoringConversation)return
   var trigger=keys.findChild(panel,"t3BranchTrigger");check(trigger&&trigger.visible&&trigger.text==="main","composer shows the thread branch");
   var fork=keys.findChild(panel.conversationView.itemAtIndex(1),"t3ForkFromHere");check(fork&&fork.visible,"completed responses offer a fork");
   check(!keys.findChild(panel.conversationView.itemAtIndex(0),"t3ForkFromHere").visible,"user messages do not");
   var bot=panel.conversationView.itemAtIndex(2),peer=panel.conversationView.itemAtIndex(3)
   check(!keys.findChild(bot,"t3MessageAttribution").visible&&keys.findChild(bot,"t3FoldedMessageHeader").visible&&keys.findChild(bot,"t3FoldedMessageLabel").text===Test.T3I18n.tr("Automation"),"automation messages fold to one line that says who sent them");
   check(!keys.findChild(bot,"t3MessageBody").visible&&!keys.findChild(bot,"t3MessageActions").visible,"folded messages hide their body and actions");
   check(keys.findChild(peer,"t3FoldedMessageLabel").text==="Parent"&&keys.findChild(peer,"t3FoldedMessageSummary").text==="Review the parser","agent messages fold under the sending thread's title and first line");
   check(!keys.findChild(panel.conversationView.itemAtIndex(0),"t3FoldedMessageHeader").visible,"the user's own messages never fold");
   panel.toggleFold("peer");check(keys.findChild(peer,"t3MessageBody").visible&&keys.findChild(peer,"t3OpenSenderThread").visible,"an opened fold shows the message and a link to the sending thread");panel.toggleFold("peer");
   check(panel.composerView.promptHistory.indexOf("daily")<0&&panel.composerView.promptHistory.indexOf("hi")>=0,"prompt recall skips automation messages");
   check(!keys.findChild(panel,"t3MergeBack").visible,"merge back hidden outside forks");
   keys.findChild(panel,"t3BranchPicker").open();phase=3
  }else if(phase===3){var popup=keys.findChild(panel,"t3BranchPopup");check(popup&&popup.opened,"branch popup opens");check(!!b.branchRefs.requestId,"opening requests refs")
   b.action({op:"branch-refs",requestId:b.branchRefs.requestId,environment:"fixture",thread:"one",result:{refs:[{name:"main",current:true,isDefault:true},{name:"feature",current:false,isDefault:false},{name:"origin/remote-only",isRemote:true,remoteName:"origin",current:false,isDefault:false}]}});phase=4
  }else if(phase===4){var rows=keys.findChild(panel,"t3BranchRows");check(rows.count===3,"refs listed: "+rows.count);var search=keys.findChild(panel,"t3BranchSearch");search.text="new idea";check(keys.findChild(panel,"t3BranchCreate").visible,"typing a new name offers to create it");keys.findChild(panel,"t3BranchPicker").choose(b.branchRefs.refs[2]);check(b.branchRefs.switching&&b.branchRefs.pendingBranch==="remote-only","switching a remote ref names its local branch");check(keys.findChild(panel,"t3BranchTrigger").text==="remote-only","trigger shows the pending branch")
   b.action({op:"branch-switch",requestId:b.branchRefs.switchRequestId,environment:"fixture",thread:"one",error:"git_failed"});phase=6
  }else if(phase===6){check(!b.branchRefs.switching&&!!b.branchRefs.error,"failures are kept");check(keys.findChild(panel,"t3BranchTrigger").text==="main","failed switch shows the real branch")
   b.selectedWorkflow={key:b.selectedKey,target:"parent",runId:"run-9"};phase=7
  }else if(phase===7){var merge=keys.findChild(panel,"t3MergeBack");check(merge.visible&&merge.enabled,"forks offer merge back");check(b.mergeBackTitle==="Parent","merge back names its target");var te=keys.findChild(panel.conversationView.itemAtIndex(1),"chatMessageText");te.selectAll();var q=keys.findChild(panel.conversationView.itemAtIndex(1),"t3QuoteSelection");check(q&&q.visible,"selecting a response offers Quote");q.clicked();check(String(b.draft.text).indexOf("> done")===0,"quote goes to the composer: "+b.draft.text);var meter=keys.findChild(panel,"t3ComposerContext");check(meter&&!meter.visible,"no context meter without usage");b.contextWindow={usedTokens:45000,maxTokens:200000,usedPercentage:22.5,compactsAutomatically:true};check(meter.visible&&meter.summary.indexOf("45k")>=0&&meter.summary.indexOf("200k")>=0,"context meter summarizes usage: "+meter.summary);panel.close();console.log("BRANCH_FORK_QML_PASS: branch picker lists, creates and switches; fork and merge-back actions; context meter; sender attribution and recall");Qt.quit()}
 }}
}
