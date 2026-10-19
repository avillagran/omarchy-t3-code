import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("NATIVE_TERMINAL_QML_FAIL "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"},{key:"fixture/two",id:"two",environmentId:"fixture",title:"Two"}],projects:[],environments:[{id:"fixture",state:"ready"}]};b.state=fixture;b.catalogs={fixture:{canWrite:true,providers:[],models:[]}};b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("Do not lose this draft");var button=keys.findChild(panel,"t3OpenTerminal");check(button&&button.enabled,"header terminal action available");keys.mouseClick(button,12,12);check(b.terminalOperation.busy&&!button.enabled,"launch intent disables duplicate opens");b.action({op:"terminal-open",requestId:b.terminalOperation.requestId,error:"terminal_launch_failed"});check(panel.opened&&b.draft.text==="Do not lose this draft"&&b.terminalOperation.error,"launch failure preserves panel and draft");keys.mouseClick(button,12,12);var id=b.terminalOperation.requestId;b.selectThread("fixture","two");b.action({op:"terminal-open",requestId:id,result:{opened:true}});check(panel.opened,"late success for another thread cannot hide panel");b.selectThread("fixture","one");phase=2
  }else if(phase===2){if(b.draft.loading)return;panel.composerView.inputField.forceActiveFocus();var before=b.terminalOperation.requestId;keys.keyClick(Qt.Key_J,Qt.ControlModifier);check(b.terminalOperation.busy||b.terminalOperation.requestId!==before,"CtrlJ opens native terminal");b.action({op:"terminal-open",requestId:b.terminalOperation.requestId,result:{opened:true}});check(!panel.opened&&b.draft.text==="Do not lose this draft","success hides overlaid panel while preserving draft");b.catalogs={fixture:{canWrite:false,providers:[],models:[]}};check(!b.canOpenTerminal,"read-only environment cannot open shell");console.log("NATIVE_TERMINAL_QML_PASS: button, keyboard, duplicate guard, failed/late launches, saved draft and read-only access");Qt.quit()}
 }}
}
