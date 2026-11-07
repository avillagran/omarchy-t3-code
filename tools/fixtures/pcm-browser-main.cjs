// Actual media graph fixture. Only the admitted synthetic Chromium device is used.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),crypto=require('node:crypto');
const {app,BrowserWindow,ipcMain,session}=require('electron');
if(!fs.existsSync('/run/.containerenv')||process.versions.electron!=='44.10.1'||!app.commandLine.hasSwitch('use-fake-device-for-media-stream'))throw Error('Private synthetic media runtime required');
const nonce=crypto.randomBytes(24).toString('hex'),root='/source';
const resource=fs.readFileSync(path.join(root,'NativePcmCapture.js'));
if(crypto.createHash('sha256').update(resource).digest('hex')!=='82e67fc3e59f40b522492cd85ed340254ebb96fc2a664d796766be957f1f9c83')throw Error('Changed capture resource');
const files={
 'index.html':Buffer.from('<!doctype html><meta charset="utf-8"><script type="module" src="driver.mjs"></script>'),
 'compiled.mjs':Buffer.from(resource.toString().replace(/^\.pragma library\n/,'')+'\nexport {createDsp,createCapture};\n'),
 'worklet.mjs':Buffer.from("import {createDsp} from './compiled.mjs';createDsp(sampleRate,{AudioWorkletProcessor,registerProcessor,math:Math});"),
 'driver.mjs':fs.readFileSync(path.join(root,'pcm-browser-driver.mjs')),
 'browser-capture-owner.mjs':fs.readFileSync(path.join(root,'browser-capture-owner.mjs')),
};
app.setPath('userData','/state/profile');app.setPath('cache','/state/cache');
let window,server,origin,finished=false,mediaRequests=0,deniedRequests=0;
function finish(code,record){if(finished)return;finished=true;if(record)fs.writeFileSync('/state/browser.json',JSON.stringify({...record,runtime:process.versions,mediaRequests,deniedRequests})+'\n',{flag:'wx',mode:0o600});if(window&&!window.isDestroyed())window.destroy();if(server)server.close();app.exit(code)}
function own(event,identity){return !finished&&identity===nonce&&window&&!window.isDestroyed()&&event.sender===window.webContents&&event.senderFrame===window.webContents.mainFrame&&event.senderFrame.url===origin+'/'+nonce+'/index.html'}
ipcMain.on('o3-browser-complete',(event,identity,text)=>{
 if(!own(event,identity)||typeof text!=='string'||Buffer.byteLength(text)>128*1024)return finish(3,{error:'invalid_private_receipt'});
 try{const result=JSON.parse(text);if(result.cases.length!==7)throw Error();finish(0,{exit:0,result})}catch(_){finish(3,{error:'invalid_private_receipt'})}
});
ipcMain.on('o3-browser-failed',(event,identity,error)=>{if(own(event,identity))finish(4,{exit:4,error:String(error).slice(0,2048)})});
app.whenReady().then(async()=>{
 server=http.createServer((request,response)=>{
  const prefix='/'+nonce+'/';if(request.method!=='GET'||!request.url.startsWith(prefix)){response.writeHead(404);response.end();return}
  const name=request.url.slice(prefix.length),body=files[name];if(!body){response.writeHead(404);response.end();return}
  response.writeHead(200,{'Content-Type':name.endsWith('.html')?'text/html; charset=utf-8':'application/javascript; charset=utf-8','Content-Length':body.length,'Content-Security-Policy':"default-src 'none'; script-src 'self'; connect-src 'self' blob:; worker-src 'self'; media-src blob:"});response.end(body);
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));origin='http://127.0.0.1:'+server.address().port;
 const isolated=session.fromPartition('synthetic-pcm-'+nonce,{cache:false});
 const ownedContents=contents=>window&&!window.isDestroyed()&&contents===window.webContents&&contents.mainFrame.url===origin+'/'+nonce+'/index.html';
 isolated.setPermissionCheckHandler((contents,permission,requestingOrigin)=>ownedContents(contents)&&permission==='media'&&requestingOrigin===origin);
 isolated.setPermissionRequestHandler((contents,permission,callback,details)=>{
  const allowed=ownedContents(contents)&&permission==='media'&&details.isMainFrame===true&&details.requestingUrl===origin+'/'+nonce+'/index.html'&&Array.isArray(details.mediaTypes)&&details.mediaTypes.length===1&&details.mediaTypes[0]==='audio';
  if(allowed)mediaRequests++;else deniedRequests++;callback(allowed);
 });
 isolated.webRequest.onBeforeRequest((details,callback)=>callback({cancel:!details.url.startsWith(origin+'/'+nonce+'/')&&!details.url.startsWith('blob:'+origin+'/')}));
 window=new BrowserWindow({show:false,width:64,height:64,webPreferences:{session:isolated,preload:path.join(root,'pcm-browser-preload.cjs'),additionalArguments:['--pcm-nonce='+nonce],sandbox:true,nodeIntegration:false,contextIsolation:true,spellcheck:false,backgroundThrottling:false}});
 window.webContents.setAudioMuted(true);window.webContents.setWindowOpenHandler(()=>({action:'deny'}));window.webContents.on('will-attach-webview',event=>event.preventDefault());window.webContents.on('will-navigate',event=>event.preventDefault());
 window.webContents.on('render-process-gone',(_event,details)=>finish(5,{exit:5,error:'renderer_gone',reason:details.reason,code:details.exitCode}));
 await window.loadURL(origin+'/'+nonce+'/index.html');
}).catch(()=>finish(6,{exit:6,error:'private_media_startup_failed'}));
setTimeout(()=>finish(7,{exit:7,error:'private_media_deadline'}),45000).unref();
