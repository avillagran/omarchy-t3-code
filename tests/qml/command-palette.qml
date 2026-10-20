import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("COMMAND_PALETTE_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1300,windowHeight:900});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One",project:"Alpha",projectId:"p",updatedAt:1},{id:"two",key:"fixture/two",environmentId:"fixture",title:"Refactor parser",project:"Beta",projectId:"q",updatedAt:5}],projects:[{id:"p",key:"fixture/p",title:"Alpha",environmentId:"fixture"}],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;panel.toggleCommandPalette();phase=2
  }else if(phase===2){var p=keys.findChild(panel,"t3CommandPalette");check(p&&p.opened,"palette opens");check(p.rows.some(function(r){return r.header&&r.label==="Actions"}),"actions group");check(p.rows.some(function(r){return !r.header&&r.item.id==="thread:fixture/two"}),"recent threads exclude the open one and list others");check(!p.rows.some(function(r){return !r.header&&r.item.id==="thread:fixture/one"}),"open thread is not listed")
   keys.findChild(p,"t3CommandPaletteSearch").text="refactor pars";phase=3
  }else if(phase===3){var p=keys.findChild(panel,"t3CommandPalette");check(p.runnable.length===1&&p.runnable[0].item.id==="thread:fixture/two","multi-word search finds the thread: "+p.runnable.length);p.run(p.runnable[0].item);phase=4
  }else if(phase===4){check(b.selectedKey==="fixture/two","choosing a thread opens it");check(!keys.findChild(panel,"t3CommandPalette").opened,"palette closes after running");panel.toggleCommandPalette();phase=5
  }else if(phase===5){var p=keys.findChild(panel,"t3CommandPalette");keys.findChild(p,"t3CommandPaletteSearch").text="storage";phase=6
  }else if(phase===6){var p=keys.findChild(panel,"t3CommandPalette");check(p.runnable.length>=1&&p.runnable[0].item.id==="settings:storage","settings pages are searchable");p.run(p.runnable[0].item);phase=7
  }else if(phase===7){check(panel.preferencesOpen,"settings page opens");panel.close();console.log("COMMAND_PALETTE_QML_PASS: actions, recent threads, multi-word search, run and close, settings pages");Qt.quit()}
 }}
}
