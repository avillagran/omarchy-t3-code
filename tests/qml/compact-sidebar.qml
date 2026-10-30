import QtQuick
import QtTest
import qs.Commons
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property bool done:false;property var fixture:null
 function check(ok,msg){if(!ok){console.error("T3_COMPACT_SIDEBAR_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 Timer{interval:200;running:!root.done;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.rememberView({language:"es"});b.state={threads:[{id:"busy",key:"fixture/busy",environmentId:"fixture",project:"P",title:"Busy",working:true,status:"running",lastSentAt:20},{id:"busy2",key:"fixture/busy2",environmentId:"fixture",project:"P",title:"Busy older",working:true,status:"running",lastSentAt:10},{id:"ask",key:"fixture/ask",environmentId:"fixture",project:"P",title:"Question",working:true,status:"running",pending:true,reason:"request"},{id:"idle",key:"fixture/idle",environmentId:"fixture",project:"P",title:"Inbox",status:"completed"},{id:"nap",key:"fixture/nap",environmentId:"fixture",project:"P",title:"Nap",snoozed:true},{id:"done",key:"fixture/done",environmentId:"fixture",project:"P",title:"Closed",settled:true},{id:"child",key:"fixture/child",environmentId:"fixture",title:"Child",working:true,relationshipToParent:"subagent"}],projects:[],environments:[]}
   fixture=b.state;b.selectedEnvironment="fixture";b.selectedThread="busy";b.selectedKey="fixture/busy";b.patchDraft(b.selectedKey,{loading:false,text:"Safe draft"});panel.open();phase=1
  }else if(phase===1){
   root.check(panel.activeRows.length===3&&panel.inputRows.length===1&&!panel.workingRows.length,"original list before toggle")
   var footer=keys.findChild(panel,"t3SidebarFooter"),group=keys.findChild(panel,"sidebarCompactToggle");root.check(!keys.findChild(panel,"t3LanguagePicker"),"language lives in Preferences, not the sidebar footer");root.check(!!group&&group.visible,"footer keeps its controls")
   var button=keys.findChild(panel,"sidebarCompactToggle");root.check(button.name==="layers"&&panel.sidebarGrouped,"the footer button starts grouped");panel.setSidebarGrouped(true);phase=2
  }else if(phase===2){
   root.check(panel.sidebarCompact&&b.viewState.sidebarCompact,"the Working shelf preference persists")
   root.check(panel.activeRows.length===1&&panel.inputRows.length===1&&panel.inputRows[0].id==="ask","attention request waits in its own group, outside Working")
   root.check(panel.workingRows.length===2&&panel.workingRows[0].id==="busy","working section sorted by send time")
   root.check(panel.visibleWorkingRows.length===0,"collapsed section hides even the open thread")
   root.check(panel.snoozedRows.length===1&&panel.settledRows.length===1,"separate snoozed and settled groups")
   for(var name of ["working","snoozed","settled"]){var line=keys.findChild(panel,name+"ShelfDivider");root.check(line.visible&&line.width>0&&line.height===1&&line.x>Style.space(30)&&line.y>0,"visible separator for "+name)}
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/t3-compact-sidebar.png")})
   var header=keys.findChild(panel,"workingShelfToggle");header.forceActiveFocus();keys.keyClick(Qt.Key_Return);phase=3
  }else if(phase===3){
   root.check(panel.workingExpanded&&panel.visibleWorkingRows.length===2&&b.viewState.workingExpanded,"group expands with keyboard")
   panel.workingExpanded=false;b.query="Busy";root.check(panel.visibleWorkingRows.length===0,"search preserves explicit collapse");b.query=""
   b.selectedKey="fixture/nap";root.check(panel.visibleSnoozedRows.length===0,"selected snoozed thread does not expand group")
   b.selectedKey="fixture/done";root.check(panel.visibleSettledRows.length===0,"selected settled thread does not expand group")
   b.selectedKey="fixture/idle";root.check(!panel.visibleWorkingRows.length&&!panel.visibleSettledRows.length,"unselected collapsed rows hidden")
   var original=b.state;b.state={threads:[original.threads[3]],projects:[],environments:[]};
   root.check(panel.sidebarCompact&&!panel.workingRows.length&&!panel.snoozedRows.length&&!panel.settledRows.length,"idle data has no work to move")
   root.check(!keys.findChild(panel,"workingShelfHeader").shown&&!keys.findChild(panel,"snoozedShelfHeader").shown&&!keys.findChild(panel,"settledShelfHeader").shown,"empty group headers hide")
   var button=keys.findChild(panel,"sidebarCompactToggle");root.check(button.name==="layers","grouping is an icon-only active button");b.state=original;panel.setSidebarGrouped(false);phase=4
  }else if(phase===4){
   root.check(!panel.sidebarCompact&&panel.activeRows.length===3&&!panel.workingRows.length,"turning off restores inbox")
   root.check(b.drafts["fixture/busy"].text==="Safe draft","grouping preserves prompt")
   panel.workingExpanded=true;panel.snoozedExpanded=true;panel.settledExpanded=true;panel.setSidebarGrouped(true);root.check(panel.workingExpanded&&panel.snoozedExpanded&&panel.settledExpanded,"enabling grouping preserves expansion preferences");panel.setSidebarGrouped(false);panel.close();panel.open();root.check(!panel.sidebarCompact,"toggle choice survives reopen")
   panel.close();root.done=true;console.log("T3_COMPACT_SIDEBAR_QML_PASS: footer toggle, working/snoozed/settled shelves, attention, selection, search, keyboard and drafts")
  }
 }}
}
