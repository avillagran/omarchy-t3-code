import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property real before:0
 function check(ok,msg){if(!ok){console.error("SHELF_RESIZE_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 function threads(){var r=[];for(var i=0;i<3;i++)r.push({id:"a"+i,key:"fixture/a"+i,environmentId:"fixture",project:"P",title:"Active "+i,status:"completed",updatedAt:2e12-i,activeAnchorAt:2e12-i})
  for(var j=0;j<14;j++)r.push({id:"s"+j,key:"fixture/s"+j,environmentId:"fixture",project:"P",title:"Settled "+j,settled:true,updatedAt:1e12-j});return r}
 function list(){return keys.findChild(panel,"settledThreadList")}
 function handle(){return keys.findChild(panel,"settledShelfResizer")}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:300;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({sidebarCompact:true,transitionStyle:"instant",settledShelfHeight:220});b.state={threads:threads(),projects:[],environments:[]};fixture=b.state;panel.open();panel.settledExpanded=true;phase=1
  }else if(phase===1){var l=list(),h=handle();check(l&&l.visible&&h&&h.visible,"settled shelf and its handle are shown");check(Math.abs(l.height-Style.space(220))<2,"default shelf height "+l.height);before=l.height
   var p=h.mapToItem(null,h.width/2,h.height/2);keys.mousePress(h,h.width/2,h.height/2);keys.mouseMove(h,h.width/2,h.height/2-Style.space(100));keys.mouseRelease(h,h.width/2,h.height/2-Style.space(100));phase=2
  }else if(phase===2){var l=list();check(Math.abs(panel.settledShelfHeight-320)<=2,"dragging up grows the shelf "+panel.settledShelfHeight);check(l.height>before+Style.space(90),"list follows the new height "+l.height);check(b.viewState.settledShelfHeight===panel.settledShelfHeight,"height is remembered")
   handle().forceActiveFocus();keys.keyClick(Qt.Key_Down);check(Math.abs(panel.settledShelfHeight-(b.viewState.settledShelfHeight))<1&&panel.settledShelfHeight<320,"arrow keys step the height");phase=3
  }else if(phase===3){var h=handle();keys.mouseDoubleClickSequence(h,h.width/2,h.height/2);check(panel.settledShelfHeight===220,"double click restores the default");panel.resizeShelf("settledShelfHeight",5);check(panel.settledShelfHeight===60,"minimum clamp")
   panel.resizeShelf("workingShelfHeight",400);check(panel.workingShelfHeight===400&&b.viewState.workingShelfHeight===400,"working shelf resizes too")
   console.log("SHELF_RESIZE_QML_PASS: drag, keyboard, double click, clamp and persistence");panel.close();Qt.callLater(Qt.quit)}
 }}
}
