// Private fixture only; no capture commands, credentials or raw audio cross IPC.
const {contextBridge,ipcRenderer}=require('electron');
const nonce=process.argv.find(value=>value.startsWith('--pcm-nonce='))?.slice(12);
if(!nonce)throw Error('Missing private PCM identity');
contextBridge.exposeInMainWorld('pcmBrowser',{
 complete:value=>ipcRenderer.send('o3-browser-complete',nonce,JSON.stringify(value)),
 failed:error=>ipcRenderer.send('o3-browser-failed',nonce,String(error).slice(0,2048)),
});
