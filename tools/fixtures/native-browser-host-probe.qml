import QtQuick
import QtQuick.Window
import QtWebEngine
Window {
 id:root;width:640;height:480;visible:true
 property bool painting:false
 Component.onCompleted:{let script=WebEngine.script();script.injectionPoint=WebEngineScript.DocumentCreation;script.worldId=WebEngineScript.MainWorld;script.sourceCode="Object.defineProperty(globalThis,'__o3PrivateCdpMinimal',{value:true});"+bridgeSource+"\n"+nativeSource;inspector.userScripts.insert(script);page.devToolsView=inspector}
 WebEngineView{id:other;anchors.fill:parent;url:'data:text/html,<style>body{background:%23ff0000}</style><title>unrelated</title>'}
 WebEngineView{id:page;anchors.fill:parent;visible:root.painting;opacity:0;url:'data:text/html,<style>html,body{margin:0;background:%23123456}</style><title>owned</title><input id="typed" autofocus><p>private</p>'}
 WebEngineView{id:inspector;width:600;height:400;visible:false;onJavaScriptConsoleMessage:function(level,message){console.log('INSPECTOR',message)}}
 Timer{interval:1000;running:true;repeat:false;onTriggered:{
  const expected=page.devToolsId;
  inspector.runJavaScript(`(async()=>{
    const expected=${JSON.stringify(expected)};
    const requests=new Map(),listeners=new Set(),navigation=new Set();let sequence=0,inflight=0,maxInflight=0;
    const completed=new Map();
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
      if(globalThis.__qtHost){const output=__qtHost.take();for(const raw of output.events){const event=JSON.parse(raw);if(event.type==='cdp'){const reply=JSON.parse(event.message);const done=completed.get(reply.id);if(done){completed.delete(reply.id);done(reply);}}}stats.captures.push(...output.events.map(raw=>JSON.parse(raw)).filter(v=>v.type==='cdp').map(v=>JSON.parse(v.message)).filter(v=>v.id===11||v.id===12||v.id===24));return {painting:output.painting,attached:output.events.some(raw=>JSON.parse(raw).type==='attached')};}
      return {};
    };
    const info=await send('Target.getTargetInfo');if(info.targetInfo.targetId!==expected)throw Error('actual target mismatch');stats.target=expected;
    const agent=await send('Runtime.evaluate',{expression:'navigator.userAgent',returnByValue:true});
    const host=await T3NativeDesktopHost.create();globalThis.__qtHost=host;
    const platform=T3QtGuestInput.create(send,()=>({url:info.targetInfo.url,title:info.targetInfo.title,userAgent:agent.result.value,activeFocus:globalThis.__qtActualFocus===true}),code=>{stats.failure=code});
    const webContents=Object.assign(platform.webContents,{on:(name,fn)=>{if(name==='did-navigate')navigation.add(fn)},off:(name,fn)=>navigation.delete(fn)});
    host.attach({threadId:'thread',tabId:'tab'},{webContents,debugger:{sendCommand:platform.sendCommand,on:(name,fn)=>{if(name==='message')listeners.add(fn)},off:(name,fn)=>listeners.delete(fn)}});
    host.setShown({threadId:'thread',tabId:'tab'},false);
    globalThis.__qtCommand=(id,method,params={},sessionId)=>host.command(JSON.stringify({type:'cdp',threadId:'thread',tabId:'tab',message:JSON.stringify({id,method,params,...(sessionId?{sessionId}:{})})}));
    globalThis.__qtRequest=(id,method,params={},sessionId)=>new Promise((resolve,reject)=>{completed.set(id,reply=>reply.error?reject(Error(reply.error.message)):resolve(reply.result));__qtCommand(id,method,params,sessionId).catch(reject);});
  })()`);
 }}
 Timer{interval:20;running:true;repeat:true;onTriggered:inspector.runJavaScript('globalThis.__qtActualFocus='+JSON.stringify(page.activeFocus)+'; typeof __qtPump === "function" ? __qtPump() : {}',function(value){if(value&&value.painting)for(const event of value.painting)root.painting=event.active})}
 Timer{interval:2300;running:true;repeat:false;onTriggered:inspector.runJavaScript('__qtCommand(10,"Target.setAutoAttach")')}
 Timer{interval:2500;running:true;repeat:false;onTriggered:inspector.runJavaScript('__qtCommand(11,"Page.captureScreenshot",{},"t3-preview-page");__qtCommand(12,"Page.captureScreenshot",{},"t3-preview-page")')}
 Timer{interval:3500;running:true;repeat:false;onTriggered:inspector.runJavaScript(`(async()=>{
  await __qtRequest(20,'Runtime.evaluate',{expression:'document.getElementById("typed").focus()'},'t3-preview-page');
  await __qtRequest(21,'Input.insertText',{text:'old'},'t3-preview-page');
  await __qtRequest(22,'Input.dispatchKeyEvent',{type:'keyDown',key:'a',modifiers:2,commands:['selectAll','copy']},'t3-preview-page');
  await __qtRequest(23,'Input.dispatchKeyEvent',{type:'keyDown',key:'界',text:'界😀'},'t3-preview-page');
  await __qtRequest(24,'Runtime.evaluate',{expression:'document.getElementById("typed").value',returnByValue:true},'t3-preview-page');
  try{await __qtRequest(25,'Input.imeSetComposition',{text:'composition'},'t3-preview-page')}catch(error){__qtStats.imeRefusal=error.message;}
})()`) }
 Timer{interval:14000;running:true;repeat:false;onTriggered:inspector.runJavaScript('JSON.stringify(__qtStats)',function(raw){const stats=JSON.parse(raw),screens=[];stats.inputs=stats.captures.filter(reply=>reply.id===24);for(const reply of stats.captures.filter(reply=>reply.id!==24))screens.push({id:reply.id,image:reply.result?images.inspect(reply.result.data):null,error:reply.error});stats.captures=screens;console.log('NATIVE_HOST_QT_RESULT',JSON.stringify(stats));inspector.runJavaScript('__qtHost.detach({threadId:"thread",tabId:"tab"});__qtHost.close()')})}
}
