// Private capture process. No audio is written to stdout, files or network.
'use strict';
const fs=require('node:fs'),path=require('node:path'),net=require('node:net'),http=require('node:http'),crypto=require('node:crypto');
const data=JSON.parse(process.env.O3_NATIVE_VOICE_DATA||'null');delete process.env.O3_NATIVE_VOICE_DATA;
if(!data||!Number.isInteger(data.fd)||data.fd<64||!Number.isInteger(data.peerPid)||data.peerPid<1||typeof data.captureId!=='string'||!/^[a-f0-9]{48}$/.test(data.captureId)||!path.isAbsolute(data.directory))throw Error('Invalid private capture launch');
const directory=fs.lstatSync(data.directory);
if(!directory.isDirectory()||directory.isSymbolicLink()||directory.uid!==process.getuid()||(directory.mode&0o077)!==0||fs.realpathSync(data.directory)!==data.directory)throw Error('Invalid private capture directory');
require('./electron-private-fd.node').admit(data.fd,data.peerPid);
const {app,BrowserWindow,ipcMain,session}=require('electron');
if(process.versions.electron!=='44.10.1')throw Error('Unsupported capture runtime');
app.setPath('userData',path.join(data.directory,'profile'));app.setPath('cache',path.join(data.directory,'cache'));
const nonce=crypto.randomBytes(24).toString('hex'),MaxPcm=19200000,MaxControl=65536,MaxQueued=MaxPcm+MaxControl*8;
const pipe=new net.Socket({fd:data.fd,readable:true,writable:true});
const resource=fs.readFileSync(path.join(__dirname,'NativePcmCapture.js'));
if(crypto.createHash('sha256').update(resource).digest('hex')!=='82e67fc3e59f40b522492cd85ed340254ebb96fc2a664d796766be957f1f9c83')throw Error('Changed capture resource');
const files={
 'index.html':Buffer.from('<!doctype html><meta charset="utf-8"><script type="module" src="renderer.mjs"></script>'),
 'compiled.mjs':Buffer.from(resource.toString().replace(/^\.pragma library\n/,'')+'\nexport {createDsp,createCapture};\n'),
 'worklet.mjs':Buffer.from("import {createDsp} from './compiled.mjs';createDsp(sampleRate,{AudioWorkletProcessor,registerProcessor,math:Math});"),
 'renderer.mjs':fs.readFileSync(path.join(__dirname,'electron-capture-renderer.mjs')),
 'browser-capture-owner.mjs':fs.readFileSync(path.join(__dirname,'browser-capture-owner.mjs')),
};
let window,server,origin,ready=false,closing=false,terminal=false,sequence=0,totalPcm=0,bufferDelivered=false,permissionAllowed=false,captureMode,recording=false,ordered=Promise.resolve(),queued=0;
const pending=new Map(),permissions=new Map();
function fatal(){if(terminal)return;terminal=true;closing=true;for(const decision of permissions.values())decision(false);permissions.clear();if(window&&!window.isDestroyed())window.destroy();if(server)server.close();pipe.destroy();app.exit(2)}
function packet(kind,body){
 if(terminal||pipe.destroyed||body.length>(kind===0?MaxControl:32000)||pipe.writableLength>MaxQueued-body.length-12)return fatal();
 const header=Buffer.alloc(12);header.write('OVC1');header.writeUInt32LE(kind,4);header.writeUInt32LE(body.length,8);
 // No await separates the header and body, so frames cannot interleave.
 pipe.cork();pipe.write(header);pipe.write(body);pipe.uncork();
}
function control(value){packet(0,Buffer.from(JSON.stringify({...value,captureId:data.captureId})))}
function own(event,identity){return !terminal&&identity===nonce&&window&&!window.isDestroyed()&&event.sender===window.webContents&&event.senderFrame===window.webContents.mainFrame&&event.senderFrame.url===origin+'/'+nonce+'/index.html'}
function invoke(command){
 if(!ready||terminal)return Promise.reject(Error('Capture unavailable'));
 return new Promise((resolve,reject)=>{pending.set(command.id,{resolve,reject});window.webContents.send('o3-voice-command',nonce,command)});
}
function denyPermissions(){permissionAllowed=false;for(const decision of permissions.values())decision(false);permissions.clear()}
async function close(command){
 if(closing)throw Error('Capture closing');closing=true;denyPermissions();
 const result=await invoke(command);
 if(result.ok!==true||result.value?.cleanup!=='confirmed')return fatal();
 control({type:'result',id:command.id,...result});
 terminal=true;if(server)server.close();pipe.end(()=>{if(window&&!window.isDestroyed())window.destroy();app.exit(0)});
}
function input(value){
 if(!value||Array.isArray(value)||value.captureId!==data.captureId||!Number.isSafeInteger(value.id)||value.id<=sequence||typeof value.type!=='string')return fatal();
 sequence=value.id;
 if(value.type==='permission'){
  if(Object.keys(value).some(key=>!['type','id','captureId','requestId','allow'].includes(key))||typeof value.requestId!=='string'||typeof value.allow!=='boolean')return fatal();
  const decision=permissions.get(value.requestId);if(!decision)return control({type:'result',id:value.id,ok:false,value:{error:'stale_permission'}});
  permissions.delete(value.requestId);permissionAllowed=!closing&&value.allow;decision(permissionAllowed);return control({type:'result',id:value.id,ok:true,value:null});
 }
 if(value.type==='close'){
  if(Object.keys(value).some(key=>!['type','id','captureId'].includes(key)))return fatal();
  void close(value).catch(fatal);return;
 }
 if(closing||!ready||queued>=8)return fatal();
 if(value.type==='prepare'){
  if(Object.keys(value).some(key=>!['type','id','captureId','mode','microphoneId'].includes(key))||!['streaming','buffered'].includes(value.mode)||typeof value.microphoneId!=='string')return fatal();
 }else if(!['record','stop'].includes(value.type)||Object.keys(value).some(key=>!['type','id','captureId'].includes(key)))return fatal();
 queued++;ordered=ordered.then(async()=>{if(closing)return;if(value.type==='prepare')captureMode=value.mode;if(value.type==='record')recording=true;const result=await invoke(value);if(value.type==='stop'||result.ok!==true)recording=false;if(!closing)control({type:'result',id:value.id,...result})}).catch(fatal).finally(()=>queued--);
}
let chunks=[],received=0,expected;
pipe.on('data',chunk=>{
 if(terminal)return;if(received+chunk.length>MaxControl*8)return fatal();chunks.push(chunk);received+=chunk.length;
 let bytes=Buffer.concat(chunks,received);chunks=[];
 while(bytes.length){
  if(expected===undefined){if(bytes.length<12)break;if(bytes.toString('ascii',0,4)!=='OVC1'||bytes.readUInt32LE(4)!==0)return fatal();expected=bytes.readUInt32LE(8);bytes=bytes.subarray(12);if(expected<2||expected>MaxControl)return fatal()}
  if(bytes.length<expected)break;const body=bytes.subarray(0,expected);bytes=bytes.subarray(expected);expected=undefined;
  try{input(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(body)))}catch(_){return fatal()}if(terminal)return;
 }
 chunks=[Buffer.from(bytes)];received=bytes.length;
});
pipe.on('end',fatal);pipe.on('error',fatal);
ipcMain.on('o3-voice-renderer-ready',(event,identity)=>{if(!own(event,identity)||ready)return fatal();ready=true;control({type:'ready',runtime:process.versions.electron})});
ipcMain.on('o3-voice-result',(event,identity,id,ok,value)=>{
 if(!own(event,identity)||typeof ok!=='boolean'||!pending.has(id)||Buffer.byteLength(JSON.stringify(value??null))>1024)return fatal();
 const completion=pending.get(id);pending.delete(id);completion.resolve({ok,value});
});
ipcMain.on('o3-voice-pcm',(event,identity,kind,bytes)=>{
 if(!own(event,identity)||!(bytes instanceof ArrayBuffer)||!['stream','buffer'].includes(kind)||!bytes.byteLength||bytes.byteLength%4||bytes.byteLength>MaxPcm-totalPcm||(kind==='stream'&&bytes.byteLength>32000)||(kind==='buffer'&&bufferDelivered))return fatal();
 if(closing)return;
 if(!recording||(kind==='stream'?captureMode!=='streaming':captureMode!=='buffered'))return fatal();
 if(kind==='buffer')bufferDelivered=true;totalPcm+=bytes.byteLength;const buffer=Buffer.from(bytes);
 for(let at=0;at<buffer.length;at+=32000)packet(kind==='stream'?1:2,buffer.subarray(at,at+32000));
});
ipcMain.on('o3-voice-level',(event,identity,value)=>{if(!own(event,identity)||!Number.isFinite(value)||value<0||value>1)return fatal();if(!closing)control({type:'level',value})});
ipcMain.on('o3-voice-limit',(event,identity)=>{if(!own(event,identity))return fatal();if(!closing)control({type:'duration-limit'})});
ipcMain.on('o3-voice-failed',(event,identity)=>{if(!own(event,identity))return fatal();if(!closing)control({type:'capture-error',error:'capture_failed'})});
app.whenReady().then(async()=>{
 server=http.createServer((request,response)=>{
  const prefix='/'+nonce+'/';if(request.method!=='GET'||!request.url.startsWith(prefix)){response.writeHead(404);response.end();return}
  const name=request.url.slice(prefix.length),body=Object.hasOwn(files,name)?files[name]:undefined;if(!body){response.writeHead(404);response.end();return}
  response.writeHead(200,{'Content-Type':name.endsWith('.html')?'text/html; charset=utf-8':'application/javascript; charset=utf-8','Content-Length':body.length,'Content-Security-Policy':"default-src 'none'; script-src 'self'; connect-src 'self' blob:; worker-src 'self'; media-src blob:"});response.end(body);
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));origin='http://127.0.0.1:'+server.address().port;
 const isolated=session.fromPartition('voice-'+nonce,{cache:false});
 const ownedContents=contents=>window&&!window.isDestroyed()&&contents===window.webContents&&contents.mainFrame.url===origin+'/'+nonce+'/index.html';
 isolated.setPermissionCheckHandler((contents,permission,requestingOrigin)=>!closing&&permissionAllowed&&ownedContents(contents)&&permission==='media'&&requestingOrigin===origin);
 isolated.setPermissionRequestHandler((contents,permission,callback,details)=>{
  if(closing||!ownedContents(contents)||permission!=='media'||details.isMainFrame!==true||details.requestingUrl!==origin+'/'+nonce+'/index.html'||!Array.isArray(details.mediaTypes)||details.mediaTypes.length!==1||details.mediaTypes[0]!=='audio'||permissions.size>=1){callback(false);return}
  const requestId=crypto.randomBytes(24).toString('hex');permissions.set(requestId,callback);control({type:'permission-request',requestId,media:'audio'});
 });
 isolated.webRequest.onBeforeRequest((details,callback)=>callback({cancel:!details.url.startsWith(origin+'/'+nonce+'/')&&!details.url.startsWith('blob:'+origin+'/')}));
 window=new BrowserWindow({show:false,width:64,height:64,webPreferences:{session:isolated,preload:path.join(__dirname,'electron-capture-preload.cjs'),additionalArguments:['--voice-nonce='+nonce],sandbox:true,nodeIntegration:false,contextIsolation:true,spellcheck:false,backgroundThrottling:false}});
 window.webContents.setAudioMuted(true);window.webContents.setWindowOpenHandler(()=>({action:'deny'}));window.webContents.on('will-attach-webview',event=>event.preventDefault());window.webContents.on('will-navigate',event=>event.preventDefault());window.webContents.on('render-process-gone',fatal);
 await window.loadURL(origin+'/'+nonce+'/index.html');
}).catch(fatal);
process.on('uncaughtException',fatal);process.on('unhandledRejection',fatal);
