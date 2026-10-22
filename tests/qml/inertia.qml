import QtQuick
import QtQuick.Window 6.11
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/WheelMotion.js" as Motion
ShellRoot {
 id:root;property int phase:0;property var policy:({mouseNative:false,touchpadNative:false});property real before:0;property int coastTicks:0
 function check(ok,msg){if(!ok){console.error("T3_INERTIA_QML_FAIL: "+msg);Qt.exit(1)}}
 Window{id:win;width:600;height:500;visible:true
  TestCase{id:keys;when:false}
  Flickable{id:outer;anchors.fill:parent;contentHeight:2000;boundsBehavior:Flickable.StopAtBounds
   Test.T3Wheel{id:wheel;flickable:outer;policy:root.policy}
   Rectangle{y:10;width:400;height:250;color:"#222222"
    Flickable{id:inner;anchors.fill:parent;contentHeight:800;clip:true;boundsBehavior:Flickable.StopAtBounds
     Test.T3Wheel{id:innerWheel;flickable:inner;policy:root.policy}
     Rectangle{width:inner.width;height:inner.contentHeight;color:"#333333"}
    }
   }
  }
 }
 Timer{id:pixelBurst;interval:16;onTriggered:wheel.scroll(0,-20,true)}
 Timer{interval:240;running:true;repeat:true;onTriggered:{
  if(phase===0){
   check(Motion.glide.speed===1.3&&Motion.glide.max===4&&Motion.glide.coasting===1000,"requested Glide parameters")
   var first=Motion.step({},120,0,false,1000),burst=Motion.step(first.state,120,0,false,1001),reverse=Motion.step(burst.state,-120,0,false,1002),other=Motion.step(burst.state,0,-15,true,1003)
   check(first.factor===1&&burst.factor===4&&burst.continueTarget,"adaptive acceleration reaches bounded 4x")
   check(reverse.factor===1&&!reverse.continueTarget&&reverse.delta<0,"reversal cancels accumulated travel immediately")
   check(other.factor===1&&!other.continueTarget,"devices do not amplify each other")
   var p1=Motion.plan({},0,-20,true,false,1000),p2=Motion.plan(p1.state,0,-20,true,false,1016),pn=Motion.plan(p1.state,0,-20,true,true,1016)
   check(p2.coast< -100&&pn.coast===0,"fallback seeds a velocity coast; compositor streams never receive a second coast")
   check(Motion.duration(20)<150&&Motion.duration(216)===1000&&Motion.duration(2000)===1000,"one detent retains a full coast; tiny edge distances settle quickly")
   keys.mouseWheel(inner,100,100,0,-120);check(innerWheel.running,"fallback starts local inertia");innerWheel.scroll(0,0,true);check(innerWheel.running,"zero-delta gesture end preserves coast");phase=1
  }else if(phase===1){check(innerWheel.running,"coast survives after wheel release");check(inner.contentY>100&&outer.contentY===0,"nested viewport consumes wheel once");inner.contentY=inner.contentHeight-inner.height;keys.mouseWheel(inner,100,100,0,-120);phase=2
  }else if(phase===2){check(outer.contentY>100,"nested edge propagates to outer viewport");policy={mouseNative:true,touchpadNative:true};before=outer.contentY;check(wheel.scroll(-120,0,false),"native mouse input accepted");check(wheel.running&&outer.contentY===before&&wheel.targetY===before+216,"angle-only native wheel eases without adding travel");wheel.stop();before=outer.contentY;wheel.scroll(0,-60,true);check(!wheel.running&&outer.contentY===before+60,"native touchpad passes precise delta");policy={mouseNative:true,touchpadNative:false};wheel.scroll(0,-60,true);check(wheel.running,"per-device touchpad fallback despite native mouse");phase=3
  }else if(phase===3){if(++coastTicks<5)return;outer.contentY=0;outer.contentHeight=outer.height+24;policy={mouseNative:false,touchpadNative:false};wheel.scroll(-120,0,false);phase=4;return;}else if(phase===4){check(Math.abs(outer.contentY-24)<0.5,"small overflow reaches edge within one tick");check(!wheel.running,"no idle animation after 1000ms coast");outer.contentHeight=4000;outer.contentY=500;wheel.stop();wheel.scroll(0,-20,true);pixelBurst.start();phase=5;return;}else if(phase===5){check(wheel.coasting&&wheel.running&&wheel.targetY>600,"released pixel gesture receives velocity travel beyond its input distance");wheel.scroll(0,0,true);check(wheel.running,"gesture end preserves velocity coast");var y=outer.contentY;wheel.scroll(0,20,true);check(!wheel.coasting&&wheel.targetY<y,"new opposite input cancels remaining projected coast");phase=6;return;}else if(phase===6){check(!wheel.running,"reversal does not leave a stale coast timer");console.log("T3_INERTIA_QML_PASS: nested wheel/edge propagation, fallback, native bypass and independent devices");Qt.quit()}
 }}
}
