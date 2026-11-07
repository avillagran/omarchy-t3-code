import QtQuick
import QtQuick.Window
import QtWebEngine
Window {
 id:root;width:640;height:480;visible:true
 property bool painting:false
 Component.onCompleted:{let script=WebEngine.script();script.injectionPoint=WebEngineScript.DocumentCreation;script.worldId=WebEngineScript.MainWorld;script.sourceCode="Object.defineProperty(globalThis,'__o3PrivateCdpMinimal',{value:true});"+bridgeSource+"\n"+nativeSource;inspector.userScripts.insert(script);page.devToolsView=inspector}
 WebEngineView{id:other;anchors.fill:parent;url:'data:text/html,<style>body{background:%23ff0000}</style><title>unrelated</title>'}
 WebEngineView{id:page;anchors.fill:parent;visible:root.painting;opacity:0;url:'data:text/html,<style>html,body{margin:0;background:%23123456}</style><title>owned</title><p>private</p>'}
 WebEngineView{id:inspector;width:600;height:400;visible:false;onJavaScriptConsoleMessage:function(level,message){console.log('INSPECTOR',message)}}
 Timer{interval:1000;running:true;repeat:false;onTriggered:{
  const expected=page.devToolsId;
  inspector.runJavaScript(`(async()=>{
    const expected=${JSON.stringify(expected)};
    const requests=new Map(),listeners=new Set(),navigation=new Set();let sequence=0,inflight=0,maxInflight=0;
    const stats={calls:[],captures:[],target:null,closed:false};
    globalThis.__qtStats=stats;
    const send=(method,params={},sessionId)=>new Promise((resolve,reject)=>{
      const id=++sequence;requests.set(id,{resolve,reject,method});stats.calls.push({method,params});
      if(method==='Page.captureScreenshot'){inflight++;maxInflight=Math.max(maxInflight,inflight);stats.maxInflight=maxInflight;}
      __o3PrivateCdp.send(JSON.stringify({id,method,params,...(sessionId?{sessionId}:{})}));
    });
    globalThis.__qtPump=()=>{
      for(const raw of __o3PrivateCdp.drain().messages){const value=JSON.parse(raw);
        if(value.id!==undefined){const request=requests.get(value.id);if(!request)throw Error('unknown actual reply');requests.delete(value.id);if(request.method==='Page.captureScreenshot')inflight--;if(value.error)request.reject(Error(value.error.message));else request.resolve(value.result);}
        else {if(value.method==='Page.frameNavigated'&&!value.params?.frame?.parentId)for(const fn of navigation)fn();for(const fn of listeners)fn(null,value.method,value.params,value.sessionId);}
      }
      if(globalThis.__qtHost){const output=__qtHost.take();stats.captures.push(...output.events.map(raw=>JSON.parse(raw)).filter(v=>v.type==='cdp').map(v=>JSON.parse(v.message)).filter(v=>v.id===11||v.id===12));return {painting:output.painting,attached:output.events.some(raw=>JSON.parse(raw).type==='attached')};}
      return {};
    };
    const info=await send('Target.getTargetInfo');if(info.targetInfo.targetId!==expected)throw Error('actual target mismatch');stats.target=expected;
    const agent=await send('Runtime.evaluate',{expression:'navigator.userAgent',returnByValue:true});
    const host=await T3NativeDesktopHost.create();globalThis.__qtHost=host;
    const webContents={getURL:()=>info.targetInfo.url,getTitle:()=>info.targetInfo.title,getUserAgent:()=>agent.result.value,on:(name,fn)=>{if(name==='did-navigate')navigation.add(fn)},off:(name,fn)=>navigation.delete(fn)};
    host.attach({threadId:'thread',tabId:'tab'},{webContents,debugger:{sendCommand:send,on:(name,fn)=>{if(name==='message')listeners.add(fn)},off:(name,fn)=>listeners.delete(fn)}});
    host.setShown({threadId:'thread',tabId:'tab'},false);
    globalThis.__qtCommand=(id,method,params={},sessionId)=>host.command(JSON.stringify({type:'cdp',threadId:'thread',tabId:'tab',message:JSON.stringify({id,method,params,...(sessionId?{sessionId}:{})})}));
  })()`);
 }}
 Timer{interval:20;running:true;repeat:true;onTriggered:inspector.runJavaScript('typeof __qtPump === "function" ? __qtPump() : {}',function(value){if(value&&value.painting)for(const event of value.painting)root.painting=event.active})}
 Timer{interval:2300;running:true;repeat:false;onTriggered:inspector.runJavaScript('__qtCommand(10,"Target.setAutoAttach")')}
 Timer{interval:2500;running:true;repeat:false;onTriggered:inspector.runJavaScript('__qtCommand(11,"Page.captureScreenshot",{},"t3-preview-page");__qtCommand(12,"Page.captureScreenshot",{},"t3-preview-page")')}
 Timer{interval:14000;running:true;repeat:false;onTriggered:inspector.runJavaScript('JSON.stringify(__qtStats)',function(raw){const stats=JSON.parse(raw),screens=[];for(const reply of stats.captures)screens.push({id:reply.id,image:reply.result?images.inspect(reply.result.data):null,error:reply.error});stats.captures=screens;console.log('NATIVE_HOST_QT_RESULT',JSON.stringify(stats));inspector.runJavaScript('__qtHost.detach({threadId:"thread",tabId:"tab"});__qtHost.close()')})}
}
