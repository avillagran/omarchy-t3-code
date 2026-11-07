import QtQuick 6.11
Item {
 id:root;objectName:"t3BrowserPage";clip:true
 // Website pixels only. Native workspace chrome is composed around this item.
 property var page:({})
 property bool interactive:false
 readonly property string requestId:page.requestId || ""
 signal input(string requestId,var event)
 signal frameReady()
 signal surfaceResized(int width,int height)
 readonly property Item nativeInputItem:imeLoader.item
 signal inputMethodRejected(string reason)
 readonly property bool ready:frame.status===Image.Ready
 function send(event){if(visible&&interactive&&requestId)input(requestId,event)}
 function point(x,y){
  var w=frame.paintedWidth,h=frame.paintedHeight,left=(width-w)/2,top=(height-h)/2;
  if(!w||!h||x<left||y<top||x>=left+w||y>=top+h)return null;
  return {x:(x-left)*(page.width||w)/w,y:(y-top)*(page.height||h)/h};
 }
 onRequestIdChanged:{moveDelay.stop();mouse.lastMove=null}
 onVisibleChanged:if(!visible){moveDelay.stop();mouse.lastMove=null}else resizeDelay.restart()
 onWidthChanged:resizeDelay.restart()
 onHeightChanged:resizeDelay.restart()
 Image {
  id:frame;objectName:"t3BrowserPageFrame";anchors.fill:parent
  source:root.page.url?root.page.url+"#"+(root.page.revision||0):"";cache:false;asynchronous:true;retainWhileLoading:true;fillMode:Image.PreserveAspectFit
  activeFocusOnTab:root.interactive&&!imeLoader.item
  onStatusChanged:if(status===Image.Ready)root.frameReady()
  // Repaints must not steal focus back from the native address bar.
  Keys.onPressed:function(e){if(!root.interactive||!root.visible)return;root.send({type:"key",key:e.key,text:e.text,modifiers:e.modifiers});e.accepted=true}
  Keys.onReleased:function(e){if(!root.interactive||!root.visible)return;root.send({type:"keyup",key:e.key,text:e.text,modifiers:e.modifiers});e.accepted=true}
  MouseArea {
   id:mouse;objectName:"t3BrowserPageInput";anchors.fill:parent;enabled:root.interactive&&root.ready;acceptedButtons:Qt.LeftButton|Qt.MiddleButton|Qt.RightButton;hoverEnabled:true
   property var lastMove:null
   function event(type,m){var p=root.point(m.x,m.y);if(!p)return false;root.send({type:type,x:p.x,y:p.y,button:m.button,buttons:m.buttons,modifiers:m.modifiers});return true}
   onPressed:function(m){if(event("press",m))(imeLoader.item||frame).forceActiveFocus();else m.accepted=false}
   onReleased:function(m){event("release",m)}
   onDoubleClicked:function(m){event("double",m)}
   onPositionChanged:function(m){var p=root.point(m.x,m.y);if(!p)return;if(pressed){event("move",m);return;}lastMove={type:"move",x:p.x,y:p.y,button:0,buttons:0,modifiers:m.modifiers};if(!moveDelay.running)moveDelay.start()}
   onWheel:function(e){var p=root.point(e.x,e.y);if(!p){e.accepted=false;return}root.send({type:"wheel",x:p.x,y:p.y,ax:e.angleDelta.x,ay:e.angleDelta.y,px:e.pixelDelta.x,py:e.pixelDelta.y,modifiers:e.modifiers});e.accepted=true}
  }
  Loader {
   id:imeLoader;anchors.fill:parent
   active:root.page.nativeInput===true
   source:active?Qt.resolvedUrl("T3BrowserInputMethod.qml"):""
   onLoaded:{
    var focused=frame.activeFocus;
    item.scopeToken=Qt.binding(function(){return root.requestId});
    item.inputEnabled=Qt.binding(function(){return root.interactive&&root.visible&&root.ready});
    // Identity and geometry only; surrounding text uses the private C++ channel.
    item.nativeBinding=Qt.binding(function(){return root.ready?(root.page.nativeInputBinding||({})):({})});
    item.nativeContentRect=Qt.binding(function(){var r=root.ready?root.page.nativeInputContent:null;return r?Qt.rect(r.x,r.y,r.width,r.height):Qt.rect(0,0,0,0)});
    item.framePresentation=Qt.binding(function(){return Qt.rect((root.width-frame.paintedWidth)/2,(root.height-frame.paintedHeight)/2,frame.paintedWidth,frame.paintedHeight)});
    if(focused)item.forceActiveFocus()
   }
  }
  Connections {
   target:imeLoader.item
   function onInput(token,event){if(token===root.requestId)root.send(event)}
   function onInputRejected(reason){root.inputMethodRejected(reason)}
  }
 }
 Timer{id:moveDelay;interval:40;onTriggered:if(mouse.lastMove)root.send(mouse.lastMove)}
 Timer{id:resizeDelay;interval:120;onTriggered:if(root.visible&&root.width>0&&root.height>0)root.surfaceResized(Math.round(root.width),Math.round(root.height))}
}
