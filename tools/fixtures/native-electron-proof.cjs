// Synthetic native Electron acceptance; run only through the private runner.
'use strict';
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const http=require('node:http');
const readline=require('node:readline');
const {spawn}=require('node:child_process');
const privateFd=Number(process.env.O3_TEST_PRIVATE_FD);assert(Number.isInteger(privateFd)&&privateFd>=64);
assert.equal(require('./electron-private-fd.node').admit(privateFd,Number(process.env.O3_TEST_PEER_PID)),true);
const privateSocket=new(require('node:net').Socket)({fd:privateFd,readable:true,writable:true});
const descriptorFlags=/^flags:\s+([0-7]+)$/m.exec(fs.readFileSync('/proc/self/fdinfo/'+privateFd,'utf8'));assert(descriptorFlags&&(parseInt(descriptorFlags[1],8)&0o2000000),'actual private descriptor is close-on-exec before guests');
const {app,BrowserWindow,clipboard,webContents,ipcMain,session:electronSessions}=require('electron');
assert.equal(process.versions.electron,'44.10.1');
app.setPath('userData','/tmp/electron-userdata');
app.setPath('crashDumps','/tmp/electron-crashes');
app.disableHardwareAcceleration();
const observedPermissions=[];
const originalFromPartition=electronSessions.fromPartition.bind(electronSessions);
const wrappedSessions=new WeakSet();
electronSessions.fromPartition=(...args)=>{const value=originalFromPartition(...args);if(!wrappedSessions.has(value)){wrappedSessions.add(value);const check=value.setPermissionCheckHandler.bind(value),request=value.setPermissionRequestHandler.bind(value);value.setPermissionCheckHandler=handler=>check((...args)=>{const result=handler(...args);observedPermissions.push({kind:'check',permission:args[1],result});return result});value.setPermissionRequestHandler=handler=>request((contents,permission,callback,details)=>handler(contents,permission,result=>{observedPermissions.push({kind:'request',permission,result});callback(result)},details));}return value;};
const delays=[];
const windows=[];
let services,producer,server,sequence=0;
const pending=new Map(),producerPending=new Map(),sandbox=new Map(),dialogs=[],attachments=new Map();
let choices=[1,1,0,1];
function deadline(promise,label){let timer;return Promise.race([promise,new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('Private fixture deadline: '+label)),12000)})]).finally(()=>clearTimeout(timer))}
function event(line){const row=JSON.parse(new TextDecoder().decode(line));if(row.type==='attached'){attachments.get(row.tabId)?.();attachments.delete(row.tabId)}if(row.type!=='cdp')return;const frame=JSON.parse(row.message);if(typeof frame.id!=='number')return;const wait=pending.get(frame.id);if(!wait)return;pending.delete(frame.id);frame.error?wait.reject(Error(frame.error.message)):wait.resolve(frame.result)}
async function command(tab,method,params={},sessionId){const id=++sequence;let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b});pending.set(id,{resolve,reject});await services.command(JSON.stringify({type:'cdp',...tab.key,message:JSON.stringify({id,method,params,...(sessionId===undefined?{}:{sessionId})})}));return deadline(promise,method)}
async function evaluate(tab,expression){const result=await command(tab,'Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true},tab.session);if(result.exceptionDetails)throw Error('Fixture evaluation failed: '+(result.exceptionDetails.exception?.description||result.exceptionDetails.text));return result.result.value}
async function producerCommand(row){const id=++sequence;let resolve;const promise=new Promise(r=>resolve=r);producerPending.set(id,resolve);producer.stdin.write(JSON.stringify({...row,id})+'\n');return deadline(promise,'selection owner')}
async function navigate(tab,url){await command(tab,'Page.navigate',{url},tab.session);for(let count=0;count<300;count++){if(await evaluate(tab,'location.href === '+JSON.stringify(url)+' && document.readyState === \"complete\"'))return;await new Promise(r=>setTimeout(r,20))}throw Error('Actual navigation did not finish')}
async function tab(scope,host){const browserSession=await services.getSession(scope,false,'profile');const partition=await services.getPartition(scope,false,'profile');
 const window=new BrowserWindow({show:false,width:640,height:480,webPreferences:{offscreen:true,webviewTag:true,contextIsolation:true,nodeIntegration:false,sandbox:true}});windows.push(window);
 window.webContents.on('will-attach-webview',(event,preferences,params)=>{assert.equal(params.partition,partition);preferences.sandbox=true;preferences.nodeIntegration=false;preferences.nodeIntegrationInSubFrames=false;preferences.contextIsolation=false;preferences.disableHtmlFullscreenWindowResize=true;preferences.preload=path.join(__dirname,'electron-sandbox-preload.cjs')});
 const attachedGuest=new Promise(resolve=>window.webContents.once('did-attach-webview',(_event,guest)=>resolve(guest)));
 await window.loadURL('data:text/html,'+encodeURIComponent('<html><body style="margin:0"><webview partition="'+partition+'" src="about:blank" allowpopups="true" webpreferences="contextIsolation=false,sandbox=true,nodeIntegration=false" style="display:flex;width:640px;height:480px"></webview></body></html>'));
 const contents=await deadline(attachedGuest,'actual native webview attachment');assert.equal(contents.getType(),'webview');assert.equal(contents.hostWebContents,window.webContents);assert.equal(contents.session,browserSession,'actual guest uses the native service partition');await contents.loadURL('about:blank');contents.debugger.attach('1.3');
 const info=await contents.debugger.sendCommand('Target.getTargetInfo');assert.equal(webContents.fromDevToolsTargetId(info.targetInfo.targetId),contents,'actual debugger belongs to the exact created contents');
 const key={threadId:'owned-thread',tabId:'owned-'+scope};window.__ownedProofKey=key;const attached=new Promise(resolve=>attachments.set(key.tabId,resolve));services.attach(key,{webContents:contents,debugger:contents.debugger});services.setShown(key,true);await deadline(attached,'native attached-event barrier');
 const value={window,contents,key};const targets=await command(value,'Target.getTargets');assert.equal(targets.targetInfos.length,1);assert.equal(targets.targetInfos[0].targetId,info.targetInfo.targetId);
 value.session=(await command(value,'Target.attachToTarget',{targetId:info.targetInfo.targetId,flatten:true})).sessionId;
 await navigate(value,'http://'+host+':'+server.address().port+'/');
 const isolation=await evaluate(value,'[typeof require,typeof process,typeof Buffer,typeof T3NativeDesktopHost]');assert.deepEqual(isolation,['undefined','undefined','undefined','undefined']);
 const proof=sandbox.get(contents.id);assert(proof?.sandboxed&&proof.isolated===false&&proof.electron==='44.10.1','real sandboxed native-preference webview preload');
 return value;
}
ipcMain.on('owned-sandbox-proof',(event,value)=>sandbox.set(event.sender.id,value));
app.whenReady().then(async()=>{
 producer=spawn('/proof/clipboard-owner',[],{stdio:['pipe','pipe','pipe'],env:{...process.env,QT_QPA_PLATFORM:'xcb'}});
 let producerReady;const ready=new Promise(resolve=>producerReady=resolve);
 readline.createInterface({input:producer.stdout}).on('line',line=>{const row=JSON.parse(line);if(row.id!==undefined){producerPending.get(row.id)?.(row);producerPending.delete(row.id)}else if(row.ready)producerReady()});
 producer.stderr.on('data',()=>{});await deadline(ready,'independent clipboard owner start');
 server=http.createServer((_request,response)=>{response.end('<html><title>Owned Electron fixture</title></html>')});await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const {create}=require('./native-electron-services.cjs');
 services=await create({event,showMessageBox:async(options,owner)=>{dialogs.push({options,owner:owner?.webContents.id});assert.equal(options.defaultId,1);assert.equal(options.cancelId,1);const response=choices.shift();assert.notEqual(response,undefined,'No unsolicited dialog');return {response,checkboxChecked:false}}});
 const writeCheck=await tab('writecheck','127.0.0.1');
 const denied="navigator.clipboard.readText().then(()=>false,()=>true)";
 const nativeWrite=await evaluate(writeCheck,"navigator.clipboard.writeText('owned asynchronous write').then(()=>({allowed:true}),error=>({allowed:false,name:error.name,message:error.message}))");
 assert.equal(nativeWrite.allowed,false,'actual upstream async-write context is denied without read consent');assert.equal(dialogs.length,1);
 const copied=await evaluate(writeCheck,"(()=>{const input=document.createElement('textarea');input.value='owned sanitized gesture copy';document.body.appendChild(input);input.focus();input.select();return document.execCommand('copy')})()");
 assert.equal(copied,false,'native DevTools copy context is not silently promoted to a human gesture');
 writeCheck.contents.focus();writeCheck.contents.copy();
 let copiedText;for(let count=0;count<100;count++){copiedText=(await producerCommand({op:'read'})).text;if(copiedText==='owned sanitized gesture copy')break;await new Promise(resolve=>setTimeout(resolve,30))}
 assert.equal(copiedText,'owned sanitized gesture copy','separate application receives actual explicit-human-copy clipboard bytes');assert.equal(dialogs.length,1,'explicit user copy adds no read prompt');
 const one=await tab('one','127.0.0.1');
 await producerCommand({op:'text',text:'owned external clipboard one'});
 assert.equal(await evaluate(one,denied),true);assert.equal(dialogs.length,2);
 assert.equal(await evaluate(one,denied),true);assert.equal(dialogs.length,2,'remembered Block');
 await navigate(one,'http://localhost:'+server.address().port+'/');
 assert.equal(await evaluate(one,'navigator.clipboard.readText()'),'owned external clipboard one');assert.equal(dialogs.length,3);
 await producerCommand({op:'text',text:'owned external clipboard changed'});
 assert.equal(await evaluate(one,'navigator.clipboard.readText()'),'owned external clipboard changed');assert.equal(dialogs.length,3,'remembered Allow still reads fresh external bytes');
 await producerCommand({op:'image'});
 const image=await evaluate(one,"navigator.clipboard.read().then(async items=>{const item=items.find(item=>item.types.includes('image/png'));if(!item)throw Error('No PNG');const blob=await item.getType('image/png');const image=await createImageBitmap(blob);const canvas=document.createElement('canvas');canvas.width=image.width;canvas.height=image.height;const context=canvas.getContext('2d');context.drawImage(image,0,0);return {width:image.width,height:image.height,pixel:Array.from(context.getImageData(0,0,1,1).data)}})");assert.deepEqual(image,{width:2,height:2,pixel:[18,52,86,255]});
 const two=await tab('two','localhost');assert.equal(await evaluate(two,denied),true);assert.equal(dialogs.length,4,'native decisions stay in the actual session');
 const foreign=await two.contents.debugger.sendCommand('Target.getTargetInfo');let blocked=false;try{await command(one,'Target.attachToTarget',{targetId:foreign.targetInfo.targetId,flatten:true})}catch{blocked=true}assert(blocked,'native root relay rejects a foreign target');
 const ownSocket=process.env.O3_TEST_PRIVATE_INODE;const descendants=new Set([process.pid]);let grew=true;
 while(grew){grew=false;for(const pid of fs.readdirSync('/proc').filter(name=>/^\d+$/.test(name))){try{const stat=fs.readFileSync('/proc/'+pid+'/stat','utf8');const parent=Number(stat.slice(stat.lastIndexOf(')')+2).split(' ')[1]);if(descendants.has(parent)&&!descendants.has(Number(pid))){descendants.add(Number(pid));grew=true}}catch{}}}
 assert(descendants.has(one.contents.getOSProcessId()),'actual guest renderer belongs to the owned process tree');
 for(const name of fs.readdirSync('/proc').filter(name=>/^\d+$/.test(name))){const pid=Number(name);if(pid===process.pid||pid===Number(process.env.O3_TEST_PEER_PID))continue;let descriptors;try{descriptors=fs.readdirSync('/proc/'+pid+'/fd')}catch{continue}for(const fd of descriptors){let link;try{link=fs.readlinkSync('/proc/'+pid+'/fd/'+fd)}catch{continue}assert.notEqual(link,ownSocket,'private socket must not leak into any other private-namespace process, including reparented crash helpers')}}
 const scopeRealm={};require('node:vm').runInNewContext(fs.readFileSync(path.join(__dirname,'target-scope.js'),'utf8'),scopeRealm);
 const ownInfo=await one.contents.debugger.sendCommand('Target.getTargetInfo');
 let retired=false;const guard=scopeRealm.T3QtTargetScope.create(ownInfo.targetInfo.targetId,(...args)=>one.contents.debugger.sendCommand(...args),()=>retired=true);
 let rawForeignVisible=false;try{const raw=await one.contents.debugger.sendCommand('Target.getTargetInfo',{targetId:foreign.targetInfo.targetId});rawForeignVisible=raw.targetInfo.targetId===foreign.targetInfo.targetId}catch{}
 const forbidden=[['Target.getTargetInfo',{targetId:foreign.targetInfo.targetId}],['Target.attachToTarget',{targetId:foreign.targetInfo.targetId,flatten:true}],['Target.closeTarget',{targetId:foreign.targetInfo.targetId}],['Target.getTargets',{}],['Target.createTarget',{url:'about:blank'}],['Target.createBrowserContext',{}],['Target.setDiscoverTargets',{discover:true}],['Browser.getVersion',{}]];
 for(const [method,params]of forbidden)await assert.rejects(guard.sendCommand(method,params),/Only this preview tab/);
 await assert.rejects(guard.sendCommand('Runtime.evaluate',{expression:'1'},'foreign-session'),/Only this preview tab/);
 const opaque=JSON.stringify({targetId:foreign.targetInfo.targetId,sessionId:'foreign-session',method:'Target.closeTarget'});
 const opaqueResult=await guard.sendCommand('Runtime.evaluate',{expression:JSON.stringify(opaque),returnByValue:true});assert.equal(opaqueResult.result.value,opaque,'opaque page strings stay unchanged');
 const guardedInfo=await guard.sendCommand('Target.getTargetInfo');assert.equal(guardedInfo.targetInfo.targetId,ownInfo.targetInfo.targetId);assert.equal(retired,false);guard.close();await assert.rejects(guard.sendCommand('Runtime.evaluate',{expression:'1'}),/Only this preview tab/);
 console.log('NATIVE_ELECTRON_PROOF '+JSON.stringify({electron:process.versions.electron,sandboxed:true,nativeContextIsolation:false,nodeGlobalsAbsent:true,dialogs:dialogs.map(row=>({message:row.options.message,buttons:row.options.buttons,defaultId:row.options.defaultId,cancelId:row.options.cancelId})),nativeAsyncWrite:nativeWrite,nativeDevtoolsCopy:copied,sanitizedGestureCopy:true,textRoundtrip:true,freshExternalText:true,externalPng:image,sessionIsolation:true,foreignRootAttachDenied:true,rawForeignTargetVisible:rawForeignVisible,physicalTargetScopeDenied:forbidden.length+1,privateFdCloexec:true,descendantSocketIsolation:true,privateNamespaceSocketIsolation:true,opaqueRuntimePreserved:true}));
 for(const window of windows){services.detach(window.__ownedProofKey);window.destroy()}
 await services.close();producer.stdin.end(JSON.stringify({op:'quit'})+'\n');server.close();app.exit(0);
}).catch(error=>{console.error('NATIVE_ELECTRON_FIXTURE_FAILED',error?.stack||String(error),'PERMISSIONS',JSON.stringify(observedPermissions));for(const window of windows)if(!window.isDestroyed())window.destroy();producer?.kill();server?.close();app.exit(1)});
setTimeout(()=>{console.error('NATIVE_ELECTRON_FIXTURE_TIMEOUT');producer?.kill();app.exit(2)},35000).unref();
