import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var fixture:null
 function check(ok,message){if(!ok){console.error("BOARD_CATEGORIES_QML_FAIL: "+message);Qt.exit(1)}}
 function ids(rows){return rows.map(function(t){return t.key}).sort().join("|")}
 function board(){return keys.findChild(panel,"t3FocusBoardLoader").item}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer {interval:230;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,v=board()
  if(phase===0){
   if(!b.viewStateLoaded)return
   var samples=[
    {id:"pin",pinned:true},{id:"parent",pending:true,reason:"review"},{id:"child",relationshipToParent:"subagent",parentThreadId:"parent",sidebarStatus:"working",working:true},
    {id:"pin-ready",pinned:true,pending:true,reason:"review"},{id:"idle"},{id:"failed",sidebarStatus:"failed"},{id:"work",sidebarStatus:"working",working:true},
    {id:"input",sidebarStatus:"input",pending:true,reason:"request"},{id:"plan",pending:true,reason:"plan"},{id:"sleep",snoozed:true},{id:"closed",settled:true},{id:"woke",woke:true},{id:"z-first-project",project:"Alpha",projectId:"q",projectKey:"fixture/q"}
   ]
   fixture={threads:samples.map(function(t,i){return Object.assign({key:"fixture/"+t.id,environmentId:"fixture",environmentName:"NODE",project:"O3",projectId:"p",projectKey:"fixture/p",title:t.id,sidebarStatus:"ready",updatedAt:Date.now()-i*1000},t)}),projects:[{id:"p",key:"fixture/p",environmentId:"fixture",title:"O3",workspaceRoot:"/var/tmp/o3"},{id:"q",key:"fixture/q",environmentId:"fixture",title:"Alpha",workspaceRoot:"/var/tmp/alpha"}],environments:[{id:"fixture",name:"NODE",state:"ready",olderSettled:{before:"2026-01-01T00:00:00Z",counts:[{projectId:"p",count:8}]}}]}
   b.catalogs={fixture:{canWrite:true,models:[],providers:[],capabilities:{}}};b.state=fixture
   b.rememberView({interfaceLayout:"focus",sidebarCompact:true,sidebarGrouped:true,settledExpanded:false,snoozedExpanded:false,pinnedExpanded:false,workingExpanded:false,inputExpanded:false,subthreadsExpanded:{"fixture/parent":true},boardLive:false,boardIdeas:[],boardColumns:{}})
   panel.open();panel.boardOpen=true;phase=1
  }else if(phase===1){
   if(!v)return
   var expected=panel.pinnedRows.concat(panel.activeRows,panel.workingRows,panel.inputRows,panel.snoozedRows,panel.settledRows)
   var actual=[];v.columnSpecs.forEach(function(s){actual=actual.concat(v.rows(s.key))})
   check(ids(actual)===ids(expected)&&actual.length===fixture.threads.length,"no sidebar thread or expanded child disappears")
   check(new Set(actual.map(function(t){return t.key})).size===actual.length,"each row belongs to exactly one column")
   check(ids(v.rows("pinned"))===ids(panel.pinnedRows)&&ids(v.rows("ready"))===ids(panel.activeRows)&&ids(v.rows("input"))===ids(panel.inputRows),"Pinned/Ready/Input match sidebar, including pinned ready")
   check(v.rows("snoozed")[0].id==="sleep"&&v.rows("settled")[0].id==="closed","folded sidebar shelves remain available in board")
   check(v.columnAt(v.columnIndex("settled")).view.footerItem.visible&&v.columnCount("settled")===1+panel.olderSettledMissing,"closed history count and load control remain available");
   v.configureColumn("ready",{order:"title",reverse:true})
   var ready=v.rows("ready"),parent=ready.findIndex(function(t){return t.id==="parent"});check(ready[parent+1].id==="child","sorting keeps child beneath its parent")
   b.rememberView({interfaceLayout:"projects"});phase=2
  }else if(phase===2){
   check(v.columnSpecs.map(function(s){return s.key}).join("|")==="waiting|working|failed|idle|done|snoozed|settled","project mode uses exact project tree state definitions")
   var actual=[];v.columnSpecs.forEach(function(s){actual=actual.concat(v.rows(s.key))})
   check(ids(actual)===ids(fixture.threads),"project states also keep all rows and child")
   check(v.columnCount("settled")===v.rows("settled").filter(function(t){return !t.depth}).length,"project counts match loaded project-tree rows");
   check(v.rows("idle")[0].id==="z-first-project","default order retains project ordering from sidebar");
   check(v.rows("failed")[0].id==="failed"&&v.rows("snoozed")[0].id==="sleep","failed and snoozed are not merged into Waiting/Pending")
   panel.hostFilter="another-host";phase=3
  }else if(phase===3){
   check(v.shownThreads.length===0,"host filter respected")
   panel.hostFilter="";b.query="closed";phase=4
  }else if(phase===4){
   check(v.rows("settled").length===1&&v.rows("settled")[0].id==="closed","search finds closed threads despite folded sidebar shelf")
   b.query="";b.rememberView({interfaceLayout:"focus",sidebarGrouped:false});phase=5
  }else if(phase===5){
   check(v.columnSpecs.map(function(s){return s.key}).join("|")==="ready|snoozed|settled","flat sidebar categories preserved")
   var actual=v.rows("ready").concat(v.rows("snoozed"),v.rows("settled"));check(ids(actual)===ids(fixture.threads),"ungrouped mode does not lose rows")
   v.focusCell(v.columnIndex("ready"),0);keys.parent=v;phase=6
  }else if(phase===6){keys.keyClick(Qt.Key_F1);phase=7
  }else if(phase===7){check(keys.findChild(v,"t3BoardKeyboardHelp").visible,"F1 opens board help instead of global chat help");keys.keyClick(Qt.Key_Escape);phase=8
  }else if(phase===8){keys.keyClick(Qt.Key_L,Qt.ControlModifier);check(v.liveMode,"Ctrl+L stays inside board instead of focusing chat");keys.keyClick(Qt.Key_Return);phase=9
  }else if(phase===9){check(!panel.boardOpen&&!!b.selectedKey&&!panel.sidebarUpdatesPaused,"Enter opens thread and releases sidebar interaction holds");
   console.log("BOARD_CATEGORIES_QML_PASS: exact grouped/flat/project categories, pinned ready, snoozed, closed, child, sort and filters");panel.close();Qt.quit()
  }
 }}
}
