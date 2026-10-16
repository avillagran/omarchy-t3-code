import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("T3_BACKEND_START_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Timer{interval:200;repeat:true;running:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[],projects:[],environments:[{id:"local",name:"Local",state:"unreachable"}]};b.selectedEnvironment="local";b.selectedThread="fixture";b.selectedKey="local/fixture";b.patchDraft(b.selectedKey,{loading:false,text:"Keep my draft"});panel.open();phase=1
  }else if(phase===1){var button=keys.findChild(panel,"startLocalBackend");root.check(button.visible&&button.enabled,"offline local backend exposes a start button");keys.mouseClick(button,button.width/2,button.height/2);phase=2
  }else if(phase===2){if(b.backendStart.busy)return;root.check(b.backendStart.error==="t3_cli_missing","safe isolated missing-CLI result reaches UI");root.check(b.draft.text==="Keep my draft","start does not touch the prompt");panel.close();console.log("T3_BACKEND_START_QML_PASS: offline affordance, Rust start action, failure feedback and preserved draft");Qt.quit()}
 }}
}
