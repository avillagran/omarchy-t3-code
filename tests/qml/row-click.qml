import QtQuick
import Quickshell
import QtTest
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int frames:0
 property var at:null
 function check(ok,msg){if(!ok){console.error("ROW_CLICK_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 function byId(id){var v=panel.sidebarView;for(var i=0;i<v.count;i++){var r=v.itemAtIndex(i);if(r&&r.thread&&r.thread.id===id)return r}return null}
 function threads(order){return order.map(function(id,i){return {key:"a/"+id,id:id,environmentId:"a",projectKey:"a/p",project:"omarchy-t3-code",projectMonogram:"OT",title:"Thread "+id,activeOrderKey:String.fromCharCode(97+i),updatedAt:1000}})}
 Timer {interval:150;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,layout=keys.findChild(panel,"t3PanelLayout")
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.rememberView({threadRowSize:"large"})
   b.state={environments:[{id:"a",name:"omarchy"}],projects:[{key:"a/p",name:"omarchy-t3-code"}],threads:root.threads(["one","two","three"])}
   panel.open();phase=1
  } else if(phase===1){
   var row=byId("two");if(!row)return
   // Press on "two"; the list reorders under the pointer before the release.
   root.at=row.mapToItem(layout,row.width/2,row.height/2)
   keys.mousePress(layout,root.at.x,root.at.y)
   b.snapshotAboutToChange();b.state={environments:b.state.environments,projects:b.state.projects,threads:root.threads(["two","one","three"])}
   frames=0;phase=2
  } else if(phase===2){
   if(++frames<3)return
   keys.mouseRelease(layout,root.at.x,root.at.y)
   check(b.selectedKey==="a/two","a click survives the row moving under the pointer, selected "+b.selectedKey)
   // A press that turns into a scroll drag selects nothing.
   var row=byId("three");root.at=row.mapToItem(layout,row.width/2,row.height/2)
   keys.mousePress(layout,root.at.x,root.at.y);keys.mouseMove(layout,root.at.x,root.at.y+Style.space(60));keys.mouseRelease(layout,root.at.x,root.at.y+Style.space(60))
   check(b.selectedKey==="a/two","dragging away from a row does not select it")
   var one=byId("one");keys.mouseClick(one,one.width/2,one.height/2)
   check(b.selectedKey==="a/one","a plain click selects")
   // Hover until the hover card opens, then click another row.
   var three=byId("three");keys.mouseMove(three,three.width/2,three.height/2);frames=0;phase=3
  } else if(phase===3){
   var three=byId("three");keys.mouseMove(three,three.width/2+1,three.height/2)
   if(++frames<6)return
   check(three.previewPopup.opened,"the hover card opens")
   var two=byId("two");keys.mouseClick(two,two.width/2,two.height/2)
   check(b.selectedKey==="a/two","a click lands while another row shows its hover card, selected "+b.selectedKey)
   // Many rows, a flick, and a click while the list still glides.
   b.state={environments:b.state.environments,projects:b.state.projects,threads:root.threads(["one","two","three","four","five","six","seven","eight","nine","ten","eleven","twelve","thirteen","fourteen","fifteen","sixteen"])}
   frames=0;phase=4
  } else if(phase===4){
   if(++frames<3)return
   // The list only scrolls by wheel; a press-drag never glides and steals clicks.
   var v=panel.sidebarView,first=byId("one"),y0=v.contentY
   keys.mousePress(first,first.width/2,first.height/2);keys.mouseMove(first,first.width/2,first.height/2-Style.space(80));keys.mouseRelease(first,first.width/2,first.height/2-Style.space(80))
   check(!v.moving&&v.contentY===y0,"a press-drag does not glide the list, contentY "+v.contentY+" origin "+v.originY+" interactive "+v.interactive)
   keys.mouseWheel(v,v.width/2,v.height/2,0,-360)
   var r=null;for(var i=0;i<v.count;i++){var it=v.itemAtIndex(i);if(it&&it.y>v.contentY+it.height&&it.y<v.contentY+v.height-2*it.height){r=it;break}}
   var key=r.thread.key;keys.mouseClick(r,r.width/2,r.height/2)
   check(v.contentY>0,"the wheel scrolls the list");check(b.selectedKey===key,"a click right after scrolling selects "+key+", selected "+b.selectedKey)
   panel.close();console.log("ROW_CLICK_QML_PASS: plain clicks, clicks during reorders, beside hover cards and right after scrolling; drags never glide");Qt.quit()
  }
 }}
}
