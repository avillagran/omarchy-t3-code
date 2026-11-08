import {listBrowserMicrophones} from './browser-microphones.mjs';

// One active capture owns this observer. Enumeration never opens another stream.
export function createMicrophoneMonitor({publish,mediaDevices=navigator.mediaDevices,enumerate=listBrowserMicrophones}) {
 let active=false,failed=false,epoch=0,work=null,dirty=false,nextAt=0;
 const cancelled=()=>Object.assign(Error('capture_cancelled'),{name:'AbortError'});
 function detach(){mediaDevices.removeEventListener('devicechange',changed)}
 function stop(){
  active=false;epoch++;dirty=false;detach();
  if(work){clearTimeout(work.scheduled);clearTimeout(work.deadline);work.reject(cancelled());work=null}
 }
 function request(publishResult=true){
  if(!active||failed)return Promise.reject(Error(failed?'microphones_unavailable':'capture_cancelled'));
  if(work){dirty=true;return work.promise}
  let resolve,reject;
  const pending={promise:new Promise((yes,no)=>{resolve=yes;reject=no}),resolve,reject,epoch,publishResult};
  work=pending;dirty=false;
  pending.scheduled=setTimeout(async()=>{
   if(!active||work!==pending||pending.epoch!==epoch)return;
   const timeout=new Promise((_,rejectTimeout)=>{
    pending.deadline=setTimeout(()=>rejectTimeout(Error('microphones_timeout')),3000);
   });
   try{
    const devices=await Promise.race([enumerate(),timeout]);
    if(!active||work!==pending||pending.epoch!==epoch)return;
    if(pending.publishResult)publish(devices);
    pending.resolve(devices);
   }catch(error){
    if(!active||work!==pending||pending.epoch!==epoch)return;
    if(error?.message==='microphones_timeout'){
     // A timed-out native enumeration cannot be cancelled. Disable this observer
     // rather than accumulate more unresolved native requests during recording.
     failed=true;detach();dirty=false;
    }
    pending.reject(Error('microphones_unavailable'));
   }finally{
    clearTimeout(pending.deadline);
    if(work===pending){work=null;nextAt=Date.now()+250;if(active&&dirty&&!failed)void request().catch(()=>{})}
   }
  },Math.max(0,nextAt-Date.now()));
  return pending.promise;
 }
 function changed(){void request().catch(()=>{})}
 return {
  start:()=>{if(active||failed)throw Error('Microphone observer already used');active=true;mediaDevices.addEventListener('devicechange',changed);return request(false)},
  refresh:()=>request(),stop,
 };
}
