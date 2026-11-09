import QtQuick
import QtTest
import qs.Commons
import Quickshell
import "UnderTest/ui/qml" as Test
// A window narrow enough for a vertical chat: the thread list and the chat take
// turns over the whole width, and the header keeps only what fits.
ShellRoot{
 id:root;property int phase:0;property bool done:false;property var fixture:null
 function check(ok,msg){if(!ok){console.error("T3_NARROW_WINDOW_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 Timer{interval:250;running:!root.done;repeat:true;onTriggered:{
  var b=Test.T3Backend,layout=keys.findChild(panel,"t3PanelLayout"),side=keys.findChild(panel,"sidebarReveal"),chat=keys.findChild(panel,"t3MainChatArea"),header=keys.findChild(panel,"t3ChatHeader")
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.state={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",project:"P",title:"One",lastSentAt:2},{id:"two",key:"fixture/two",environmentId:"fixture",project:"P",title:"Two",lastSentAt:1}],environments:[]};root.fixture=b.state
   b.sidebarCollapsed=false;panel.resizePanel(340,700);panel.open();phase=1
  }else if(phase===1){
   if(!layout||layout.width<=0||side.moving)return
   root.check(panel.narrow,"340 px is narrow: "+layout.width)
   root.check(side.visible&&Math.abs(side.width-layout.width)<1,"thread list takes the whole width")
   root.check(!chat.visible,"chat is hidden behind the thread list")
   root.check(!keys.findChild(panel,"t3SidebarResizer").parent.visible,"no sidebar resizer when narrow")
   b.selectThread("fixture","one");phase=2
  }else if(phase===2){
   if(side.moving||!panel.sidebarCollapsed)return
   root.check(chat.visible&&Math.abs(chat.width-layout.width)<1,"picking a thread shows the chat over the whole width")
   root.check(header.implicitWidth<=chat.width+1,"header fits: "+header.implicitWidth+" > "+chat.width)
   root.check(!keys.findChild(panel,"t3OpenGit").visible&&!keys.findChild(panel,"t3OpenTerminal").visible,"secondary header actions hide")
   root.check(keys.findChild(panel,"t3CloseWindow").visible&&keys.findChild(panel,"t3ToggleThreadDetails").visible,"essential header actions stay")
   var send=keys.findChild(panel,"t3PrimaryAction"),area=send.mapToItem(chat,0,0);root.check(area.x+send.width<=chat.width,"send button stays inside the chat: "+area.x)
   root.check(!keys.findChild(panel,"t3SendAndNewThread").visible,"composer toolbar compacts")
   layout.grabToImage(function(r){r.saveToFile("/tmp/t3-narrow-window.png")});phase=25
  }else if(phase===25){
   panel.toggleSidebar(false);phase=3
  }else if(phase===3){
   if(side.moving)return
   root.check(side.visible&&!chat.visible,"the sidebar button brings the list back over the chat")
   panel.resizePanel(1120,780);phase=4
  }else if(phase===4){
   if(side.moving||panel.narrow===undefined)return
   root.check(!panel.narrow&&side.visible&&chat.visible&&side.width<layout.width-Style.space(400),"wide windows keep list and chat side by side")
   b.selectThread("fixture","two");phase=5
  }else if(phase===5){
   root.check(!panel.sidebarCollapsed,"picking a thread in a wide window keeps the list")
   console.log("T3_NARROW_WINDOW_QML_PASS: list and chat alternate when narrow, header fits, wide layout unchanged");done=true
  }
 }}
}
