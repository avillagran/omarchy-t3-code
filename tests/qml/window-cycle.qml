import QtQuick
import QtTest
import Quickshell
import Quickshell.Io
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property bool inspected:false;property bool floating:false;property var editor:null;property int serial:0
 function check(ok,msg){if(!ok){console.error("WINDOW_CYCLE_QML_FAIL "+msg);Qt.exit(1)}}
 Process{id:inspect;command:["hyprctl","-j","clients"];stdout:StdioCollector{onStreamFinished:{var w=JSON.parse(text).find(function(w){return w.pid===Quickshell.processId&&w.title==="O3 Code"});check(!!w,"managed ot3 window exists");if(w)floating=w.floating;inspected=true}}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer{interval:250;repeat:true;running:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.selectedEnvironment="fixture";b.selectedThread="one";b.selectedKey="fixture/one";b.patchDraft(b.selectedKey,{loading:false,text:"Keep me"});panel.open();editor=panel.composerView.inputField;serial=b.requestedSerial;phase=1
  }else if(phase===1){panel.cycleWindowMode();phase=2
  }else if(phase===2){if(b.windowLayoutOperation.busy)return;inspected=false;inspect.running=true;phase=3
  }else if(phase===3){if(!inspected)return;check(panel.windowMode==="floating"&&floating,"panel to actual floating "+JSON.stringify({mode:panel.windowMode,floating:floating,error:b.windowLayoutError,pid:Quickshell.processId,managed:!!panel.managedToplevel}));panel.cycleWindowMode();phase=4
  }else if(phase===4){if(b.windowLayoutOperation.busy)return;inspected=false;inspect.running=true;phase=5
  }else if(phase===5){if(!inspected)return;check(panel.windowMode==="tiled"&&!floating,"floating to actual tiled "+JSON.stringify({mode:panel.windowMode,floating:floating,error:b.windowLayoutError,operation:b.windowLayoutOperation,attempts:panel.windowApplyAttempts}));var x=keys.findChild(panel,"t3CloseWindow"),find=keys.findChild(panel,"t3FindInChat");check(x.mapToItem(panel,0,0).x>find.mapToItem(panel,0,0).x,"close is rightmost header action");check(editor===panel.composerView.inputField&&editor.plainText==="Keep me"&&serial===b.requestedSerial,"same editor/draft/subscription across managed modes");panel.cycleWindowMode();phase=6
  }else if(phase===6){check(panel.windowMode==="panel"&&!panel.normalWindow.visible&&panel.opened,"tiled to original panel");check(b.viewState.windowMode==="panel"&&!b.viewState.detached,"mode persists");panel.close();console.log("WINDOW_CYCLE_QML_PASS: actual floating/tiled/panel cycle, rightmost close, editor and subscription retained");Qt.quit()}
 }}
}
