// Private numeric oracle only. Renderer privileges remain sandboxed and scoped
// to one digest endpoint; no filesystem or arbitrary IPC is exposed.
const {contextBridge,ipcRenderer}=require('electron');
const nonce=process.argv.find(value=>value.startsWith('--pcm-nonce='))?.slice(12);
if(!nonce)throw Error('Missing private PCM oracle identity');
function call(scope,kind,key,bytes){
 const reply=ipcRenderer.sendSync('o3-pcm-digest',nonce,scope,kind,key,bytes);
 if(!reply||reply.ok!==true)throw Error('Private PCM digest refused');
 return reply.value;
}
contextBridge.exposeInMainWorld('pcmProbe',{
 append:(scope,key,bytes)=>call(scope,'append',key,bytes),
 finish:(scope,key)=>call(scope,'finish',key),
 complete:value=>ipcRenderer.send('o3-pcm-complete',nonce,JSON.stringify(value)),
 failed:error=>ipcRenderer.send('o3-pcm-failed',nonce,String(error).slice(0,512)),
});
