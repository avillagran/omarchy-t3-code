import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var fixture:null
 property string originalKeys:""
 function check(ok,message){if(!ok){console.error("BOARD_GLOBAL_QML_FAIL: "+message);Qt.exit(1)}}
 function board(){return keys.findChild(panel,"t3FocusBoardLoader").item}
 function allKeys(v){var rows=[];v.columnSpecs.forEach(function(c){rows=rows.concat(v.rows(c.key))});return rows.map(function(t){return t.key}).sort().join("|")}
 function control(v,name){return keys.findChild(v,"t3BoardGlobal"+name)}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer {interval:230;repeat:true;running:true;onTriggered:{
  var b=Test.T3Backend,v=board()
  if(phase===0){
   if(!b.viewStateLoaded)return
   var samples=[{id:"pin",pinned:true},{id:"pin-work",pinned:true,working:true,sidebarStatus:"working"},{id:"work",working:true,sidebarStatus:"working"},{id:"ready"},{id:"input",pending:true,reason:"request",sidebarStatus:"input"},{id:"closed",settled:true},{id:"child",parentThreadId:"pin",relationshipToParent:"subagent",working:true,sidebarStatus:"working"},{id:"other",environmentId:"other",projectKey:"other/q",projectId:"q"}]
   fixture={threads:samples.map(function(t,i){return Object.assign({key:(t.environmentId || "fixture")+"/"+t.id,environmentId:"fixture",environmentName:"NODE",project:"O3",projectId:"p",projectKey:"fixture/p",title:t.id,sidebarStatus:"ready",updatedAt:Date.now()-i*1000},t)}),projects:[{id:"p",key:"fixture/p",environmentId:"fixture",title:"O3",workspaceRoot:"/var/tmp/o3"},{id:"q",key:"other/q",environmentId:"other",title:"Other",workspaceRoot:"/var/tmp/other"}],environments:[{id:"fixture",name:"NODE",state:"ready"},{id:"other",name:"M2",state:"ready"}]}
   b.state=fixture;b.catalogs={fixture:{canWrite:true,models:[],providers:[],capabilities:{}},other:{canWrite:true,models:[],providers:[],capabilities:{}}};b.rememberView({interfaceLayout:"focus",sidebarCompact:true,sidebarGrouped:true,boardShowPinned:true,boardColumns:{},boardIdeas:[],boardLive:false,subthreadsExpanded:{"fixture/pin":true}})
   panel.open();panel.boardOpen=true;phase=1
  }else if(phase===1){
   originalKeys=allKeys(v);check(v.rows("pinned").filter(function(t){return !t.depth}).length===2,"two pinned parents initially have their own column")
   keys.parent=v;keys.mouseClick(control(v,"Options"),12,12);phase=2
  }else if(phase===2){
   check(keys.findChild(v,"t3BoardGlobalPopup").visible,"top toolbar opens global filters and options")
   keys.findChild(v,"t3BoardGlobalBody").grabToImage(function(g){g.saveToFile("/var/tmp/o3-board-global-options.png")});
   var sort=control(v,"Sort");sort.activated(sort.model.findIndex(function(o){return o.key==="title"}))
   check(v.allColumnKeys.every(function(k){return b.viewState.boardColumns[k].order==="title"}),"global sort updates all layouts, including columns not currently shown")
   keys.mouseClick(control(v,"Reverse"),12,12)
   check(v.allColumnKeys.every(function(k){return b.viewState.boardColumns[k].reverse===true}),"global reversal is applied once to every column")
   keys.mouseClick(control(v,"Pinned"),12,12);phase=3
  }else if(phase===3){
   check(b.viewState.boardShowPinned===false&&v.columnSpecs.every(function(c){return c.key!=="pinned"}),"pinned column can be disabled")
   check(allKeys(v)===originalKeys,"hiding pinned column loses no thread or child")
   check(v.rows("working").some(function(t){return t.id==="pin-work"})&&v.rows("ready").some(function(t){return t.id==="pin"}),"pinned threads join their ordinary state")
   check(b.state.threads.find(function(t){return t.id==="pin"}).pinned===true,"presentation option does not unpin server thread")
   control(v,"Collapse").clicked();phase=4
  }else if(phase===4){
   v.columnSpecs.forEach(function(c){var item=v.columnAt(v.columnIndex(c.key));check(item.width<60,"folded column is a narrow rail");check(keys.findChild(v,"t3BoardRailTitle_"+c.key).rotation===90,"folded title rotates 90 degrees")})
   check(v.columnAt(0).width<60&&keys.findChild(v,"t3BoardRailTitle_ideas").rotation===90,"ideas can also fold into a rail")
   v.grabToImage(function(g){g.saveToFile("/var/tmp/o3-board-rails.png")});phase=44
  }else if(phase===44){control(v,"Expand").clicked();phase=5
  }else if(phase===5){
   check(v.columnAt(0).width>200&&v.columnSpecs.every(function(c){return !v.columnSetting(c.key).collapsed}),"expand-all restores normal columns")
   var live=control(v,"Live");live.activated(1)
   check(v.columnSpecs.every(function(c){return v.columnLive(c.key)}),"global live choice reaches every thread column")
   var host=control(v,"Host");host.activated(host.model.findIndex(function(h){return h.id==="fixture"}));phase=6
  }else if(phase===6){
   check(v.shownThreads.every(function(t){return t.environmentId==="fixture"}),"global host filter applies across all columns")
   var status=control(v,"Status");status.activated(status.model.findIndex(function(s){return s.key==="working"}));phase=7
  }else if(phase===7){
   check(v.shownThreads.some(function(t){return t.id==="pin-work"})&&v.shownThreads.every(function(t){return t.working}),"status filter includes ordinary pinned working thread")
   control(v,"Clear").clicked();phase=8
  }else if(phase===8){
   check(allKeys(v)===originalKeys&&panel.hostFilter===""&&panel.statusFilter==="all","clear restores all matching columns and rows")
   var search=control(v,"Search");search.forceActiveFocus();keys.keyClick(Qt.Key_W);keys.keyClick(Qt.Key_O);keys.keyClick(Qt.Key_R);keys.keyClick(Qt.Key_K);phase=9
  }else if(phase===9){
   check(b.query==="work"&&v.shownThreads.filter(function(t){return !t.depth}).every(function(t){return t.id.indexOf("work")>=0}),"search typing filters every column without triggering board shortcuts")
   keys.keyClick(Qt.Key_Escape);phase=10
  }else if(phase===10){
   check(panel.boardOpen&&!keys.findChild(v,"t3BoardGlobalPopup").visible,"Escape closes global options without closing board")
   v.clearFilters();v.configureColumn("ready",{collapsed:true});v.focusCell(v.columnIndex("ready"),0);phase=11
  }else if(phase===11){
   check(v.columnAt(v.columnIndex("ready")).header.activeFocus,"keyboard focus lands on folded rail")
   keys.keyClick(Qt.Key_Return);phase=12
  }else if(phase===12){
   check(!!v,"folded rail activation keeps board open; selected="+b.selectedKey+" open="+panel.opened);
   check(!v.columnSetting("ready").collapsed,"Enter unfolds a focused vertical rail")
   v.configureColumn("ideas",{collapsed:true});v.focusCell(0,0);phase=13
  }else if(phase===13){
   keys.keyClick(Qt.Key_N);phase=14
  }else if(phase===14){
   check(!v.columnSetting("ideas").collapsed&&!!v.editingIdea,"new idea shortcut unfolds Ideas before editing")
   b.rememberView({boardShowPinned:true});phase=15
  }else if(phase===15){
   check(v.columnSpecs.some(function(c){return c.key==="pinned"})&&allKeys(v)===originalKeys,"restoring pinned column keeps every thread exactly once");
   console.log("BOARD_GLOBAL_QML_PASS: global filters/order/live/folding, pinned redistribution without loss, narrow rotated rails, keyboard and search");panel.close();Qt.quit()
  }
 }}
}
