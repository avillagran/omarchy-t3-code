import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("SIDEBAR_NAVIGATION_QML_FAIL: "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:150;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({sidebarCompact:true,workingExpanded:false,snoozedExpanded:false,settledExpanded:false});var rows=[];for(var i=0;i<4;i++)rows.push({id:"t"+i,key:"fixture/t"+i,environmentId:"fixture",title:"Thread "+i,project:"Project",working:i===1,snoozed:i===2,settled:i===3});b.state={threads:rows,projects:[],environments:[]};fixture=b.state;b.selectedKey="fixture/t0";b.selectedEnvironment="fixture";b.selectedThread="t0";panel.open();phase=1
  }else if(phase===1){panel.composerView.inputField.forceActiveFocus();keys.keyClick(Qt.Key_K,Qt.ControlModifier);phase=2
  }else if(phase===2){var list=keys.findChild(panel,"threadList");check(list.activeFocus,"Ctrl+K focuses inbox");keys.keyClick(Qt.Key_Down);check(keys.findChild(panel,"workingShelfToggle").activeFocus,"down reaches closed Working header "+JSON.stringify({index:list.currentIndex,working:panel.workingRows.length,active:panel.activeRows.length,grouped:panel.sidebarCompact}));keys.keyClick(Qt.Key_Right);phase=3
  }else if(phase===3){check(panel.workingExpanded,"right expands Working");keys.keyClick(Qt.Key_Down);check(keys.findChild(panel,"workingThreadList").activeFocus,"down enters Working");keys.keyClick(Qt.Key_Down);check(keys.findChild(panel,"snoozedShelfToggle").activeFocus,"down reaches Snoozed");keys.keyClick(Qt.Key_Down);check(keys.findChild(panel,"settledShelfToggle").activeFocus,"closed Snoozed skips its rows");keys.keyClick(Qt.Key_Right);phase=4
  }else if(phase===4){keys.keyClick(Qt.Key_Down);check(keys.findChild(panel,"settledThreadList").activeFocus,"down enters Settled");keys.keyClick(Qt.Key_Up);check(keys.findChild(panel,"settledShelfToggle").activeFocus,"up returns to header");keys.keyClick(Qt.Key_Left);check(!panel.settledExpanded,"left collapses");keys.keyClick(Qt.Key_Up);keys.keyClick(Qt.Key_Up);check(keys.findChild(panel,"workingThreadList").activeFocus,"up crosses section boundary");b.selectedKey="fixture/t3";panel.composerView.inputField.forceActiveFocus();keys.keyClick(Qt.Key_K,Qt.ControlModifier);check(keys.findChild(panel,"settledShelfToggle").activeFocus&&!panel.settledExpanded,"Ctrl+K targets selected chat closed header without expanding");console.log("SIDEBAR_NAVIGATION_QML_PASS: Ctrl+K, arrows across lists/closed headers, expand/collapse");panel.close();Qt.quit()}
 }}
}
