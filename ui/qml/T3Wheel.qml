import QtQuick 6.11
import "../js/WheelMotion.js" as Motion

// Precise native streams pass through; angle-only wheels retain visual easing.
// Nested scrolls decline at their edges so the enclosing viewport can continue.
Item {
 id:root
 anchors.fill:parent
 required property var flickable
 parent:flickable
 property var policy:T3Backend.scrollPolicy
 property bool coasting:false
 property real coastDelta:0
 property real targetY:0
 property var motionState:({})
 readonly property int coastingDuration:Motion.glide.coasting
 readonly property bool running:ease.running
 function stop(){coastStart.stop();ease.stop();coasting=false;motionState=({})}
 function scroll(angle,pixel,touchpad){
  var f=flickable;if(!f||!f.visible||f.height<=0)return false
  var lo=f.originY || 0,hi=lo+Math.max(0,f.contentHeight-f.height),delta=pixel!==0?pixel:angle*1.8
  // A zero-delta ScrollEnd releases the gesture; it must not cancel its coast.
  if(delta===0)return false
  if(hi<=lo||(delta>0&&f.contentY<=lo+0.5)||(delta<0&&f.contentY>=hi-0.5)){stop();return false}
  var native=touchpad?policy.touchpadNative:policy.mouseNative
  var step=Motion.plan(motionState,angle,pixel,touchpad,native,Date.now())
  var from=!coasting&&step.animate&&ease.running&&step.continueTarget?targetY:f.contentY;ease.stop()
  coasting=false;motionState=step.state;delta=step.delta;coastDelta=step.coast
  if(coastDelta!==0)coastStart.restart();else coastStart.stop()
  targetY=Math.max(lo,Math.min(hi,from-delta))
  if(!step.animate)f.contentY=targetY;else {ease.duration=Motion.duration(targetY-f.contentY);ease.start()}
  return true
 }
 function route(angle,pixel,touchpad){
  if(scroll(angle,pixel,touchpad))return true
  var p=flickable.parent
  while(p){
   if(p.contentY!==undefined){if(typeof p.wheelScroll==="function"){p.wheelScroll(angle,pixel,touchpad);return true}var wheels=p.children.filter(function(c){return c.isT3Wheel===true});if(wheels.length&&wheels[0].scroll(angle,pixel,touchpad))return true}
   p=p.parent
  }return false
 }
 readonly property bool isT3Wheel:true
 TapHandler{parent:root.flickable;onPressedChanged:if(pressed)root.stop()}
 WheelHandler {
  parent:root.flickable
  target:null
  acceptedDevices:PointerDevice.Mouse|PointerDevice.TouchPad
  onWheel:function(event){event.accepted=root.route(event.angleDelta.y,event.pixelDelta.y,event.pixelDelta.y!==0 || event.device.type===PointerDevice.TouchPad)}
 }
 Timer{id:coastStart;interval:80;onTriggered:{
  var f=root.flickable,lo=f.originY || 0,hi=lo+Math.max(0,f.contentHeight-f.height)
  root.coasting=true;ease.stop();root.targetY=Math.max(lo,Math.min(hi,root.targetY-root.coastDelta))
  ease.duration=Motion.glide.coasting;ease.start()
 }}
 NumberAnimation{id:ease;target:root.flickable;property:"contentY";to:root.targetY;duration:root.coastingDuration;easing.type:Easing.OutCubic}
 Connections{target:root.flickable;function onDraggingChanged(){if(root.flickable.dragging)root.stop()}function onVisibleChanged(){if(!root.flickable.visible)root.stop()}}
}
