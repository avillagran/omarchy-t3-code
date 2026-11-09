import {createCapture} from './compiled.mjs';
import {createBrowserCaptureOwner} from './browser-capture-owner.mjs';
import {createMicrophoneMonitor} from './browser-microphone-monitor.mjs';
let owner,mode,microphones;
const bridge=window.voiceCapture;
bridge.subscribe(async command=>{
 if(command.type==='microphones-ended'){if(microphones)microphones.stop();return}
 try{
  let value=null;
  if(command.type==='prepare'){
   if(owner)throw Error('Capture already used');mode=command.mode;
   owner=createBrowserCaptureOwner({createCapture,workletUrl:new URL('worklet.mjs',import.meta.url).href,
    getMicrophoneId:()=>command.microphoneId,
    onPcm:pcm=>bridge.pcm('stream',pcm.buffer.slice(pcm.byteOffset,pcm.byteOffset+pcm.byteLength)),
    onLevel:bridge.level,onDurationLimit:bridge.limit,onError:bridge.failed,
   });
   await owner.prepare(mode);
   microphones=createMicrophoneMonitor({publish:bridge.microphones});
   // Only an explicitly prepared capture starts the read-only device observer.
   try{value={microphones:await microphones.start()}}catch(_){value={microphones:null}}
  }else if(command.type==='refresh-microphones'){
   if(!microphones)throw Error('microphones_unavailable');await microphones.refresh();
  }else if(command.type==='record')owner.record();
  else if(command.type==='stop'){
   if(microphones)microphones.stop();
   const pcm=await owner.stop();
   if(pcm)bridge.pcm('buffer',pcm);
   value={mode,pcmBytes:pcm?pcm.byteLength:owner.inspect().deliveredBytes};
  }else if(command.type==='close'){
   if(microphones)microphones.stop();
   if(owner)await owner.close();value={cleanup:'confirmed'};
  }else throw Error('Unsupported capture command');
  bridge.result(command.id,true,value);
 }catch(error){bridge.result(command.id,false,{error:error?.name==='AbortError'?'capture_cancelled':command.type==='refresh-microphones'?'microphones_unavailable':error?.name==='NotAllowedError'?'capture_permission_denied':'capture_failed'})}
});
bridge.ready();
