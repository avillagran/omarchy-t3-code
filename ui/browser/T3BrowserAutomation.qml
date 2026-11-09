import QtQuick
import QtWebEngine
Item {
 id: automation
 required property var owner
 required property string instance
 property bool started:false
 property bool starting:false
 property bool closing:false
 property bool busy:false
 property int attempts:0
 readonly property string threadId:owner.threadId
 readonly property string tabId:owner.tabId
 signal painting(bool active)
 signal pointer(string phase, real x, real y)
 function packet(kind,extra){return Object.assign({kind:kind,threadId:threadId,tabId:tabId,instance:instance},extra||{})}
 function write(value){if(browserPrivatePipe&&browserPrivatePipe.open)browserPrivatePipe.send(JSON.stringify(value)+"\n")}
 function stop(){
  if(closing)return
  closing=true;painting(false)
  write(packet("retired"))
  if(started)inspector.runJavaScript('globalThis.__o3OwnedInspector?.close()')
 }
 function accept(value){
  if(closing||value.threadId!==threadId||value.tabId!==tabId||value.instance!==instance)return
  if(!started)return
  inspector.runJavaScript('__o3OwnedInspector.accept('+JSON.stringify(value)+')')
 }
 Component.onCompleted:{
  // Rust allocated and registered this actual object's nonce before its open command.
  write(packet("expected"))
  const script=WebEngine.script();script.injectionPoint=WebEngineScript.DocumentCreation;script.worldId=WebEngineScript.MainWorld
  script.sourceCode="Object.defineProperty(globalThis,'__o3PrivateCdpMinimal',{value:true});\n"+browserAutomationSource
  inspector.userScripts.insert(script);owner.web.devToolsView=inspector
 }
 Component.onDestruction:stop()
 WebEngineView{id:inspector;width:640;height:480;visible:false}
 Timer{
  interval:20;repeat:true;running:!automation.started&&!automation.closing
  onTriggered:{
   if(automation.starting)return
   if(++automation.attempts>250){automation.stop();return}
   const target=owner.web.devToolsId
   if(!target||!browserPrivatePipe||!browserPrivatePipe.open)return
   automation.starting=true
   const identity={threadId:automation.threadId,tabId:automation.tabId,instance:automation.instance,target:target,userAgent:browserProfile.httpUserAgent}
   inspector.runJavaScript('(typeof T3QtOwnedInspector!=="undefined"&&typeof __o3PrivateCdp!=="undefined")?(globalThis.__o3OwnedInspector=T3QtOwnedInspector.create('+JSON.stringify(identity)+'),true):false',function(ready){
    if(automation.closing)return
    automation.starting=false;automation.started=ready===true
   })
  }
 }
 Timer{
  interval:10;repeat:true;running:automation.started&&!automation.closing
  onTriggered:{
   if(automation.busy)return
   automation.busy=true
   const snapshot={url:String(owner.web.url),title:owner.web.title,userAgent:browserProfile.httpUserAgent,activeFocus:owner.web.activeFocus,shown:owner.selected}
   inspector.runJavaScript('(()=>{try{__o3OwnedInspector.snapshot('+JSON.stringify(snapshot)+');return __o3OwnedInspector.drain()}catch(_){__o3OwnedInspector.close();return __o3OwnedInspector.drain()}})()',function(frames){
    if(automation.closing)return
    automation.busy=false
    if(!frames)return
    for(const line of frames){
     const value=JSON.parse(line)
     if(value.instance!==automation.instance||value.threadId!==automation.threadId||value.tabId!==automation.tabId){automation.stop();return}
     if(value.kind==="painting")automation.painting(value.active===true)
     else if(value.kind==="pointer")automation.pointer(value.event.phase,value.event.x,value.event.y)
     else{
      if(browserPrivatePipe&&browserPrivatePipe.open)browserPrivatePipe.send(line+"\n")
      if(value.kind==="retired"){automation.closing=true;automation.painting(false);return}
     }
    }
   })
  }
 }
}
