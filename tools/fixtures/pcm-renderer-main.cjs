// Whole native DSP in the pinned sandboxed renderer's actual V8 implementation.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),crypto=require('node:crypto');
const {stripTypeScriptTypes}=require('node:module');
const {app,BrowserWindow,ipcMain,session}=require('electron');
if(!fs.existsSync('/run/.containerenv')||process.versions.electron!=='44.10.1')throw Error('Private pinned Electron required');
const root='/source',nonce=crypto.randomBytes(24).toString('hex'),hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const expected={
 'pcmResampler.ts':'933981998febcd721175c8e83b56570fb7cb42a0bc6d2a967f244ce1cc0881f5',
 'pcmWorklet.ts':'cd93d59fc68414be05db6bae6808f20877245c79cec9253558e34040381e1983',
};
const modules={};for(const[name,digest]of Object.entries(expected)){
 const bytes=fs.readFileSync(path.join(root,'native',name));if(hash(bytes)!==digest)throw Error('Changed whole native DSP module');
 modules[name]=stripTypeScriptTypes(bytes.toString(),{mode:'strip'});
}
const originalImport='import { PcmResampler } from "./pcmResampler";';
if(modules['pcmWorklet.ts'].split(originalImport).length!==2)throw Error('Changed worklet module linkage');
const factory=`import {PcmResampler} from './resampler.mjs';
export function create(sampleRate,port){
 let Processor;
 class AudioWorkletProcessor{constructor(){this.port=port}}
 const registerProcessor=(name,value)=>{if(name!=='t3-pcm-capture')throw Error('Unexpected processor');Processor=value};
 ${modules['pcmWorklet.ts'].replace(originalImport,'')}
 return {PcmResampler,Processor,math:Math};
}`;
const compiled=fs.readFileSync(path.join(root,'NativePcmCapture.js'),'utf8').replace(/^\.pragma library\n/,'');
const compiledFactory=`import {createDsp} from './compiled.mjs';
export function create(sampleRate,port){
 let Processor;
 class AudioWorkletProcessor{constructor(){this.port=port}}
 const registerProcessor=(name,value)=>{if(name!=='t3-pcm-capture')throw Error('Unexpected processor');Processor=value};
 const api=createDsp(sampleRate,{AudioWorkletProcessor,registerProcessor,math:Math});
 return {PcmResampler:api.PcmResampler,Processor,math:Math};
}`;
const files={
 'index.html':Buffer.from('<!doctype html><meta charset="utf-8"><script src="driver.js"></script><script type="module" src="run.mjs"></script>'),
 'resampler.mjs':Buffer.from(modules['pcmResampler.ts']),
 'factory.mjs':Buffer.from(factory),
 'compiled.mjs':Buffer.from(compiled+'\nexport {createDsp};\n'),
 'compiled-factory.mjs':Buffer.from(compiledFactory),
 'driver.js':fs.readFileSync(path.join(root,'pcm-capture-driver.js')),
 'run.mjs':Buffer.from(`import {create} from './factory.mjs';import {create as createCompiled} from './compiled-factory.mjs';
  const digest=scope=>({append:(key,bytes)=>window.pcmProbe.append(scope,key,bytes),finish:key=>window.pcmProbe.finish(scope,key)});
  try{const result=await runPcmOracle(create,digest('original'));const compiled=await runPcmOracle(createCompiled,digest('compiled'));window.pcmProbe.complete({userAgent:navigator.userAgent,result,compiled})}
  catch(error){window.pcmProbe.failed(String(error&&error.message||error))}`),
};
app.setPath('userData','/state/profile');app.setPath('cache','/state/cache');
let window,server,finished=false,origin;
const digests=new Map(),lengths=new Map(),totals=new Map();
function finish(code,record){
 if(finished)return;finished=true;
 if(record)fs.writeFileSync('/state/renderer.json',JSON.stringify(record)+'\n',{flag:'wx',mode:0o600});
 if(window&&!window.isDestroyed())window.destroy();
 if(server)server.close();app.exit(code);
}
function own(event,identity){return !finished&&identity===nonce&&window&&!window.isDestroyed()&&event.sender===window.webContents&&event.senderFrame===window.webContents.mainFrame&&event.senderFrame.url===origin+'/'+nonce+'/index.html'}
ipcMain.on('o3-pcm-digest',(event,identity,scope,kind,key,bytes)=>{
 try{
  if(!own(event,identity)||!['original','compiled'].includes(scope)||typeof key!=='string'||!(/^(?:math-primitives|case:[0-9]{1,2}:(?:pcm|levels|coefficients))$/).test(key))throw Error('Invalid private digest identity');
  key=scope+':'+key;
  if(!digests.has(key)){digests.set(key,crypto.createHash('sha256'));lengths.set(key,0)}
  if(kind==='append'){
   if(!(bytes instanceof ArrayBuffer)||bytes.byteLength>65536)throw Error('Invalid private digest buffer');
   const next=lengths.get(key)+bytes.byteLength,totalBytes=(totals.get(scope)||0)+bytes.byteLength;totals.set(scope,totalBytes);
   if(next>19200000||totalBytes>32*1024*1024)throw Error('Private digest capacity exceeded');
   lengths.set(key,next);digests.get(key).update(Buffer.from(bytes));event.returnValue={ok:true};
  }else if(kind==='finish')event.returnValue={ok:true,value:lengths.get(key)+':'+digests.get(key).digest('hex')};
  else throw Error('Invalid private digest command');
 }catch(_){event.returnValue={ok:false};finish(3,{exit:3,error:'private_digest_boundary'})}
});
ipcMain.on('o3-pcm-complete',(event,identity,text)=>{
 if(!own(event,identity)||typeof text!=='string'||Buffer.byteLength(text)>128*1024){finish(4,{exit:4,error:'invalid_renderer_receipt'});return}
 try{
  const parsed=JSON.parse(text);for(const result of [parsed.result,parsed.compiled])if(result.wholeModuleCases!==73||result.fullLimitSamples!==4800000)throw Error('Invalid whole-module result');
  finish(0,{exit:0,runtime:process.versions,userAgent:parsed.userAgent,sourceSha256:expected,result:parsed.result,compiled:parsed.compiled,scope:'Actual sandboxed renderer V8, whole original and complete compiled resource numeric DSP with controlled AudioWorklet base/port; no microphone, media graph or GUI input acceptance'});
 }catch(_){finish(4,{exit:4,error:'invalid_renderer_receipt'})}
});
ipcMain.on('o3-pcm-failed',(event,identity,error)=>{if(own(event,identity))finish(5,{exit:5,error:String(error).slice(0,512)})});
app.whenReady().then(async()=>{
 server=http.createServer((request,response)=>{
  const prefix='/'+nonce+'/';
  if(request.method!=='GET'||!request.url.startsWith(prefix)){response.writeHead(404);response.end();return}
  const name=request.url.slice(prefix.length),body=files[name];
  if(!body){response.writeHead(404);response.end();return}
  response.writeHead(200,{'Content-Type':name.endsWith('.html')?'text/html; charset=utf-8':'application/javascript; charset=utf-8','Content-Length':body.length,'Content-Security-Policy':"default-src 'none'; script-src 'self'; connect-src 'self'"});response.end(body);
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));origin='http://127.0.0.1:'+server.address().port;
 const isolated=session.fromPartition('pcm-'+nonce,{cache:false});
 isolated.setPermissionCheckHandler(()=>false);isolated.setPermissionRequestHandler((_contents,_permission,callback)=>callback(false));
 isolated.webRequest.onBeforeRequest((details,callback)=>callback({cancel:!details.url.startsWith(origin+'/'+nonce+'/')}));
 window=new BrowserWindow({show:false,width:64,height:64,webPreferences:{session:isolated,preload:path.join(root,'pcm-renderer-preload.cjs'),additionalArguments:['--pcm-nonce='+nonce],sandbox:true,nodeIntegration:false,contextIsolation:true,spellcheck:false,backgroundThrottling:false}});
 window.webContents.setWindowOpenHandler(()=>({action:'deny'}));window.webContents.on('will-attach-webview',event=>event.preventDefault());
 window.webContents.on('render-process-gone',(_event,details)=>finish(6,{exit:6,error:'renderer_gone',reason:details.reason,code:details.exitCode}));
 await window.loadURL(origin+'/'+nonce+'/index.html');
}).catch(()=>finish(7,{exit:7,error:'renderer_startup_failed'}));
setTimeout(()=>finish(8,{exit:8,error:'renderer_deadline'}),60000).unref();
