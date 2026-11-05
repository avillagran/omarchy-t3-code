import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int closed:0
 property var opened:null
 property var fixture:null
 property string ideaKey:""
 function check(ok,message){if(!ok){console.error("BOARD_KEYBOARD_QML_FAIL: "+message);Qt.exit(1)}}
 function chooseByKeys(menu,text){for(var i=0;i<menu.count+1;i++){var item=menu.itemAt(menu.currentIndex);if(item&&item.text===text){keys.keyClick(Qt.Key_Return);return}keys.keyClick(Qt.Key_Down)}check(false,"menu option reachable: "+text)}
 function card(key){return keys.findChild(loader.item,"t3BoardThread_"+key)}
 TestCase{id:keys;when:false}
 FloatingWindow {
  implicitWidth:1100;implicitHeight:820;visible:true
  Loader{id:loader;anchors.fill:parent;active:false;sourceComponent:Test.T3ThreadBoard{bridge:Test.T3Backend;threads:root.fixture?root.fixture.threads:[];onClosed:root.closed++;onThreadRequested:function(t){root.opened=t}}}
 }
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer {interval:220;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,v=loader.item
  if(phase===0){
   if(!b.viewStateLoaded)return
   fixture={threads:[{id:"z",title:"Zebra",sidebarStatus:"ready"},{id:"a",title:"Alpha",sidebarStatus:"ready"},{id:"done",title:"Done",sidebarStatus:"ready",pending:true,reason:"review"}].map(function(t){return Object.assign({key:"fixture/"+t.id,environmentId:"fixture",environmentName:"NODE",project:"O3",projectKey:"fixture/p",projectId:"p"},t)}),projects:[],environments:[]}
   b.state=fixture;b.catalogs={fixture:{canWrite:true,models:[],providers:[],capabilities:{threadPinning:true}}};b.rememberView({interfaceLayout:"focus",boardIdeas:[],boardColumns:{},boardLive:false});loader.active=true;phase=1
  }else if(phase===1){keys.parent=v;v.focusCell(v.columnIndex("idle"),0);phase=2
  }else if(phase===2){
   check(card("z").activeFocus,"keyboard focus lands on card, not chat editor")
   keys.keyClick(Qt.Key_Down);check(v.navKey==="fixture/a"&&!b.selectedKey,"arrows move without selecting or sending")
   keys.keyClick(Qt.Key_Home);check(v.navKey==="fixture/z","Home reaches first card");keys.keyClick(Qt.Key_PageDown);check(v.navKey==="fixture/a","PageDown stays within column bounds");
   keys.keyClick(Qt.Key_O);phase=3
  }else if(phase===3){
   check(v.columnAt(v.columnIndex("idle")).menu.visible,"O opens column menu")
   check(v.columnAt(v.columnIndex("idle")).menu.width>180&&v.columnAt(v.columnIndex("idle")).menu.contentItem.width>150,"column menu has readable geometry");
   v.columnAt(v.columnIndex("idle")).menu.contentItem.grabToImage(function(g){g.saveToFile("/var/tmp/o3-board-column-menu.png")});phase=33
  }else if(phase===33){
   // Starting at the menu's first enabled item, choose Title with arrow keys.
   keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Return);phase=4
  }else if(phase===4){
   check((b.viewState.boardColumns.idle || {}).order==="title"&&v.rows("idle")[0].id==="a","column menu works entirely by keyboard")
   check(v.navKey==="fixture/a"&&card("a").activeFocus,"sorting retains focus on same thread")
   keys.keyClick(Qt.Key_O);phase=44
  }else if(phase===44){chooseByKeys(v.columnAt(v.columnIndex("idle")).menu,Test.T3I18n.tr("Collapse column"));phase=45
  }else if(phase===45){check(v.columnSetting("idle").collapsed&&v.columnAt(v.columnIndex("idle")).header.activeFocus,"keyboard collapses column without losing header focus: "+JSON.stringify(v.columnSetting("idle"))+" focus="+v.columnAt(v.columnIndex("idle")).header.activeFocus);keys.keyClick(Qt.Key_O);phase=46
  }else if(phase===46){chooseByKeys(v.columnAt(v.columnIndex("idle")).menu,Test.T3I18n.tr("Expand column"));phase=47
  }else if(phase===47){check(!v.columnSetting("idle").collapsed,"keyboard expands column again");keys.keyClick(Qt.Key_Right);phase=5
  }else if(phase===5){
   check(v.navColumn==="done"&&card("done").activeFocus,"right arrow enters next column and scrolls horizontally")
   keys.keyClick(Qt.Key_F10,Qt.ShiftModifier);phase=6
  }else if(phase===6){
   check(card("done").sharedRow.rowMenu.visible,"Shift+F10 opens real thread context menu")
   keys.keyClick(Qt.Key_Escape);phase=7
  }else if(phase===7){
   check(closed===0,"Escape closes menu before board")
   keys.keyClick(Qt.Key_Left);phase=8
  }else if(phase===8){
   keys.keyClick(Qt.Key_Space);check(card("a").live&&!opened,"Space expands current card without opening it")
   keys.keyClick(Qt.Key_L,Qt.ControlModifier);check(v.liveMode,"Ctrl+L toggles global live mode")
   keys.keyClick(Qt.Key_E);phase=9
  }else if(phase===9){b.patchDraft("fixture/a",{loading:true});b.receive(JSON.stringify({type:"draft-loaded",data:{key:"fixture/a",draft:{text:"Keep this draft"},error:null}}));phase=10
  }else if(phase===10){
   var reply=keys.findChild(v,"t3BoardReply_a");check(reply.activeFocus&&reply.text==="Keep this draft"&&closed===0,"E focuses existing draft inside board")
   keys.keyClick(Qt.Key_End);keys.keyClick(Qt.Key_X);check(b.draft.text==="Keep this draftx","typing goes to draft, not board shortcuts")
   keys.keyClick(Qt.Key_Tab);check(!reply.activeFocus,"Tab leaves text editor without inserting a tab");keys.keyClick(Qt.Key_Tab,Qt.ShiftModifier);check(reply.activeFocus,"Shift+Tab returns to editor");
   keys.keyClick(Qt.Key_Left);check(v.navColumn==="idle","arrows in editor move cursor, not columns")
   fixture=Object.assign({},fixture,{threads:fixture.threads.map(function(t){return t.id==="a"?Object.assign({},t,{sidebarStatus:"working",working:true}):t})});b.state=fixture
   check(v.shownThreads.find(function(t){return t.id==="a"}).sidebarStatus==="ready","incoming state does not replace card while typing")
   keys.keyClick(Qt.Key_Escape);phase=11
  }else if(phase===11){
   check(!v.editingKey&&v.navColumn==="working"&&card("a").activeFocus&&b.draft.text==="Keep this draftx","Escape returns to card preserving draft")
   keys.keyClick(Qt.Key_N);phase=12
  }else if(phase===12){
   ideaKey=v.editingIdea;var editor=keys.findChild(v,"t3BoardIdeaEditor_"+ideaKey);check(editor&&editor.activeFocus,"N creates and focuses an independent idea")
   keys.keyClick(Qt.Key_A);keys.keyClick(Qt.Key_B);check(b.viewState.boardIdeas[0].text==="ab","idea typing saves without invoking navigation")
   keys.keyClick(Qt.Key_Return,Qt.ControlModifier);phase=13
  }else if(phase===13){
   check(!v.editingIdea&&b.viewState.boardIdeas[0].text==="ab"&&closed===0,"Ctrl+Enter finishes idea without creating thread or closing board")
   v.focusCell(v.columnFor("fixture/a")==="working"?v.columnIndex("working"):v.columnIndex("idle"),0);phase=14
  }else if(phase===14){keys.keyClick(Qt.Key_F1);phase=15
  }else if(phase===15){check(keys.findChild(v,"t3BoardKeyboardHelp").visible,"F1 opens board help");keys.keyClick(Qt.Key_Escape);phase=16
  }else if(phase===16){check(closed===0,"Escape from help must not close board");keys.keyClick(Qt.Key_Return);check(opened&&opened.id==="a","Enter opens focused thread");console.log("BOARD_KEYBOARD_QML_PASS: arrows, sorting menu, stable focus, context menu, live, draft, ideas, editor escape and enter");Qt.quit()}
 }}
}
