import QtQuick
import Quickshell
import QtTest
import "UnderTest" as Test
ShellRoot {
 id:root;property int phase:0;property int ticks:0;property bool diffCaptured:false;property var fixture:null
 function check(ok,msg){if(!ok){console.error("T3_WORK_DETAILS_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,e=panel.composerView.inputField,status=keys.findChild(panel,"t3DraftStatus")
  if(phase>0&&panel.restoringConversation)return
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one");b.transcript=[{id:"work",role:"activity",activity:{active:true,command:"sed",category:"read",count:2,entries:[{command:"rg",active:false},{command:"sed",active:true}]}},{id:"changes",role:"changes",changes:{files:[{path:"src/main.rs",kind:"modified",additions:9,deletions:2},{path:"README.md",kind:"added",additions:4,deletions:0}],additions:13,deletions:2,fromTurnCount:1,toTurnCount:2}}];panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;check(b.activityText({command:"sed",active:true})==="Ejecutando sed","specific command text");var work=panel.conversationView.itemAtIndex(0);keys.mouseClick(keys.findChild(work,"t3ActivityToggle"),10,10);check(keys.findChild(work,"t3ActivityEntries").count===2,"activity group expands specific commands");var button=keys.findChild(panel.conversationView.itemAtIndex(1),"t3OpenDiff");check(!!button,"changed files card rendered");keys.mouseClick(button,10,10);phase=2
  }else if(phase===2){var popup=keys.findChild(panel,"t3DiffPopup");check(popup.visible,"diff opens inside panel");b.diffView={loading:false,error:"",text:"diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new"};phase=3
  }else if(phase===3){check(keys.findChild(panel,"t3DiffLines").count===6,"diff lines virtualized");check(keys.findChild(panel,"t3DiffContents").grabToImage(function(r){root.diffCaptured=r.saveToFile("/tmp/omarchy-t3-diff-view.png")}),"diff capture requested");phase=30
  }else if(phase===30){if(!diffCaptured)return;keys.findChild(panel,"t3DiffPopup").close();keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/omarchy-t3-work-details.png")});e.forceActiveFocus();b.edit("typing");b.patchDraft(b.selectedKey,{saveState:"saved"});ticks=0;phase=4
  }else if(phase===4){check(status.saving,"animation survives save ACK");ticks++;if(ticks===3){b.edit("typing again");b.patchDraft(b.selectedKey,{saveState:"saved"});ticks=0;phase=5}
  }else if(phase===5){ticks++;if(ticks===3)check(status.saving,"typing resets one second animation");if(ticks>=6){check(!status.saving,"animation stops after quiet period");panel.close();console.log("T3_WORK_DETAILS_QML_PASS: specific work text, changed-file card, diff viewer, and one-second save animation reset by typing");Qt.quit()}}
 }}
}
