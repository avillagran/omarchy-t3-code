import {createCapture} from './compiled.mjs';
import {createBrowserCaptureOwner} from './browser-capture-owner.mjs';
let owner,mode;
const bridge=window.voiceCapture;
bridge.subscribe(async command=>{
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
  }else if(command.type==='record')owner.record();
  else if(command.type==='stop'){
   const pcm=await owner.stop();
   if(pcm)bridge.pcm('buffer',pcm);
   value={mode,pcmBytes:pcm?pcm.byteLength:owner.inspect().deliveredBytes};
  }else if(command.type==='close'){
   if(owner)await owner.close();value={cleanup:'confirmed'};
  }else throw Error('Unsupported capture command');
  bridge.result(command.id,true,value);
 }catch(error){bridge.result(command.id,false,{error:error?.name==='AbortError'?'capture_cancelled':'capture_failed'})}
});
bridge.ready();
