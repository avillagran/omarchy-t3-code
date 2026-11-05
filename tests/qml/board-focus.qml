import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var fixture:null
 property string ideaKey:""
 function check(ok,msg){if(!ok){console.error("BOARD_FOCUS_QML_FAIL: "+msg);Qt.exit(1)}}
 function card(){return keys.findChild(board,"t3BoardThread_selected")}
 function child(name){return keys.findChild(board,name)}
 function capture(name){board.grabToImage(function(g){g.saveToFile("/var/tmp/o3-board-focus-"+name+".png")})}
 TestCase{id:keys;when:false}
 FloatingWindow {
  implicitWidth:1220;implicitHeight:800;visible:true
  Test.T3ThreadBoard {
   id:board;anchors.fill:parent;bridge:Test.T3Backend
   threads:root.fixture?root.fixture.threads:[]
   sectionData:root.fixture?[{key:"ready",label:"Ready ({count})",rows:root.fixture.threads},{key:"working",label:"Working ({count})",rows:[]}]:[]
  }
 }
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer {interval:220;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,c=card()
  if(phase===0){
   if(!b.viewStateLoaded)return
   fixture={threads:[{id:"first",title:"Primer hilo"},{id:"selected",title:"Tablero de hilos y espacio de ideas"}].map(function(t){return Object.assign({key:"fixture/"+t.id,environmentId:"fixture",environmentName:"NODE",project:"o3-code",projectKey:"fixture/p",projectId:"p",sidebarStatus:"ready",canSnooze:true},t)}),projects:[],environments:[{id:"fixture",name:"NODE",state:"ready"}]}
   b.state=fixture;b.catalogs={fixture:{canWrite:true,models:[],providers:[],capabilities:{threadSettlement:true,threadSnooze:true,threadPinning:true}}}
   b.selectedEnvironment="fixture";b.selectedThread="selected";b.selectedKey="fixture/selected"
   b.rememberView({interfaceLayout:"focus",boardIdeas:[],boardColumns:{},boardLive:false});keys.parent=board;phase=20
  }else if(phase===20){board.focusCell(board.columnIndex("ready"),0);phase=1
  }else if(phase===1){
   check(child("t3BoardThread_first").activeFocus,"arrows focus the card");check(!child("t3BoardThread_first").sharedRow.keyboardFrame,"composite card owns keyboard outline");keys.keyClick(Qt.Key_Down);phase=2
  }else if(phase===2){
   check(c.activeFocus&&c.border.width===1,"one visible card outline");check(c.sharedRow.selected&&c.sharedRow.keyboardCurrent,"selected thread and keyboard destination remain distinct states");check(keys.findChild(c.sharedRow,"threadSelectionBackground").border.width===0,"selected shared row does not add a second focus outline");check(c.height-c.sharedRow.height<=Style.space(10),"compact outline fits the row with symmetric padding");capture("compact");phase=3
  }else if(phase===3){child("t3BoardExpand_selected").forceActiveFocus(Qt.TabFocusReason);phase=4
  }else if(phase===4){
   check(child("t3BoardExpand_selected").activeFocus&&child("t3BoardExpand_selected").border.width===1,"action has its own bounded focus");check(c.border.width===0,"compact card drops its outline when a child action takes focus");board.focusCell(board.columnIndex("ready"),1);phase=5
  }else if(phase===5){keys.keyClick(Qt.Key_Space);phase=6
  }else if(phase===6){
   check(c.activeFocus&&c.live&&c.border.width===1,"live card uses same single outline");check(keys.findChild(c.sharedRow,"threadSelectionBackground").border.width===0,"live header has no nested focus frame");capture("live");phase=14
  }else if(phase===14){board.writeTo(fixture.threads[1]);b.patchDraft("fixture/selected",{loading:false,text:"Mensaje de prueba"});phase=7
  }else if(phase===7){
   var reply=child("t3BoardReply_selected"),frame=child("t3BoardReplyFrame_selected");check(reply.activeFocus,"editor owns keyboard focus");check(String(frame.border.color)===String(Color.accent)&&String(c.border.color)!==String(Color.accent),"only the actual editor has the accent outline");capture("editor");phase=15
  }else if(phase===15){keys.keyClick(Qt.Key_Escape);phase=8
  }else if(phase===8){
   check(c.activeFocus,"Escape returns focus to card");board.focusCell(board.columnIndex("working"),0);phase=9
  }else if(phase===9){
   var column=board.columnAt(board.columnIndex("working")),header=child("t3BoardHeaderFocus_working");check(column.header.activeFocus,"empty column focuses header");check(column.border.width===0&&header.border.width===1&&header.height<=Style.space(32),"header focus stays in header instead of outlining full column");capture("header");phase=13
  }else if(phase===13){board.configureColumn("working",{collapsed:true});board.restoreFocus();phase=10
  }else if(phase===10){
   var rail=child("t3BoardRail_working");check(rail.activeFocus&&rail.background.border.width===1,"collapsed column has one rail outline");check(board.columnAt(board.columnIndex("working")).border.width===0,"rail does not get a duplicate outer frame");keys.keyClick(Qt.Key_N);phase=11
  }else if(phase===11){
   ideaKey=board.editingIdea;var editor=child("t3BoardIdeaEditor_"+ideaKey),idea=child("t3BoardIdea_"+ideaKey);check(editor&&editor.activeFocus,"idea editor has keyboard focus");check(idea&&String(idea.border.color)===String(Color.accent),"idea editor focus is visible");capture("idea");phase=12
  }else if(phase===12){console.log("BOARD_FOCUS_QML_PASS: single compact/live outline, tight geometry, selected state, action/editor/header/rail/idea focus and Escape");Qt.quit()}
 }}
}
