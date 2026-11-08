// Only the sealed private capture page receives this fixed message surface.
const {contextBridge,ipcRenderer}=require('electron');
const nonce=process.argv.find(value=>value.startsWith('--voice-nonce='))?.slice(14);
if(!nonce)throw Error('Missing private capture identity');
let subscribed=false;
contextBridge.exposeInMainWorld('voiceCapture',{
 subscribe:callback=>{
  if(subscribed||typeof callback!=='function')throw Error('Capture command listener unavailable');subscribed=true;
  ipcRenderer.on('o3-voice-command',(_event,identity,command)=>{if(identity===nonce)callback(command)});
 },
 ready:()=>ipcRenderer.send('o3-voice-renderer-ready',nonce),
 result:(id,ok,value)=>ipcRenderer.send('o3-voice-result',nonce,id,ok,value),
 microphones:devices=>ipcRenderer.send('o3-voice-microphones',nonce,devices),
 pcm:(kind,bytes)=>ipcRenderer.send('o3-voice-pcm',nonce,kind,bytes),
 level:value=>ipcRenderer.send('o3-voice-level',nonce,value),
 failed:()=>ipcRenderer.send('o3-voice-failed',nonce),
 limit:()=>ipcRenderer.send('o3-voice-limit',nonce),
});
