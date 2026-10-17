import QtQuick
import "../js/WheelMotion.js" as Motion

// One-shot easing only when the compositor is not supplying coast for this device.
// Nested scrolls decline at their edges so the enclosing viewport can continue.
Item {
 id:root
 anchors.fill:parent
 required property var flickable
 parent:flickable
 property var policy:T3Backend.scrollPolicy
 property real targetY:0
 property var motionState:({})
 readonly property int coastingDuration:Motion.glide.coasting
 readonly property bool running:ease.running
 function stop(){ease.stop();motionState=({})}
 function scroll(angle,pixel,touchpad){
  var f=flickable;if(!f||!f.visible||f.height<=0)return false
  var lo=f.originY || 0,hi=lo+Math.max(0,f.contentHeight-f.height),delta=pixel!==0?pixel:angle*1.8
  if(delta===0||hi<=lo||(delta>0&&f.contentY<=lo+0.5)||(delta<0&&f.contentY>=hi-0.5)){stop();return false}
  var native=touchpad?policy.touchpadNative:policy.mouseNative
  var step=native?null:Motion.step(motionState,angle,pixel,touchpad,Date.now())
  var from=!native&&ease.running&&step.continueTarget?targetY:f.contentY;ease.stop()
  if(native)motionState=({});else{motionState=step.state;delta=step.delta}
  targetY=Math.max(lo,Math.min(hi,(native?f.contentY:from)-delta))
  if(native)f.contentY=targetY;else {ease.duration=Motion.duration(targetY-f.contentY);ease.start()}
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
 NumberAnimation{id:ease;target:root.flickable;property:"contentY";to:root.targetY;duration:root.coastingDuration;easing.type:Easing.OutCubic}
 Connections{target:root.flickable;function onDraggingChanged(){if(root.flickable.dragging)root.stop()}function onVisibleChanged(){if(!root.flickable.visible)root.stop()}}
}
