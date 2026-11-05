import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var fixture:null
 property string originalDraft:""
 property real boardRotation:0
 function check(ok,msg){if(!ok){console.error("FOCUS_LAYOUT_QML_FAIL: "+msg);Qt.exit(1)}}
 function row(id){var list=panel.sidebarView;for(var i=0;i<list.count;i++){var r=list.itemAtIndex(i);if(r&&r.thread.id===id)return r}return null}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   Test.T3Palette.values={blue:"#5aa2e6",green:"#8fc46a",yellow:"#d8c35a",orange:"#e5a050",magenta:"#d877b8"}
   b.catalogs={fixture:{canWrite:true,capabilities:{},models:[],providers:[]},local:{canWrite:true,capabilities:{},models:[],providers:[]}}
   fixture={threads:[
    {id:"work",key:"fixture/work",environmentId:"fixture",environmentName:"NODE",project:"O3",projectId:"p",projectKey:"fixture/p",title:"Working task",working:true,status:"running",repliedAt:100},
    {id:"wait",key:"local/wait",environmentId:"local",environmentName:"M2",project:"O3",projectId:"p",projectKey:"local/p",title:"Approval task",working:true,pending:true,reason:"request",status:"running",repliedAt:90},
    {id:"done",key:"fixture/done",environmentId:"fixture",environmentName:"NODE",project:"O3",projectId:"p",projectKey:"fixture/p",title:"Finished task",status:"completed",repliedAt:80},
    {id:"idle",key:"local/idle",environmentId:"local",environmentName:"M2",project:"O3",projectId:"p",projectKey:"local/p",title:"Pending task",status:"idle",repliedAt:70}],
    projects:[{id:"p",key:"fixture/p",environmentId:"fixture",title:"O3",workspaceRoot:"/tmp/o3-fixture"},{id:"p",key:"local/p",environmentId:"local",title:"O3",workspaceRoot:"/tmp/o3-fixture"}],environments:[{id:"local",name:"M2",state:"ready"},{id:"fixture",name:"NODE",state:"ready"}]}
   b.state=fixture;b.rememberView({sidebarStyle:"vivid",threadRowSize:"large",projectColors:false});panel.open();b.selectThread("fixture","work");b.receive(JSON.stringify({type:"draft-loaded",data:{key:"fixture/work",draft:{},error:null}}));b.edit("Keep this unsent draft");originalDraft=b.draft.text;phase=1
  }else if(phase===1){
   check(!panel.focusLayout,"existing installs default to classic")
   var r=row("work");if(!r)return
   check(!r.slim&&!r.focusLayout,"classic large rows preserved")
   check(keys.findChild(panel,"t3OpenFocusBoard").visible,"board available in classic layout")
   panel.preferencesOpen=true;phase=2
  }else if(phase===2){
   var loader=keys.findChild(panel,"t3PreferencesView");if(!loader.item)return
   loader.item.page="appearance";loader.item.query="Interface layout";phase=3
  }else if(phase===3){
   var picker=keys.findChild(panel,"preferenceChoice_interfaceLayout");check(picker&&picker.visible,"layout option searchable in Appearance")
   picker.activated(1);panel.preferencesOpen=false;phase=4
  }else if(phase===4){
   check(panel.focusLayout,"preference activates Focus")
   var r=row("work");if(!r)return
   check(r.slim&&r.subtitled&&r.height===Style.space(58),"Focus uses two-line rows")
   check(keys.findChild(r,"threadFocusHost").visible,"host always shown without host customization")
   check(keys.findChild(panel,"t3FocusComposerContext").visible,"composer shows context")
   check(keys.findChild(panel,"t3FocusComposerHost").text==="NODE","composer host matches selection")
   b.rememberView({hostStyles:{fixture:{icon:"cloud",color:"#5aa2e6",showName:true}}})
   var badge=keys.findChild(panel,"t3HeaderHost")
   check(badge&&badge.visible&&badge.environmentId==="fixture"&&badge.look.icon==="cloud"&&badge.look.color==="#5aa2e6","title host follows saved icon and color")
   check(keys.findChild(r,"threadFocusHost").look.icon==="cloud","sidebar follows same host icon")
   check(b.draft.text===originalDraft,"switch leaves unsent draft intact")
   check(!keys.findChild(r,"threadAura").visible,"Focus suppresses decorative row effects without rewriting preferences")
   phase=40;keys.findChild(panel,"t3PanelLayout").grabToImage(function(g){g.saveToFile("/var/tmp/o3-focus-layout.png");root.phase=41})
  }else if(phase===41){
   keys.findChild(panel,"t3OpenFocusBoard").clicked();phase=5
  }else if(phase===5){
   var board=keys.findChild(panel,"t3FocusBoardLoader").item;if(!board)return
   board.liveKeys=["fixture/work"];check(board.liveThreads.length===1,"live subscriptions target chosen cards");board.liveKeys=[];
   var idea=board.addIdea("- A durable idea");board.updateIdea(idea,"- A refined idea");check(b.viewState.boardIdeas[0].text==="- A refined idea","independent idea saves edits");
   var shared=keys.findChild(board,"t3BoardRow_work");var sidebar=row("work");
   check(shared&&shared.stateColor===sidebar.stateColor&&shared.timeLabel===sidebar.timeLabel&&shared.hostLook.color===sidebar.hostLook.color,"board uses sidebar colors, clock and host style");
   var glyph=keys.findChild(shared,"threadStateIcon");check(glyph.visible&&glyph.spinning,"board working icon spins like sidebar");boardRotation=glyph.rotation;
   var boardAura=keys.findChild(shared,"threadAura"),sideAura=keys.findChild(sidebar,"threadAura");
   check(boardAura.visible===sideAura.visible&&Qt.colorEqual(boardAura.tone,sideAura.tone)&&Qt.colorEqual(boardAura.alert,sideAura.alert),"same aura visibility and working/input palette");
   check(shared.sky===sidebar.sky&&shared.effects===sidebar.effects&&shared.compactRows===sidebar.compactRows,"board shares sidebar sky and effect preferences");
   keys.mouseClick(shared,10,shared.height/2,Qt.RightButton);check(shared.rowMenu&&shared.rowMenu.visible,"board opens real sidebar context menu");shared.rowMenu.close();
   check(board.rows("input").length===1&&board.rows("input")[0].id==="wait","approval has priority over working")
   check(board.rows("ready").length===3,"board uses exact Ready sidebar section")
   phase=50;board.grabToImage(function(g){g.saveToFile("/var/tmp/o3-focus-board.png");root.phase=51})
  }else if(phase===51){
   check(keys.findChild(keys.findChild(keys.findChild(panel,"t3FocusBoardLoader").item,"t3BoardRow_work"),"threadStateIcon").rotation!==boardRotation,"board working icon actually rotates");
   keys.mouseClick(keys.findChild(keys.findChild(panel,"t3FocusBoardLoader").item,"t3BoardRow_work"),10,20,Qt.LeftButton);phase=52
  }else if(phase===52){
   check(!panel.boardOpen&&b.selectedThread==="work","clicking shared board row opens thread and closes board");panel.boardOpen=true;phase=53
  }else if(phase===53){
   row("work").threadActivated(fixture.threads[0]);check(!panel.boardOpen,"sidebar activation closes board even for selected thread");panel.boardOpen=true;panel.hostFilter="fixture";phase=6
  }else if(phase===6){
   var board=keys.findChild(panel,"t3FocusBoardLoader").item
   check(board.threads.length===2&&board.rows("input").length===0,"board honors host filter")
   var card=keys.findChild(board,"t3BoardThread_done");check(!!card,"task card rendered");card.clicked();phase=7
  }else if(phase===7){
   check(!panel.boardOpen&&b.selectedThread==="done","card opens actual thread and returns to conversation")
   b.selectThread("fixture","work");b.rememberView({interfaceLayout:"classic"});phase=8
  }else if(phase===8){
   var r=row("work");if(!r)return
   check(!panel.focusLayout&&!r.slim&&!r.sober&&!r.projectWash,"classic restores prior appearance settings")
   check(!keys.findChild(panel,"t3FocusComposerContext").visible,"classic composer restored")
   check(b.draft.text===originalDraft,"draft survives full round trip")
   keys.findChild(panel,"sidebarLayoutToggle").clicked();phase=9
  }else if(phase===9){
   check(keys.findChild(panel,"sidebarLayoutMenu").opened,"quick layout menu opens from sidebar footer")
   keys.findChild(panel,"sidebarLayoutFocus").triggered();keys.findChild(panel,"sidebarLayoutMenu").close();phase=10
  }else if(phase===10){
   check(panel.focusLayout&&b.viewState.interfaceLayout==="focus","quick menu shares the persisted preference")
   keys.findChild(panel,"sidebarLayoutClassic").triggered();phase=11
  }else if(phase===11){
   check(!panel.focusLayout&&!panel.sidebarCompact&&b.draft.text===originalDraft,"quick switch keeps grouping and draft unchanged")
   check(!panel.projectThreads,"other modes do not keep a hidden project tree alive")
   console.log("FOCUS_LAYOUT_QML_PASS: default classic, searchable preference, reversible style, composer host, board status/filter/navigation and draft preservation");Qt.quit()
  }
 }}
}
