import QtQuick
import QtTest
import qs.Commons
import Quickshell
import Quickshell.Wayland
import "UnderTest/ui/qml" as Test
// The O3 wordmark: O at rest, morphs to T, spells OMARCHY, and comes back.
ShellRoot{
 id:root;property int phase:0;property bool done:false;property int shot:0
 function check(ok,msg){if(!ok){console.error("T3_BRAND_MARK_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 FloatingWindow{
  id:win;implicitWidth:320;implicitHeight:110;color:"#1b1712";visible:true
  Item{id:stage;anchors.fill:parent
  Rectangle{anchors.fill:parent;color:"#1b1712"}
  Test.T3BrandMark{id:brand;x:24;y:20;fg:"#e8e2d6";dim:"#8a8377";accent:"#e0a030";restMs:400;holdMs:600;running:true;variant:Quickshell.env("O3_BRAND_GLYPH")||"o"}}
 }
 TestCase{id:keys;when:false}
 function grab(name){var n=root.shot++;stage.grabToImage(function(r){r.saveToFile("/tmp/o3-brand-"+brand.variant+"-"+(n<10?"0":"")+n+"-"+name+".png")})}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 Timer{interval:90;running:!root.done;repeat:true;onTriggered:{
  root.phase++
  if(root.phase===1){root.check(brand.morph===0,"rests as an O");root.check(brand.outline[0].length===brand.shapes.tOuter.length+1,"closed outline");root.check(brand.flights.length>=90&&brand.wordmark.length===7,"logo blocks and seven letters");root.grab("rest")}
  if(root.phase>3&&root.phase<90&&root.phase%2===0)root.grab("s"+brand.step)
  if(root.phase===55)root.check(brand.step>=2,"cycle advanced to spelling: "+brand.step)
  if(root.phase===95){root.check(brand.width>40&&brand.height>20,"has a size");console.log("T3_BRAND_MARK_QML_PASS");root.done=true}
 }}
}
