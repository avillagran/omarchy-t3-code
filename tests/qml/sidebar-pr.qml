import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property bool captured:false
 function check(ok,msg){if(!ok){console.error("SIDEBAR_PR_QML_FAIL "+msg);Qt.exit(1)}}
 readonly property var entry:({projectId:"p",host:"github.com",repository:"example/project",number:314,url:"https://github.com/example/project/pull/314",snapshot:{state:"open",title:"Example change"}})
 readonly property var badge:({kind:"single",text:"314",state:"open",current:entry,links:[entry]})
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"Example PR",branch:"feature/a-long-middle-truncated-branch",worktreePath:"/work/tree",pullRequestBadge:root.badge},{key:"local/two",id:"two",environmentId:"local",environmentName:"T3 Code",title:"No branch",branch:null},{key:"fixture/scratch",id:"scratch",environmentId:"fixture",environmentName:"node",projectKey:"fixture/scratch",title:"Scratch",branch:null}],projects:[{key:"fixture/scratch",environmentId:"fixture",title:"No project",workspaceRoot:"/native/scratch"}],environments:[{id:"fixture",state:"ready"},{id:"local",state:"ready"}]};fixture=b.state;b.catalogs={fixture:{scratchWorkspaceRoot:"/native/scratch",models:[],capabilities:{}}};b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){var row=panel.sidebarView.itemAtIndex(0),second=panel.sidebarView.itemAtIndex(1);var scratch=panel.sidebarView.itemAtIndex(2);if(!row||!second||!scratch)return;check(keys.findChild(scratch,"threadBranch").text==="node"&&keys.findChild(scratch,"threadWorkspaceIcon").visible&&!keys.findChild(scratch,"threadRemoteIcon").visible,"native scratch root inherits machine label without duplicate icon");check(keys.findChild(row,"threadBranch").elide===Text.ElideMiddle,"native middle branch truncation");check(keys.findChild(row,"threadWorkspaceIcon").visible,"worktree marker");check(keys.findChild(row,"threadRemoteIcon").visible,"remote server marker");check(keys.findChild(second,"threadBranch").text==="","local row does not replace empty branch with environment name");check(!keys.findChild(second,"threadRemoteIcon").visible&&!keys.findChild(second,"threadWorkspaceIcon").visible,"local non-worktree has no fake server/git icon");var pr=keys.findChild(row,"threadPullRequestBadge");check(pr.visible&&pr.badge.text==="314","native PR number badge");panel.sidebarView.grabToImage(function(image){root.captured=image.saveToFile("/tmp/t3-sidebar-pr.png")});phase=11
  }else if(phase===11){if(!captured)return;var pr=keys.findChild(panel.sidebarView.itemAtIndex(0),"threadPullRequestBadge");keys.mouseClick(pr,pr.width/2,pr.height/2);phase=2
  }else if(phase===2){var loader=keys.findChild(panel,"t3PullRequestsView");check(panel.pullRequestsOpen&&loader.active,"badge opens PR in chat");check(loader.item.initialEntry.number===314&&loader.item.environment==="fixture","correct PR and environment");check(loader.item.selected.number===314,"single badge selects PR details directly");panel.pullRequestsOpen=false;phase=3
  }else if(phase===3){var other=Object.assign({},entry,{number:9639,url:"https://github.com/example/project/pull/9639",snapshot:{state:"merged",title:"Merged change"}});var rows=b.state.threads.slice();rows[0]=Object.assign({},rows[0],{pullRequestBadge:{kind:"multiple",text:"+2",state:"open",current:entry,links:[entry,other]}});fixture=Object.assign({},b.state,{threads:rows});b.state=fixture;phase=4
  }else if(phase===4){var pr=keys.findChild(panel.sidebarView.itemAtIndex(0),"threadPullRequestBadge");keys.mouseClick(pr,pr.width/2,pr.height/2);phase=5
  }else if(phase===5){var view=keys.findChild(panel,"t3PullRequestsView").item;check(view.threadScoped&&view.entries.length===2,"multiple badge shows only its thread links including merged PRs");check(view.selected===null,"multiple badge opens list instead of arbitrary detail");panel.close();console.log("SIDEBAR_PR_QML_PASS: native branch/worktree/remote layout, PR number and in-app target routing");Qt.quit()}
 }}
}
