// One private renderer capture lifetime. The host supplies only the pinned
// factory/worklet and local callbacks; this module has no server credentials.
export function createBrowserCaptureOwner(input, platform=globalThis) {
 const MaxBytes=4800000*4;
 const controller=new platform.AbortController();
 const streams=new Set(),contexts=new Set(),permissions=new Set(),closingContexts=new Set(),contextClosures=new WeakMap();
 let state='new',closePromise,mode,bytes=0,consumed=false;
 const alive=()=>!controller.signal.aborted;
 class OwnedAudioContext extends platform.AudioContext {
  constructor(...args){super(...args);contexts.add(this)}
  close(){if(contextClosures.has(this))return contextClosures.get(this);const pending=super.close();contextClosures.set(this,pending);closingContexts.add(pending);void pending.catch(()=>{});return pending}
 }
 const observedMedia={getUserMedia(constraints){
  const pending=platform.navigator.mediaDevices.getUserMedia(constraints);
  permissions.add(pending);
  void pending.then(stream=>{
   streams.add(stream);
   if(!alive())for(const track of stream.getTracks())track.stop();
  },()=>{});
  return pending;
 }};
 function fail(message){if(!alive())return;try{input.onError(message)}catch(_){}finally{void close().catch(()=>{})}}
 const capture=input.createCapture({
  navigator:{mediaDevices:observedMedia},AudioContext:OwnedAudioContext,
  AudioWorkletNode:platform.AudioWorkletNode,Blob:platform.Blob,URL:platform.URL,
  setTimeout:platform.setTimeout.bind(platform),clearTimeout:platform.clearTimeout.bind(platform),
  workletUrl:input.workletUrl,
 }).createBrowserPcmCapture({
  getMicrophoneId:input.getMicrophoneId,
  onLevel:value=>{if(alive())input.onLevel(value)},
  onDurationLimit:()=>{if(alive())input.onDurationLimit()},onError:fail,
 },async()=>mode==='streaming'?{streaming:{feed(pcm){
  if(!alive())return;
  if(!(pcm instanceof Float32Array)||pcm.byteLength>32000||bytes>MaxBytes-pcm.byteLength){fail('Microphone processing returned invalid PCM.');return}
  bytes+=pcm.byteLength;
  try{input.onPcm(pcm)}catch(_){fail('Microphone audio delivery failed.')}
 }}}:{});
 async function close(){
  if(closePromise)return closePromise;
  state='closing';controller.abort();capture.cancelRecording();
  if(capture.recorder.uri)capture.deleteRecording(capture.recorder.uri);
  closePromise=(async()=>{
   // A pending system permission may outlive cancellation. The process owner
   // must fence the renderer if it cannot settle; an early receipt is unsafe.
   await Promise.allSettled([...permissions]);
   for(const stream of streams)for(const track of stream.getTracks())track.stop();
   for(const context of contexts)if(context.state!=='closed')void context.close().catch(()=>{});
   const outcomes=await Promise.allSettled([...closingContexts]);
   if([...contexts].some(context=>context.state!=='closed')||outcomes.some(row=>row.status==='rejected')){
    state='unknown';throw Error('Microphone cleanup could not be confirmed.');
   }
   state='closed';
  })().catch(error=>{state='unknown';throw error});
  return closePromise;
 }
 return {
  async prepare(selectedMode){
   if(state!=='new'||!['streaming','buffered'].includes(selectedMode))throw Error('Microphone capture is already prepared.');
   state='preparing';mode=selectedMode;
   try{
    await capture.transcriber.prepare({signal:controller.signal});
    await capture.recorder.prepareToRecordAsync();controller.signal.throwIfAborted();state='prepared';
   }catch(error){await close();throw error}
  },
  record(){
   if(state!=='prepared')throw Error('Microphone capture is not prepared.');
   controller.signal.throwIfAborted();state='recording';capture.recorder.record({forDuration:300});
  },
  async stop(){
   if(state!=='recording'||consumed)throw Error('Microphone capture is not recording.');
   consumed=true;state='stopping';
   try{
    await capture.recorder.stop();controller.signal.throwIfAborted();
    let pcm=null;
    if(mode==='buffered'&&capture.recorder.uri){
     const uri=capture.recorder.uri;
     try{
      const response=await platform.fetch(uri);pcm=await response.arrayBuffer();
      if(pcm.byteLength>MaxBytes||pcm.byteLength%4)throw Error('Microphone recording exceeded its buffer limit.');
     }finally{capture.deleteRecording(uri)}
    }
    controller.signal.throwIfAborted();state='stopped';return pcm;
   }catch(error){await close();throw error}
  },
  close,
  inspect(){return {state,streamCount:streams.size,trackStates:[...streams].flatMap(stream=>stream.getTracks().map(track=>track.readyState)),contextStates:[...contexts].map(context=>context.state),deliveredBytes:bytes}},
 };
}
