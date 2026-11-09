// Trusted main process only. Pages receive neither these channels nor Node APIs.
'use strict';
const fs=require('node:fs'),path=require('node:path'),net=require('node:net'),vm=require('node:vm'),crypto=require('node:crypto');
const data=JSON.parse(process.env.O3_NATIVE_BROWSER_DATA||'null');delete process.env.O3_NATIVE_BROWSER_DATA;
if(!data||!Number.isInteger(data.privateBrowserFd)||data.privateBrowserFd<64||!Number.isInteger(data.privateBrowserPeerPid)||!path.isAbsolute(data.frameDirectory))throw Error('invalid_browser_payload');
const privateDescriptors=require('./electron-private-fd.node');privateDescriptors.admit(data.privateBrowserFd,data.privateBrowserPeerPid);
if(data.privateClipboardIdentityFd!==undefined)privateDescriptors.identify(data.privateClipboardIdentityFd,data.privateBrowserPeerPid);
const {app,BrowserWindow,webContents}=require('electron');
if(process.versions.electron!=='44.10.1')throw Error('unsupported_native_browser_runtime');
app.setPath('userData',data.electronDirectory);app.setPath('sessionData',data.electronDirectory);
const humanInput=require('./electron-human-input.cjs');
const realm={};vm.runInNewContext(fs.readFileSync(path.join(__dirname,'qt-target-scope.js'),'utf8'),realm);
const LIMIT=256*1024*1024,MAX_TABS=128;
let traceBytes=0;function trace(stage){const target=process.env.O3_NATIVE_ELECTRON_DIAGNOSTICS;if(target&&traceBytes<65536){const line=String(stage)+'\n';traceBytes+=line.length;fs.appendFileSync(target,line,{mode:0o600})}}
function inputErrorKind(error){const message=typeof error?.message==='string'?error.message:'';if(message==='invalid_browser_input')return 'shape';if(message==='native_browser_closed'||message==='target closed while handling command'||message==='No target available')return 'closed';if(message.includes('Internal error'))return 'cdp-internal';if(message.includes('WebFrameMain')||message==='Object has been destroyed')return 'frame-lifetime';if(message==="Only this preview tab's target and child sessions are available.")return 'scope';if(message.includes('Invalid parameters'))return 'cdp-parameters';if(message==='Empty session id is not allowed')return 'empty-session';if(message==='Invalid method')return 'invalid-method';return 'other'}
function frameErrorKind(error){return new Map([['Current display surface not available for capture','surface-unavailable'],['Unknown','copy-unknown'],['Not implemented','copy-not-implemented'],['Frame Gone','copy-frame-gone'],['Timeout','copy-timeout'],['EmbeddingTokenChanged','copy-embedding-changed'],['VizSentEmptyBitmap','copy-empty-bitmap'],['UnknownVizError','copy-viz-error'],['browser_frame_invalid','invalid-png']]).get(error?.message)||'other'}
const tabs=new Map(),dialogs=new Map();let services,selected='',surfaceId='',revision=0,closing=false,privateAlive=true,frameTicker;
const keyOf=value=>JSON.stringify([value.threadId,value.tabId]);
const pipe=new net.Socket({fd:data.privateBrowserFd,readable:true,writable:true});
function privatePost(tab,kind,fields={}){
 if(closing||tab.closed||!privateAlive)return;
 const bytes=Buffer.from(JSON.stringify({kind,...tab.key,instance:tab.instance,...fields})+'\n');
 if(bytes.length>LIMIT||pipe.writableLength>LIMIT-bytes.length)return shutdown('private_browser_output_limit');
 pipe.write(bytes);
}
function ui(event){if(closing)return;const row=JSON.stringify(event)+'\n';if(Buffer.byteLength(row)>2*1024*1024||process.stdout.writableLength>2*1024*1024)return shutdown('browser_output_limit');process.stdout.write(row)}
function status(tab,state){if(state)tab.navigation=state;ui({type:'status',...tab.key,navStatus:tab.navigation,canGoBack:tab.contents.navigationHistory.canGoBack(),canGoForward:tab.contents.navigationHistory.canGoForward(),profileId:data.profileId,runtime:'desktop'})}
let capturing=false,dirty=false;
function requestCapture(){if(data.ownedNativeHost!==true)capture()}
function scheduleFrame(){if(closing)return;frameTicker=setTimeout(async()=>{if(selected&&surfaceId)await capture();scheduleFrame()},Math.ceil(1000/12));frameTicker.unref()}
async function nativeFrame(contents){let timeout;try{return await Promise.race([contents.capturePage(),new Promise((_,reject)=>{timeout=setTimeout(()=>reject(Error('Timeout')),1000)})])}finally{clearTimeout(timeout)}}
// This reads only our trusted host container, never the target document or AX.
async function nativeContent(tab){
 if(!tab.contextTitle||tab.closed||tab.window.isDestroyed()||tab.contents.hostWebContents!==tab.window.webContents)return null;
 const before=tab.window.getBounds(),content=tab.window.getContentBounds();
 const host=await tab.window.webContents.executeJavaScript('(()=>{const view=document.querySelector("webview"),r=view.getBoundingClientRect();return {id:view.getWebContentsId(),x:r.x,y:r.y,width:r.width,height:r.height}})()');
 const after=tab.window.getBounds(),next=tab.window.getContentBounds();
 if(tab.closed||tab.window.isDestroyed()||host.id!==tab.contents.id||['x','y','width','height'].some(key=>before[key]!==after[key]||content[key]!==next[key])||!['x','y','width','height'].every(key=>Number.isFinite(host[key]))||host.width<=0||host.height<=0)return null;
 return {x:content.x-before.x+host.x,y:content.y-before.y+host.y,width:host.width,height:host.height};
}
async function capture(){
 if(closing)return;if(capturing){if(data.ownedNativeHost!==true)dirty=true;return}const tab=tabs.get(selected);if(!tab||tab.closed)return;
 capturing=true;const identity=selected,view=surfaceId,instance=tab.instance;
 try{if(!tab.guard)return;const contextBefore=tab.contextTitle?await nativeContent(tab):null;let png;if(data.ownedNativeHost===true){const image=await nativeFrame(tab.contents),size=image.getSize();if(!Number.isFinite(size.width)||!Number.isFinite(size.height)||size.width<=0||size.height<=0){trace('native-frame-not-ready');return}png=image.toPNG()}else png=Buffer.from((await tab.guard.sendCommand('Page.captureScreenshot',{format:'png',fromSurface:true,captureBeyondViewport:false})).data,'base64');if(closing||tab.closed||tab.contents.isDestroyed()||tabs.get(identity)!==tab||tab.instance!==instance||selected!==identity||surfaceId!==view)return;if(png.length<24||!png.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])))throw Error('browser_frame_invalid');
  const file='frame-'+(++revision%2)+'.png',target=path.join(data.frameDirectory,file),temporary=target+'.next';
  const fd=fs.openSync(temporary,fs.constants.O_WRONLY|fs.constants.O_CREAT|fs.constants.O_EXCL|fs.constants.O_NOFOLLOW,0o600);
  try{fs.writeFileSync(fd,png)}finally{fs.closeSync(fd)}fs.renameSync(temporary,target);
  const size={width:png.readUInt32BE(16),height:png.readUInt32BE(20)},contextAfter=tab.contextTitle?await nativeContent(tab):null,nativeInputContent=contextBefore&&JSON.stringify(contextBefore)===JSON.stringify(contextAfter)?contextAfter:null;if(closing||tab.closed||tabs.get(identity)!==tab||tab.instance!==instance||selected!==identity||surfaceId!==view)return;ui({type:'frame',file,revision,width:size.width,height:size.height,surfaceId:view,tabKey:identity,nativeInput:!!tab.contents.focusedFrame,...(nativeInputContent?{nativeContextTitle:tab.contextTitle,nativeInputContent}:{})});
 }catch(error){trace('frame-error-'+frameErrorKind(error));if(data.ownedNativeHost!==true&&!closing&&privateAlive&&!tab.closed&&tabs.get(identity)===tab&&tab.instance===instance&&selected===identity&&surfaceId===view)shutdown('browser_frame_failed')}finally{capturing=false;if(dirty&&data.ownedNativeHost!==true){dirty=false;setImmediate(capture)}}
}
function selectNativeHost(tab){if(data.ownedNativeHost!==true)return;trace('native-host-before-focusedFrame-'+Number(!!tab.contents.focusedFrame));for(const item of tabs.values()){if(item===tab){item.window.show();item.window.focus();item.contents.focus();item.window.webContents.setBackgroundThrottling(false);item.contents.setBackgroundThrottling(false)}else{item.contents.setBackgroundThrottling(true);item.window.webContents.setBackgroundThrottling(true);item.window.hide()}}trace('native-host-visible-'+Number(tab.window.isVisible())+'-focusedFrame-'+Number(!!tab.contents.focusedFrame));if(process.env.O3_NATIVE_ELECTRON_DIAGNOSTICS&&process.platform==='linux'){const handle=tab.window.getNativeWindowHandle();if(handle.length>=4)trace('native-window-xid-'+handle.readUInt32LE(0))}trace('native-host-offscreen-owner-'+Number(tab.window.webContents.isOffscreen())+'-guest-'+Number(tab.contents.isOffscreen()));if(process.env.O3_NATIVE_ELECTRON_DIAGNOSTICS)tab.window.webContents.executeJavaScript('(()=>{const view=document.querySelector("webview"),rect=view.getBoundingClientRect();return [document.visibilityState,Math.round(innerWidth),Math.round(innerHeight),Math.round(rect.width),Math.round(rect.height),view.getWebContentsId()]})()').then(state=>{if(!tab.closed&&state[0]==='visible'&&state.slice(1).every(Number.isInteger))trace('native-host-geometry-'+state.slice(1,5).join('-')+'-guest-match-'+Number(state[5]===tab.contents.id))},()=>trace('native-host-geometry-unavailable'));capture()}
function matches(tab,frame){return !tab.closed&&frame.instance===tab.instance&&frame.threadId===tab.key.threadId&&frame.tabId===tab.key.tabId}
async function retire(tab,reason){trace('retire-'+reason+'-already-'+Number(tab.closed));if(tab.closed)return;if(tab.instance)privatePost(tab,'retired',{...(tab.lease?{lease:tab.lease}:{}),code:reason});tab.closed=true;tab.guard?.close();services?.detach(tab.key);for(const [id,dialog]of dialogs)if(dialog.tab===tab){dialogs.delete(id);dialog.reject(Error('native_dialog_unavailable'));ui({type:'permission-cancelled',requestId:id})}if(!tab.window.isDestroyed())tab.window.destroy()}
async function shutdown(reason){trace(reason);if(closing)return;closing=true;if(frameTicker)clearTimeout(frameTicker);for(const tab of tabs.values())await retire(tab,reason);pipe.destroy();for(const request of dialogs.values())request.reject(Error('native_dialog_unavailable'));dialogs.clear();try{await services?.close()}finally{app.exit(reason==='browser_closed'?0:1)}}
function navigationAllowed(raw){try{const url=new URL(raw);return ['http:','https:'].includes(url.protocol)||url.href==='about:blank'||url.href.startsWith('about:blank#')}catch{return false}}
async function automate(tab,instance){trace('automate');
 if(!privateAlive||typeof instance!=='string'||!/^[0-9a-f-]{36}$/i.test(instance)||tab.closed)return;if(tab.instance===instance)return;
 if(tab.instance){privatePost(tab,'retired',{code:'private_browser_inspector_replaced',...(tab.lease?{lease:tab.lease}:{})});services.detach(tab.key);tab.guard?.close()}
 tab.instance=instance;tab.lease=undefined;privatePost(tab,'expected');trace('expected');
 if(!tab.contents.debugger.isAttached())tab.contents.debugger.attach('1.3');
 const info=await tab.contents.debugger.sendCommand('Target.getTargetInfo');
 if(tab.closed||tab.instance!==instance)return;
 if(webContents.fromDevToolsTargetId(info.targetInfo.targetId)!==tab.contents)throw Error('private_browser_target_mismatch');
 const target=info.targetInfo.targetId;
 tab.guard=realm.T3QtTargetScope.create(target,(...args)=>{if(args[0]==='Runtime.evaluate'&&typeof args[1]?.expression==='string')trace('physical-script-sha256-'+crypto.createHash('sha256').update(args[1].expression).digest('hex'));const pending=tab.contents.debugger.sendCommand(...args);return args[0]==='Emulation.setFocusEmulationEnabled'?pending.then(result=>{trace('physical-focus-emulation-'+Number(args[1]?.enabled===true));return result}):pending},()=>retire(tab,'private_browser_target_mismatch'));
 const humanGuard=tab.guard;tab.human=humanInput.create(tab.contents,{sendCommand:(method,params,sessionId)=>{trace('human-physical-'+method);if(method==='Input.imeSetComposition')trace('synthetic-preedit-exact-'+Number(params.text==='\u306b\u307b\u3093')+'-keys-'+Object.keys(params).sort().join(',')+'-plain-'+Number(Object.getPrototypeOf(params)===Object.prototype));if(method==='Input.dispatchMouseEvent')trace('human-pointer-'+params.type+'-modifiers-'+params.modifiers+'-buttons-'+params.buttons+'-clickCount-'+params.clickCount+'-x-'+params.x+'-y-'+params.y);return humanGuard.sendCommand(method,params,sessionId).then(value=>{trace('human-physical-complete-'+method);return value},error=>{trace('human-physical-failed-'+method+'-'+inputErrorKind(error));throw error})}},services.humanInput);privatePost(tab,'proof',{viewTarget:target,cdpTarget:info.targetInfo.targetId});trace('proof');
}
async function privateCommand(frame){
 if(closing||!privateAlive)return;
 const tab=tabs.get(keyOf(frame));if(!tab||!matches(tab,frame))return;
 if(frame.kind==='retire'&&frame.lease===tab.lease){await retire(tab,'private_browser_released');return}
 if(frame.kind==='admitted'){trace('admitted');
  if(!tab.guard||tab.lease!==undefined||typeof frame.lease!=='string'||!frame.lease)throw Error('private_browser_scope_mismatch');
  tab.lease=frame.lease;
  const listeners=new Set();tab.listeners=listeners;
  const observedMethods=new Set(['Target.getTargets','Target.attachToTarget','Target.getTargetInfo','Page.getNavigationHistory','Browser.getVersion','Page.enable','Page.getFrameTree','Page.navigate','Page.captureScreenshot','Runtime.enable','Runtime.evaluate','Runtime.callFunctionOn','DOM.enable','DOM.getDocument','Network.enable','Target.setDiscoverTargets','Target.setAutoAttach','Emulation.setDeviceMetricsOverride','Emulation.clearDeviceMetricsOverride','Emulation.setFocusEmulationEnabled','Input.dispatchMouseEvent','Input.dispatchKeyEvent','Input.imeSetComposition','Input.insertText']);
  const debuggerBoundary={sendCommand:(...args)=>{const method=observedMethods.has(args[0])?args[0]:'other';trace('driver-start-'+method);return tab.guard.sendCommand(...args).then(value=>{trace('driver-complete-'+method);return value},error=>{trace('driver-failed-'+method);throw error})},on:(name,fn)=>{if(name==='message')listeners.add(fn)},off:(name,fn)=>{if(name==='message')listeners.delete(fn)}};
  services.setShown(tab.key,selected===keyOf(tab.key));await services.command(JSON.stringify({type:'turn',threadId:tab.key.threadId,active:frame.agentTurn===true}));
  services.attach(tab.key,{webContents:tab.contents,debugger:debuggerBoundary});trace('native-attached');return;
 }
 if(frame.kind!=='command'||!tab.lease||frame.lease!==tab.lease)return;
 const command=frame.command;if(!command||command.threadId!==tab.key.threadId||(command.type!=='turn'&&command.tabId!==tab.key.tabId))throw Error('private_browser_scope_mismatch');
 await services.command(JSON.stringify(command));
}
function lines(stream,limit,callback,ended=()=>shutdown('browser_closed')){
 let chunks=[],size=0,retained=0,records=0;
 stream.on('data',chunk=>{if(closing)return;let start=0;
  for(let i=0;i<chunk.length;i++)if(chunk[i]===10){const part=chunk.subarray(start,i);
   if(size+part.length+1>limit-retained||records>=4096)return shutdown('browser_input_limit');
   chunks.push(part);const bytes=size+part.length+1,row=Buffer.concat(chunks,bytes-1);chunks=[];size=0;start=i+1;
   let value;try{value=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(row))}catch{return shutdown('browser_input_invalid')}
   retained+=bytes;records++;
   Promise.resolve().then(()=>callback(value)).catch(()=>shutdown('browser_command_failed')).finally(()=>{retained-=bytes;records--});
  }
  if(start<chunk.length){const part=chunk.subarray(start);if(size+part.length+1>limit-retained)return shutdown('browser_input_limit');chunks.push(Buffer.from(part));size+=part.length}
 });
 stream.on('end',ended);stream.on('error',ended);
}
async function open(command){trace('open');
 const key=keyOf(command);let tab=tabs.get(key);if(tab&&!tab.closed)return tab;if(tab?.closed)tabs.delete(key);if(tabs.size>=MAX_TABS)throw Error('browser_tab_limit');
 const partition=await services.getPartition(data.environmentId,data.persistent,data.profileId);
 const nativeSession=await services.getSession(data.environmentId,data.persistent,data.profileId);
 const contextTitle=typeof command.nativeContextTitle==='string'&&/^O3-native-[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(command.nativeContextTitle)?command.nativeContextTitle:null;
 const window=new BrowserWindow({...(contextTitle?{title:contextTitle}:{}),show:false,useContentSize:true,width:data.width,height:data.height,webPreferences:{offscreen:data.ownedNativeHost!==true,backgroundThrottling:false,webviewTag:true,contextIsolation:true,nodeIntegration:false,sandbox:true}});
 const parent=window.webContents;
 if(contextTitle)parent.on('page-title-updated',event=>event.preventDefault());
 parent.on('will-attach-webview',(event,preferences,parameters)=>{if(parameters.partition!==partition||parameters.src!=='about:blank'){event.preventDefault();return}preferences.sandbox=true;preferences.nodeIntegration=false;preferences.nodeIntegrationInSubFrames=false;preferences.contextIsolation=false;preferences.disableHtmlFullscreenWindowResize=true;delete preferences.preload});
 const attached=new Promise(resolve=>parent.once('did-attach-webview',(_event,contents)=>resolve(contents)));
 await window.loadURL('data:text/html,'+encodeURIComponent('<html><body style="margin:0"><webview partition="'+partition+'" src="about:blank" allowpopups="true" webpreferences="contextIsolation=false,sandbox=true,nodeIntegration=false" style="display:flex;width:100vw;height:100vh"></webview></body></html>'));
 const contents=await attached;trace('webview-attached');if(contents.hostWebContents!==parent||contents.session!==nativeSession||contents.getType()!=='webview')throw Error('private_browser_guest_mismatch');
 await contents.loadURL('about:blank');trace('blank-ready');tab={key:{threadId:command.threadId,tabId:command.tabId},window,contents,contextTitle,closed:false,navigation:{_tag:'Idle'},instance:undefined,lease:undefined};tabs.set(key,tab);
 contents.on('will-navigate',(event,url)=>{if(!navigationAllowed(url)){event.preventDefault();ui({type:'external-protocol',...tab.key,url})}});
 contents.on('did-start-loading',()=>status(tab,{_tag:'Loading',url:contents.getURL(),title:contents.getTitle()}));
 contents.on('did-stop-loading',()=>{status(tab,{_tag:'Success',url:contents.getURL(),title:contents.getTitle()});requestCapture()});
 contents.on('did-fail-load',(_event,code,description,url,isMainFrame)=>{if(isMainFrame)status(tab,{_tag:'LoadFailed',url,title:contents.getTitle(),code,description})});
 contents.on('page-title-updated',()=>status(tab));contents.on('context-menu',(_event,params)=>ui({type:'context-menu',...tab.key,x:params.x,y:params.y}));
 contents.setWindowOpenHandler(({url})=>{ui({type:'new-window',...tab.key,url});return {action:'deny'}});
 for(const [source,emitter]of [['window',window],['parent',parent],['guest',contents]])for(const type of ['focus','blur'])emitter.on(type,()=>trace('native-focus-'+source+'-'+type));
 contents.on('did-finish-load',()=>{trace('native-load-finished-window-'+Number(window.isFocused())+'-guest-'+Number(contents.isFocused())+'-focusedFrame-'+Number(!!contents.focusedFrame));if(process.env.O3_NATIVE_ELECTRON_DIAGNOSTICS){const pid=contents.getOSProcessId();if(Number.isInteger(pid)&&pid>0)trace('native-renderer-pid-'+pid);if(process.env.O3_NATIVE_MODULE_METADATA==='1')trace('native-process-metrics-'+JSON.stringify(app.getAppMetrics().map(item=>({pid:item.pid,type:['Browser','GPU','Utility','Tab','Zygote','Sandbox helper'].includes(item.type)?item.type:'other',service:typeof item.serviceName==='string'&&/^[A-Za-z0-9_.]{1,128}$/.test(item.serviceName)?item.serviceName:''}))))}});
 contents.debugger.on('message',(event,method,params,sessionId)=>{if(!tab.guard||!tab.guard.event(method,params,sessionId))return;for(const listener of tab.listeners||[])listener(event,method,params,sessionId)});
 contents.debugger.on('detach',(_event,reason)=>{trace('debugger-detach-'+(['target closed','canceled_by_user','replaced_with_devtools'].includes(reason)?reason.replaceAll(' ','-'):'other'));if(!tab.closed)retire(tab,'private_browser_debugger_detached')});
 contents.on('render-process-gone',(_event,details)=>{const reason=['clean-exit','abnormal-exit','killed','crashed','oom','launch-failed','integrity-failure','memory-eviction'].includes(details?.reason)?details.reason:'other';trace('renderer-gone-'+reason+'-exit-'+(Number.isInteger(details?.exitCode)?details.exitCode:'unknown'));retire(tab,'browser_renderer_failed')});return tab;
}
async function control(command){
 if(closing)return;
 if(command.type==='close')return shutdown('browser_closed');
 if(command.type==='permission-decision'){const pending=dialogs.get(command.requestId);if(pending){dialogs.delete(command.requestId);command.remember===true?pending.resolve({response:command.allow===true?0:1,checkboxChecked:false}):pending.reject(Error('native_dialog_unavailable'))}return}
 if(command.type==='deactivate'){selected='';surfaceId='';for(const tab of tabs.values()){services.setShown(tab.key,false);if(data.ownedNativeHost===true){tab.contents.setBackgroundThrottling(true);tab.window.webContents.setBackgroundThrottling(true);tab.window.hide()}}return}
 if(command.type==='resize'){if(Number.isInteger(command.width)&&Number.isInteger(command.height)&&command.width>=240&&command.height>=240&&command.width<=3840&&command.height<=3840&&command.width*command.height<=3840*2160){data.width=command.width;data.height=command.height;for(const item of tabs.values())if(!item.closed)item.window.setContentSize(command.width,command.height);requestCapture()}return}
 const human=['press','release','move','double','wheel','key','keyup','text','composition'].includes(command.type);
 let tab=tabs.get(human?selected:keyOf(command));
 if(command.type==='open'){
  tab=await open(command);if(command.automation===true)await automate(tab,command.automationInstance);
  if(command.background!==true){selected=keyOf(tab.key);surfaceId=command.surfaceId||'';for(const item of tabs.values())services.setShown(item.key,item===tab);tab.contents.focus();selectNativeHost(tab)}
  if(command.url&&navigationAllowed(command.url))await tab.contents.loadURL(command.url);requestCapture();return;
 }
 if(!tab||tab.closed)return;
 if(human){if(tab.human&&tab.lease){trace('human-'+command.type+'-focus-'+Number(!!tab.contents.focusedFrame)+'-window-'+Number(tab.window.isFocused())+'-guest-'+Number(tab.contents.isFocused())+'-main-'+Number(tab.contents.focusedFrame===tab.contents.mainFrame)+'-capture-'+Number(capturing));if(command.type==='composition')trace('composition-shape-'+typeof command.preedit+'-'+(typeof command.preedit==='string'?command.preedit.length:-1)+'-start-'+typeof command.selectionStart+'-'+(typeof command.selectionStart==='number'?command.selectionStart:0)+'-end-'+typeof command.selectionEnd+'-'+(typeof command.selectionEnd==='number'?command.selectionEnd:0)+'-replace-'+typeof command.replacementStart);const admittedKey=keyOf(tab.key),admittedView=surfaceId,admittedInstance=tab.instance,admittedLease=tab.lease;try{await tab.human.accept(command);trace('human-complete-'+command.type)}catch(error){trace('human-failed-'+command.type+'-'+inputErrorKind(error));if(closing||tab.closed||tab.instance!==admittedInstance||tab.lease!==admittedLease)return;if(error?.message==='Input.imeSetComposition needs the preview to have focus.'||error?.message==="The page's focus is inside a cross-site frame, which takes typed input only while the preview has focus. Click the field first."){if(closing||tab.closed||selected!==admittedKey||surfaceId!==admittedView)return;ui({type:'input-rejected',surfaceId:admittedView,tabKey:admittedKey,inputType:command.type,error:'browser_native_input_focus_required'});return}throw error}requestCapture()}return}
 if(command.type==='automation'){await automate(tab,command.automationInstance);return}
 if(command.type==='activate'){if(command.automation===true)await automate(tab,command.automationInstance);selected=keyOf(tab.key);surfaceId=command.surfaceId||'';for(const item of tabs.values())services.setShown(item.key,item===tab);tab.contents.focus();selectNativeHost(tab)}
 else if(command.type==='close-tab'){await retire(tab,'browser_tab_closed');tabs.delete(keyOf(tab.key));if(selected===keyOf(tab.key))selected='';ui({type:'closed',...tab.key})}
 else if(command.type==='navigate'&&navigationAllowed(command.url))await tab.contents.loadURL(command.url);
 else if(command.type==='refresh'){command.bypassCache?tab.contents.reloadIgnoringCache():tab.contents.reload()}
 else if(command.type==='stop')tab.contents.stop();
 else if(command.type==='back'&&tab.contents.navigationHistory.canGoBack())tab.contents.navigationHistory.goBack();
 else if(command.type==='forward'&&tab.contents.navigationHistory.canGoForward())tab.contents.navigationHistory.goForward();
 else if(command.type==='zoom'&&Number.isFinite(command.value)&&command.value>=.25&&command.value<=5)tab.contents.setZoomFactor(command.value);
 else if(command.type==='resize'&&Number.isInteger(command.width)&&Number.isInteger(command.height)&&command.width>=240&&command.height>=240&&command.width<=3840&&command.height<=3840&&command.width*command.height<=3840*2160){tab.window.setSize(command.width,command.height)}
 else if(command.type==='evaluate'&&typeof command.requestId==='string'&&typeof command.script==='string'&&command.script.length<=32768){ui({type:'evaluation',requestId:command.requestId,value:await tab.contents.executeJavaScript(command.script)??null})}
 requestCapture();
}
app.whenReady().then(async()=>{
 trace('rendering-software-'+Number(process.env.LIBGL_ALWAYS_SOFTWARE==='1')+'-mesa-'+Number(process.env.__EGL_VENDOR_LIBRARY_FILENAMES==='/usr/share/glvnd/egl_vendor.d/50_mesa.json'));
 services=await require('./native-electron-services.cjs').create({
  failed:()=>shutdown('native_browser_service_failed'),
  event:line=>{trace('native-event');const event=JSON.parse(new TextDecoder().decode(line));const tab=tabs.get(keyOf(event));if(tab?.lease)privatePost(tab,'event',{lease:tab.lease,event})},
  painting:()=>requestCapture(),pointer:event=>{const tab=tabs.get(keyOf(event));if(tab)ui({type:'automation-pointer',...tab.key,event})},
  showMessageBox:(options,owner)=>new Promise((resolve,reject)=>{const tab=[...tabs.values()].find(item=>item.window===owner&&item.contents.hostWebContents===owner.webContents);if(!tab||tab.closed||dialogs.size>=64){reject(Error('native_dialog_unavailable'));return}const requestId=crypto.randomUUID();dialogs.set(requestId,{tab,resolve,reject});ui({type:'native-dialog-request',requestId,...tab.key,origin:new URL(tab.contents.getURL()).origin,options})})
 });
 if(typeof services.humanInput!=='function')throw Error('native_browser_assets_incompatible');
 const privateEnded=()=>{trace('native-private-ended');if(!privateAlive)return;privateAlive=false;for(const tab of tabs.values()){tab.guard?.close();services.detach(tab.key)}};let ordered=Promise.resolve();lines(pipe,LIMIT,privateCommand,privateEnded);lines(process.stdin,65536,value=>{if(value.type==='permission-decision'||value.type==='close')return control(value);ordered=ordered.then(()=>control(value));return ordered});if(data.ownedNativeHost===true){scheduleFrame()}ui({type:'ready',frameDirectory:data.frameDirectory});trace('ready');
}).catch(()=>shutdown('browser_start_failed'));
process.on('uncaughtException',()=>shutdown('browser_runtime_failed'));process.on('unhandledRejection',()=>shutdown('browser_runtime_failed'));
