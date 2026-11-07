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
const tabs=new Map(),dialogs=new Map();let services,selected='',surfaceId='',revision=0,closing=false,privateAlive=true;
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
async function capture(){
 if(closing)return;if(capturing){dirty=true;return}const tab=tabs.get(selected);if(!tab||tab.closed)return;
 capturing=true;const identity=selected,view=surfaceId;
 try{if(!tab.guard)return;const screenshot=await tab.guard.sendCommand('Page.captureScreenshot',{format:'png',fromSurface:true,captureBeyondViewport:false});if(closing||selected!==identity||surfaceId!==view)return;const png=Buffer.from(screenshot.data,'base64');if(png.length<24||!png.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])))throw Error('browser_frame_invalid');
  const file='frame-'+(++revision%2)+'.png',target=path.join(data.frameDirectory,file),temporary=target+'.next';
  const fd=fs.openSync(temporary,fs.constants.O_WRONLY|fs.constants.O_CREAT|fs.constants.O_EXCL|fs.constants.O_NOFOLLOW,0o600);
  try{fs.writeFileSync(fd,png)}finally{fs.closeSync(fd)}fs.renameSync(temporary,target);
  const size={width:png.readUInt32BE(16),height:png.readUInt32BE(20)};ui({type:'frame',file,revision,width:size.width,height:size.height,surfaceId:view,tabKey:identity,nativeInput:!!tab.contents.focusedFrame});
 }catch(error){trace('frame-error-'+String(error?.name||'error')+'-'+String(error?.code||'none'));if(!closing&&privateAlive&&!tab.closed&&selected===identity&&surfaceId===view)shutdown('browser_frame_failed')}finally{capturing=false;if(dirty){dirty=false;setImmediate(capture)}}
}
function selectNativeHost(tab){if(data.ownedNativeHost!==true)return;trace('native-host-before-focusedFrame-'+Number(!!tab.contents.focusedFrame));for(const item of tabs.values()){if(item===tab){item.window.show();item.window.focus();item.contents.focus()}else item.window.hide()}trace('native-host-visible-'+Number(tab.window.isVisible())+'-focusedFrame-'+Number(!!tab.contents.focusedFrame))}
function matches(tab,frame){return !tab.closed&&frame.instance===tab.instance&&frame.threadId===tab.key.threadId&&frame.tabId===tab.key.tabId}
async function retire(tab,reason){if(tab.closed)return;if(tab.instance)privatePost(tab,'retired',{...(tab.lease?{lease:tab.lease}:{}),code:reason});tab.closed=true;tab.guard?.close();services?.detach(tab.key);for(const [id,dialog]of dialogs)if(dialog.tab===tab){dialogs.delete(id);dialog.reject(Error('native_dialog_unavailable'));ui({type:'permission-cancelled',requestId:id})}if(!tab.window.isDestroyed())tab.window.destroy()}
async function shutdown(reason){trace(reason);if(closing)return;closing=true;for(const tab of tabs.values())await retire(tab,reason);pipe.destroy();for(const request of dialogs.values())request.reject(Error('native_dialog_unavailable'));dialogs.clear();try{await services?.close()}finally{app.exit(reason==='browser_closed'?0:1)}}
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
 tab.guard=realm.T3QtTargetScope.create(target,(...args)=>tab.contents.debugger.sendCommand(...args),()=>retire(tab,'private_browser_target_mismatch'));
 tab.human=humanInput.create(tab.contents,tab.guard,services.humanInput);privatePost(tab,'proof',{viewTarget:target,cdpTarget:info.targetInfo.targetId});trace('proof');
}
async function privateCommand(frame){
 if(closing||!privateAlive)return;
 const tab=tabs.get(keyOf(frame));if(!tab||!matches(tab,frame))return;
 if(frame.kind==='retire'&&frame.lease===tab.lease){await retire(tab,'private_browser_released');return}
 if(frame.kind==='admitted'){trace('admitted');
  if(!tab.guard||tab.lease!==undefined||typeof frame.lease!=='string'||!frame.lease)throw Error('private_browser_scope_mismatch');
  tab.lease=frame.lease;
  const listeners=new Set();tab.listeners=listeners;
  const debuggerBoundary={sendCommand:(...args)=>tab.guard.sendCommand(...args),on:(name,fn)=>{if(name==='message')listeners.add(fn)},off:(name,fn)=>{if(name==='message')listeners.delete(fn)}};
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
 const window=new BrowserWindow({show:false,useContentSize:true,width:data.width,height:data.height,webPreferences:{offscreen:data.ownedNativeHost!==true,webviewTag:true,contextIsolation:true,nodeIntegration:false,sandbox:true}});
 const parent=window.webContents;
 parent.on('will-attach-webview',(event,preferences,parameters)=>{if(parameters.partition!==partition||parameters.src!=='about:blank'){event.preventDefault();return}preferences.sandbox=true;preferences.nodeIntegration=false;preferences.nodeIntegrationInSubFrames=false;preferences.contextIsolation=false;preferences.disableHtmlFullscreenWindowResize=true;delete preferences.preload});
 const attached=new Promise(resolve=>parent.once('did-attach-webview',(_event,contents)=>resolve(contents)));
 await window.loadURL('data:text/html,'+encodeURIComponent('<html><body style="margin:0"><webview partition="'+partition+'" src="about:blank" allowpopups="true" webpreferences="contextIsolation=false,sandbox=true,nodeIntegration=false" style="display:flex;width:100vw;height:100vh"></webview></body></html>'));
 const contents=await attached;trace('webview-attached');if(contents.hostWebContents!==parent||contents.session!==nativeSession||contents.getType()!=='webview')throw Error('private_browser_guest_mismatch');
 await contents.loadURL('about:blank');trace('blank-ready');tab={key:{threadId:command.threadId,tabId:command.tabId},window,contents,closed:false,navigation:{_tag:'Idle'},instance:undefined,lease:undefined};tabs.set(key,tab);
 contents.on('will-navigate',(event,url)=>{if(!navigationAllowed(url)){event.preventDefault();ui({type:'external-protocol',...tab.key,url})}});
 contents.on('did-start-loading',()=>status(tab,{_tag:'Loading',url:contents.getURL(),title:contents.getTitle()}));
 contents.on('did-stop-loading',()=>{status(tab,{_tag:'Success',url:contents.getURL(),title:contents.getTitle()});capture()});
 contents.on('did-fail-load',(_event,code,description,url,isMainFrame)=>{if(isMainFrame)status(tab,{_tag:'LoadFailed',url,title:contents.getTitle(),code,description})});
 contents.on('page-title-updated',()=>status(tab));contents.on('context-menu',(_event,params)=>ui({type:'context-menu',...tab.key,x:params.x,y:params.y}));
 contents.setWindowOpenHandler(({url})=>{ui({type:'new-window',...tab.key,url});return {action:'deny'}});
 contents.debugger.on('message',(event,method,params,sessionId)=>{if(!tab.guard||!tab.guard.event(method,params,sessionId))return;for(const listener of tab.listeners||[])listener(event,method,params,sessionId)});
 contents.debugger.on('detach',()=>{if(!tab.closed)retire(tab,'private_browser_debugger_detached')});
 contents.on('render-process-gone',()=>retire(tab,'browser_renderer_failed'));return tab;
}
async function control(command){
 if(closing)return;
 if(command.type==='close')return shutdown('browser_closed');
 if(command.type==='permission-decision'){const pending=dialogs.get(command.requestId);if(pending){dialogs.delete(command.requestId);command.remember===true?pending.resolve({response:command.allow===true?0:1,checkboxChecked:false}):pending.reject(Error('native_dialog_unavailable'))}return}
 if(command.type==='deactivate'){selected='';surfaceId='';for(const tab of tabs.values()){services.setShown(tab.key,false);if(data.ownedNativeHost===true)tab.window.hide()}return}
 if(command.type==='resize'){if(Number.isInteger(command.width)&&Number.isInteger(command.height)&&command.width>=240&&command.height>=240&&command.width<=3840&&command.height<=3840&&command.width*command.height<=3840*2160){data.width=command.width;data.height=command.height;for(const item of tabs.values())if(!item.closed)item.window.setContentSize(command.width,command.height);capture()}return}
 const human=['press','release','move','double','wheel','key','keyup','text','composition'].includes(command.type);
 let tab=tabs.get(human?selected:keyOf(command));
 if(command.type==='open'){
  tab=await open(command);if(command.automation===true)await automate(tab,command.automationInstance);
  if(command.background!==true){selected=keyOf(tab.key);surfaceId=command.surfaceId||'';for(const item of tabs.values())services.setShown(item.key,item===tab);tab.contents.focus();selectNativeHost(tab)}
  if(command.url&&navigationAllowed(command.url))await tab.contents.loadURL(command.url);capture();return;
 }
 if(!tab||tab.closed)return;
 if(human){if(tab.human&&tab.lease){const admittedKey=keyOf(tab.key),admittedView=surfaceId;try{await tab.human.accept(command)}catch(error){if(error?.message==='Input.imeSetComposition needs the preview to have focus.'||error?.message==="The page's focus is inside a cross-site frame, which takes typed input only while the preview has focus. Click the field first."){if(closing||tab.closed||selected!==admittedKey||surfaceId!==admittedView)return;ui({type:'input-rejected',surfaceId:admittedView,tabKey:admittedKey,inputType:command.type,error:'browser_native_input_focus_required'});return}throw error}capture()}return}
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
 capture();
}
app.whenReady().then(async()=>{
 services=await require('./native-electron-services.cjs').create({
  failed:()=>shutdown('native_browser_service_failed'),
  event:line=>{trace('native-event');const event=JSON.parse(new TextDecoder().decode(line));const tab=tabs.get(keyOf(event));if(tab?.lease)privatePost(tab,'event',{lease:tab.lease,event})},
  painting:()=>capture(),pointer:event=>{const tab=tabs.get(keyOf(event));if(tab)ui({type:'automation-pointer',...tab.key,event})},
  showMessageBox:(options,owner)=>new Promise((resolve,reject)=>{const tab=[...tabs.values()].find(item=>item.window===owner&&item.contents.hostWebContents===owner.webContents);if(!tab||tab.closed||dialogs.size>=64){reject(Error('native_dialog_unavailable'));return}const requestId=crypto.randomUUID();dialogs.set(requestId,{tab,resolve,reject});ui({type:'native-dialog-request',requestId,...tab.key,origin:new URL(tab.contents.getURL()).origin,options})})
 });
 if(typeof services.humanInput!=='function')throw Error('native_browser_assets_incompatible');
 const privateEnded=()=>{if(!privateAlive)return;privateAlive=false;for(const tab of tabs.values()){tab.guard?.close();services.detach(tab.key)}};let ordered=Promise.resolve();lines(pipe,LIMIT,privateCommand,privateEnded);lines(process.stdin,65536,value=>{if(value.type==='permission-decision'||value.type==='close')return control(value);ordered=ordered.then(()=>control(value));return ordered});ui({type:'ready',frameDirectory:data.frameDirectory});trace('ready');
}).catch(()=>shutdown('browser_start_failed'));
process.on('uncaughtException',()=>shutdown('browser_runtime_failed'));process.on('unhandledRejection',()=>shutdown('browser_runtime_failed'));
