import QtQuick
import QtTest
import qs.Commons
import Quickshell
import "UnderTest/ui/qml" as Test
// The footer's group/ungroup button changes the sidebar in every interface layout:
// grouped = sections by state, ungrouped = one flat list.
ShellRoot{
 id:root;property int phase:0;property bool done:false;property var fixture:null
 function check(ok,msg){if(!ok){console.error("LAYOUT_GROUPING_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 readonly property var layouts:["classic","focus","projects"]
 property int layoutIndex:0
 function press(){var button=keys.findChild(panel,"sidebarCompactToggle");keys.mouseClick(button,button.width/2,button.height/2)}
 function ids(rows){return rows.map(function(t){return t.id}).sort().join()}
 Timer{interval:250;running:!root.done;repeat:true;onTriggered:{
  var b=Test.T3Backend,layout=layouts[layoutIndex]
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.rememberView({sidebarCompact:true,workingExpanded:true,sidebarGrouped:true,interfaceLayout:"classic"})
   b.state={threads:[
    {id:"pin",key:"fixture/pin",environmentId:"fixture",projectKey:"fixture/p",project:"P",title:"Pinned",status:"completed",pinned:true,repliedAt:1},
    {id:"busy",key:"fixture/busy",environmentId:"fixture",projectKey:"fixture/p",project:"P",title:"Busy",working:true,status:"running",lastSentAt:20,repliedAt:Date.now()-3600000,updatedAt:Date.now()-1000,startedAt:Date.now()-3600000},
    {id:"ask",key:"fixture/ask",environmentId:"fixture",projectKey:"fixture/q",project:"Q",title:"Question",working:true,status:"running",pending:true,reason:"request"},
    {id:"idle",key:"fixture/idle",environmentId:"fixture",projectKey:"fixture/q",project:"Q",title:"Inbox",status:"completed",repliedAt:9},
    {id:"nap",key:"fixture/nap",environmentId:"fixture",projectKey:"fixture/q",project:"Q",title:"Nap",snoozed:true},
    {id:"done",key:"fixture/done",environmentId:"fixture",projectKey:"fixture/q",project:"Q",title:"Closed",settled:true}],
    projects:[{key:"fixture/p",environmentId:"fixture",title:"P",workspaceRoot:"/w/p"},{key:"fixture/q",environmentId:"fixture",title:"Q",workspaceRoot:"/w/q"}],environments:[]}
   fixture=b.state;panel.open();phase=1
  }else if(phase===1){
   b.rememberView({interfaceLayout:layout,sidebarGrouped:true});phase=2
  }else if(phase===2){
   // Grouped: sections by state.
   root.check(panel.sidebarGrouped,layout+": starts grouped")
   if(layout==="projects"){
    var kinds=panel.projectThreads.entries.map(function(e){return e.kind==="state"?"s:"+e.state:e.kind==="project"?"p":"t"}).join()
    root.check(kinds.indexOf("s:working")>=0&&kinds.indexOf("s:waiting")>=0,layout+": grouped shows a header per state: "+kinds)
   }else{
    root.check(ids(panel.pinnedRows)==="pin"&&ids(panel.inputRows)==="ask"&&ids(panel.workingRows)==="busy"&&ids(panel.activeRows)==="idle",layout+": grouped sections: "+ids(panel.pinnedRows)+"|"+ids(panel.activeRows)+"|"+ids(panel.workingRows)+"|"+ids(panel.inputRows))
   }
   root.press();phase=3
  }else if(phase===3){
   // Ungrouped: one flat list.
   root.check(!panel.sidebarGrouped&&b.viewState.sidebarGrouped===false,layout+": the button ungroups and remembers it")
   if(layout==="projects"){
    var flat=panel.projectThreads.entries.map(function(e){return e.kind==="state"?"s":e.kind==="project"?"p":"t"}).join("")
    root.check(flat.indexOf("s")<0&&panel.projectThreads.visibleThreads.length===6,layout+": ungrouped has no state headers: "+flat)
   }else{
    root.check(!panel.pinnedRows.length&&!panel.inputRows.length&&!panel.workingRows.length,layout+": no sections when ungrouped")
    root.check(ids(panel.activeRows)==="ask,busy,idle,pin","a flat list holds pinned, waiting, working and ready threads: "+ids(panel.activeRows))
    root.check(panel.activeRows[0].id==="busy","recent running activity leads the flat list, including pins")
    var busy=panel.sidebarView.itemAtIndex(0)
    root.check(busy&&busy.timeLabel===panel.age(busy.thread.updatedAt),"visible age matches the sorting clock, not run duration")
    root.check(panel.snoozedRows.length===1&&panel.settledRows.length===1,layout+": snoozed and settled keep their shelves")
   }
   root.press();phase=4
  }else if(phase===4){
   root.check(panel.sidebarGrouped&&b.viewState.sidebarGrouped!==false,layout+": pressing again groups again")
   if(layoutIndex<layouts.length-1){layoutIndex++;phase=1}else{console.log("LAYOUT_GROUPING_QML_PASS");root.done=true}
  }
 }}
}
