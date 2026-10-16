import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("CONTENT_PANELS_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:250;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"},{key:"fixture/two",id:"two",environmentId:"fixture",title:"Two"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("Preserved draft");panel.pullRequestsOpen=true;phase=2
  }else if(phase===2){var area=keys.findChild(panel,"t3MainChatArea"),pr=keys.findChild(panel,"t3PullRequestsView");check(pr.parent===area,"PR view belongs to chat area");check(pr.active&&pr.visible,"PR view visible in chat");check(keys.findChild(panel,"t3SidebarFooter").visible,"sidebar stays visible while viewing PRs");check(pr.width>panel.sidebarView.width,"PR receives conversation width");b.selectThread("fixture","two");phase=3
  }else if(phase===3){check(!panel.pullRequestsOpen,"selecting chat returns from PR");check(b.drafts["fixture/one"].text==="Preserved draft","PR retains chat draft");panel.close();console.log("CONTENT_PANELS_QML_PASS: PR in conversation area, sidebar retained, width, navigation and draft");Qt.quit()}
 }}
}
