import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/NativeKeybindings.js" as Native
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("SIDEBAR_KEYBINDINGS_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)});throw new Error(msg)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;parent:panel.composerView.inputField;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   var config={data:{keybindings:Native.nativeKeys.compileResolvedKeybindingsConfig([{key:"mod+b",command:"sidebar.toggle"}])}}
   b.keybindingConfigs={"":config,fixture:config}
   b.rememberView({animationsEnabled:false,sidebarCompact:true,workingExpanded:false,snoozedExpanded:false,settledExpanded:false,windowMode:"floating",windowWidth:1200,windowHeight:760})
   var rows=[];for(var i=0;i<22;i++)rows.push({key:"fixture/t"+i,id:"t"+i,environmentId:"fixture",title:"Thread "+i,updatedAt:1000+i,working:i===20,settled:i===21})
   fixture={threads:rows,projects:[],environments:[]};b.state=fixture;b.sidebarCollapsed=false;b.selectThread("fixture","t18");panel.open();phase=1
  }else if(phase===1){
   if(b.draft.loading)return
   b.edit("Borrador con tildes: áéíóú");panel.composerView.inputField.forceActiveFocus();panel.composerView.inputField.selectAll()
   keys.keyClick(Qt.Key_B,Qt.ControlModifier);check(panel.sidebarCollapsed,"Ctrl+B hides even with selected editor text")
   keys.keyClick(Qt.Key_B,Qt.ControlModifier);check(!panel.sidebarCollapsed,"Ctrl+B shows")
   panel.composerView.inputField.forceActiveFocus();keys.keyClick(Qt.Key_B,Qt.MetaModifier);phase=2
  }else if(phase===2){
   var list=keys.findChild(panel,"threadList")
   check(list.activeFocus&&panel.activeRows[list.currentIndex].key==="fixture/t18","Super+B focuses current chat")
   check(list.itemAtIndex(list.currentIndex).y>=list.contentY-1&&list.itemAtIndex(list.currentIndex).y<list.contentY+list.height,"selected row scrolled into view")
   keys.keyClick(Qt.Key_Down);check(panel.activeRows[list.currentIndex].key==="fixture/t19","arrows navigate from current chat")
   check(b.selectedKey==="fixture/t18"&&b.draft.text==="Borrador con tildes: áéíóú","focus and arrows preserve selection/draft")
   b.selectedKey="fixture/t20";panel.workingExpanded=false;panel.composerView.inputField.forceActiveFocus();keys.keyClick(Qt.Key_B,Qt.MetaModifier);phase=3
  }else if(phase===3){
   var list=keys.findChild(panel,"workingThreadList")
   check(panel.workingExpanded&&list.activeFocus&&panel.visibleWorkingRows[list.currentIndex].key==="fixture/t20","Super+B opens selected Working section")
   b.selectedKey="fixture/t21";panel.settledExpanded=false;panel.composerView.inputField.forceActiveFocus();keys.keyClick(Qt.Key_B,Qt.ControlModifier);check(panel.sidebarCollapsed,"Ctrl+B hides from another focus")
   keys.keyClick(Qt.Key_B,Qt.MetaModifier);phase=4
  }else if(phase===4){
   var list=keys.findChild(panel,"settledThreadList")
   check(!panel.sidebarCollapsed&&panel.settledExpanded&&list.activeFocus&&panel.visibleSettledRows[list.currentIndex].key==="fixture/t21","Super+B reveals hidden sidebar and selected settled row")
   check(panel.shortcutHelp().indexOf("Super+B")>=0&&panel.shortcutHelp().indexOf("Ctrl+B")>=0,"help exposes both shortcuts")
   var rows=fixture.threads.concat([{key:"fixture/child",id:"child",environmentId:"fixture",title:"Child",relationshipToParent:"subagent",parentThreadId:"t0"}]);fixture=Object.assign({},fixture,{threads:rows});b.state=fixture;b.rememberView({subthreadsExpanded:{"fixture/t0":false}});b.selectedKey="fixture/child";panel.statusFilter="settled";b.query="no match";panel.composerView.inputField.forceActiveFocus();phase=-1;keys.keyClick(Qt.Key_B,Qt.MetaModifier);phase=5
  }else if(phase===5){
   var list=keys.findChild(panel,"threadList")
   check(b.query===""&&panel.statusFilter==="all"&&b.viewState.subthreadsExpanded["fixture/t0"]===true,"reveal clears excluding filters and opens selected child's parent")
   check(list.activeFocus&&panel.activeRows[list.currentIndex].key==="fixture/child","focus lands on exact child")
   panel.close();console.log("SIDEBAR_KEYBINDINGS_QML_PASS: Ctrl+B toggles, Super+B reveals exact selected row, arrows, closed sections, selection and draft retained");Qt.quit()
  }
 }}
}
