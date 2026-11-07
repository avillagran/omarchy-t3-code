import QtQuick
import QtQuick.Window
import QtWebEngine
Window {
 id: root
 width: 640; height: 480; visible: true
 property bool painting: false
 property bool ready: false
 property var waiting: []
 Component.onCompleted: {
  let script=WebEngine.script();script.injectionPoint=WebEngineScript.DocumentCreation;
  script.worldId=WebEngineScript.MainWorld;
  script.sourceCode="Object.defineProperty(globalThis,'__o3PrivateCdpMinimal',{value:true});"+privateSource;
  inspector.userScripts.insert(script);page.devToolsView=inspector;
 }
 WebEngineView { id: page; anchors.fill:parent; visible:root.painting; opacity:0; url:'about:blank'
  onLoadingChanged:function(request){if(request.status===WebEngineView.LoadSucceededStatus)privatePipe.loaded(String(page.url),page.title)}
  onTitleChanged:privatePipe.loaded(String(page.url),page.title)
 }
 WebEngineView { id: inspector; width:640; height:480; visible:false }
 Connections {
  target: privatePipe
  function onCommand(line) {
   if(!root.ready){if(root.waiting.length>=4096){Qt.quit();return;}root.waiting.push(line);return;}
   inspector.runJavaScript('__ownedHost.command('+JSON.stringify(line)+').catch(()=>{globalThis.__ownedFailure=true})');
  }
 }
 Timer {
  interval:1000;running:true;repeat:false
  onTriggered: {
   const target=page.devToolsId,key={threadId:threadKey,tabId:tabKey};
   inspector.runJavaScript(`(async()=>{
    const expected=${JSON.stringify(target)},key=${JSON.stringify(key)};
    let sequence=0;const requests=new Map(),listeners=new Set(),navigation=new Set();
    const send=(method,params={},sessionId)=>new Promise((resolve,reject)=>{
     const id=++sequence;requests.set(id,{resolve,reject});
     __o3PrivateCdp.send(JSON.stringify({id,method,params,...(sessionId===undefined?{}:{sessionId})}));
    });
    globalThis.__ownedPump=()=>{
     const drained=__o3PrivateCdp.drain();if(drained.closed||drained.failure)__ownedFailure=true;
     for(const raw of drained.messages){const value=JSON.parse(raw);
      if(value.id!==undefined){const request=requests.get(value.id);if(!request){__ownedFailure=true;continue;}requests.delete(value.id);if(value.error)request.reject(Error(value.error.message));else request.resolve(value.result);}
      else if(!globalThis.__ownedGuard||__ownedGuard.event(value.method,value.params,value.sessionId)){
       if(value.method==='Page.frameNavigated'&&!value.params?.frame?.parentId)for(const fn of navigation)fn();
       for(const fn of listeners)fn(null,value.method,value.params,value.sessionId);
      }
     }
     return globalThis.__ownedHost?__ownedHost.take():{events:[],painting:[]};
    };
    const info=await send('Target.getTargetInfo');if(info.targetInfo.targetId!==expected)throw Error('private_target_mismatch');
    const guard=T3QtTargetScope.create(expected,send,()=>{__ownedFailure=true});globalThis.__ownedGuard=guard;
    const agent=await guard.sendCommand('Runtime.evaluate',{expression:'navigator.userAgent',returnByValue:true});
    const platform=T3QtGuestInput.create(guard.sendCommand,()=>({...globalThis.__ownedSnapshot,userAgent:agent.result.value}),()=>{__ownedFailure=true});
    const webContents=Object.assign(platform.webContents,{on:(name,fn)=>{if(name==='did-navigate')navigation.add(fn)},off:(name,fn)=>navigation.delete(fn)});
    const host=await T3NativeDesktopHost.create();globalThis.__ownedHost=host;
    host.attach(key,{webContents,debugger:{sendCommand:platform.sendCommand,on:(name,fn)=>{if(name==='message')listeners.add(fn)},off:(name,fn)=>listeners.delete(fn)}});
    host.setShown(key,false);
   })().catch(()=>{globalThis.__ownedFailure=true})`);
  }
 }
 Timer {
  interval:10;running:true;repeat:true
  onTriggered: {
   const snapshot={url:String(page.url),title:page.title,activeFocus:page.activeFocus};
   inspector.runJavaScript('globalThis.__ownedSnapshot='+JSON.stringify(snapshot)+';typeof __ownedPump==="function"?({...__ownedPump(),failure:globalThis.__ownedFailure===true}):({events:[],painting:[]})',function(output){
    if(!output)return;
    if(output.failure||output.closed){Qt.quit();return;}
    for(const event of output.painting||[])root.painting=event.active;
    for(const event of output.events||[]){
     privatePipe.send(event);
     if(!root.ready&&JSON.parse(event).type==='attached'){
      root.ready=true;const waiting=root.waiting;root.waiting=[];
      for(const line of waiting)inspector.runJavaScript('__ownedHost.command('+JSON.stringify(line)+').catch(()=>{globalThis.__ownedFailure=true})');
     }
    }
   });
  }
 }
}
