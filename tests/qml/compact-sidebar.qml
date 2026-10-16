import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property bool done:false
 function check(ok,msg){if(!ok){console.error("T3_COMPACT_SIDEBAR_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 Timer{interval:200;running:!root.done;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.rememberView({language:"es"});b.state={threads:[{id:"busy",key:"fixture/busy",environmentId:"fixture",project:"P",title:"Busy",working:true,status:"running",lastSentAt:20},{id:"busy2",key:"fixture/busy2",environmentId:"fixture",project:"P",title:"Busy older",working:true,status:"running",lastSentAt:10},{id:"ask",key:"fixture/ask",environmentId:"fixture",project:"P",title:"Question",working:true,status:"running",pending:true,reason:"request"},{id:"idle",key:"fixture/idle",environmentId:"fixture",project:"P",title:"Inbox",status:"completed"},{id:"nap",key:"fixture/nap",environmentId:"fixture",project:"P",title:"Nap",snoozed:true},{id:"done",key:"fixture/done",environmentId:"fixture",project:"P",title:"Closed",settled:true},{id:"child",key:"fixture/child",environmentId:"fixture",title:"Child",working:true,relationshipToParent:"subagent"}],projects:[],environments:[]}
   b.selectedEnvironment="fixture";b.selectedThread="busy";b.selectedKey="fixture/busy";b.patchDraft(b.selectedKey,{loading:false,text:"Safe draft"});panel.open();phase=1
  }else if(phase===1){
   root.check(panel.activeRows.length===4&&!panel.workingRows.length,"original list before toggle")
   var footer=keys.findChild(panel,"t3SidebarFooter"),language=keys.findChild(panel,"t3LanguagePicker"),group=keys.findChild(panel,"sidebarCompactToggle");root.check(Math.abs(group.y-language.y)<4,"footer controls stay on one line");root.check(language.width<135&&language.width>40,"language selector fits Spanish label")
   var button=keys.findChild(panel,"sidebarCompactToggle");keys.mouseClick(button,button.width/2,button.height/2);phase=2
  }else if(phase===2){
   root.check(panel.sidebarCompact&&b.viewState.sidebarCompact,"button persists compact mode")
   root.check(panel.activeRows.length===2&&panel.activeRows.some(function(t){return t.id==="ask"}),"attention request stays in inbox")
   root.check(panel.workingRows.length===2&&panel.workingRows[0].id==="busy","working section sorted by send time")
   root.check(panel.visibleWorkingRows.length===1&&panel.visibleWorkingRows[0].id==="busy","collapsed section retains open thread")
   root.check(panel.snoozedRows.length===1&&panel.settledRows.length===1,"separate snoozed and settled groups")
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){r.saveToFile("/tmp/t3-compact-sidebar.png")})
   var header=keys.findChild(panel,"workingShelfToggle");header.forceActiveFocus();keys.keyClick(Qt.Key_Return);phase=3
  }else if(phase===3){
   root.check(panel.workingExpanded&&panel.visibleWorkingRows.length===2&&b.viewState.workingExpanded,"group expands with keyboard")
   panel.workingExpanded=false;b.query="Busy";root.check(panel.visibleWorkingRows.length===2,"search reveals collapsed matches");b.query=""
   b.selectedKey="fixture/nap";root.check(panel.visibleSnoozedRows.length===1,"selected snoozed thread visible")
   b.selectedKey="fixture/done";root.check(panel.visibleSettledRows.length===1,"selected settled thread visible")
   b.selectedKey="fixture/idle";root.check(!panel.visibleWorkingRows.length&&!panel.visibleSettledRows.length,"unselected collapsed rows hidden")
   var original=b.state;b.state={threads:[original.threads[3]],projects:[],environments:[]};
   root.check(panel.sidebarCompact&&!panel.workingRows.length&&!panel.snoozedRows.length&&!panel.settledRows.length,"idle data has no work to move")
   root.check(keys.findChild(panel,"workingShelfToggle").visible&&keys.findChild(panel,"snoozedShelfToggle").visible&&keys.findChild(panel,"settledShelfToggle").visible,"all three group headers remain visible with zero counts")
   var button=keys.findChild(panel,"sidebarCompactToggle");root.check(button.name==="check","grouping is an icon-only active button");b.state=original;keys.mouseClick(button,button.width/2,button.height/2);phase=4
  }else if(phase===4){
   root.check(!panel.sidebarCompact&&panel.activeRows.length===4&&!panel.workingRows.length,"turning off restores inbox")
   root.check(b.drafts["fixture/busy"].text==="Safe draft","grouping preserves prompt")
   panel.close();panel.open();root.check(!panel.sidebarCompact,"toggle choice survives reopen")
   panel.close();root.done=true;console.log("T3_COMPACT_SIDEBAR_QML_PASS: footer toggle, working/snoozed/settled shelves, attention, selection, search, keyboard and drafts")
  }
 }}
}
