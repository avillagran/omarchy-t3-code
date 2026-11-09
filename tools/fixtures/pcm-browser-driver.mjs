import {createCapture} from './compiled.mjs';
import {createBrowserCaptureOwner} from './browser-capture-owner.mjs';
const assert=(value,message)=>{if(!value)throw Error(message)};
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(predicate){const end=performance.now()+6000;while(!predicate()){if(performance.now()>end)throw Error('Synthetic audio deadline');await delay(20)}}
function fixture(microphone='',overrides={}){
 const observation={packets:0,bytes:0,levels:0,nonzero:0,errors:[],limits:0,requests:[],settings:[]};
 let signalRequest;const entered=new Promise(resolve=>signalRequest=resolve);
 const platform={AbortController,AudioContext,AudioWorkletNode,Blob,URL,setTimeout:setTimeout.bind(globalThis),clearTimeout:clearTimeout.bind(globalThis),fetch:overrides.fetch||fetch.bind(globalThis),
  navigator:{mediaDevices:{getUserMedia(constraints){
   observation.requests.push(constraints);signalRequest();
   const pending=navigator.mediaDevices.getUserMedia(constraints);
   void pending.then(stream=>{for(const track of stream.getAudioTracks()){
    const settings=track.getSettings();observation.settings.push({sampleRate:settings.sampleRate,channelCount:settings.channelCount,echoCancellation:settings.echoCancellation,autoGainControl:settings.autoGainControl,noiseSuppression:settings.noiseSuppression});
   }},()=>{});return pending;
  }}},
 };
 const owner=createBrowserCaptureOwner({createCapture,workletUrl:new URL('worklet.mjs',import.meta.url).href,getMicrophoneId:()=>microphone,
  onLevel:value=>{assert(Number.isFinite(value)&&value>=0&&value<=1,'Invalid real meter');observation.levels++},
  onPcm:pcm=>{observation.packets++;observation.bytes+=pcm.byteLength;for(const sample of pcm){assert(Number.isFinite(sample),'Nonfinite real PCM');if(sample!==0)observation.nonzero++}if(overrides.refusePcm)throw Error('Owned synthetic sink refused audio')},
  onError:error=>observation.errors.push(error),onDurationLimit:()=>observation.limits++,
 },platform);
 return {owner,observation,entered};
}
function closed(owner){const view=owner.inspect();assert(view.state==='closed'&&view.trackStates.every(state=>state==='ended')&&view.contextStates.every(state=>state==='closed'),'Actual audio resources remain live');return view}
const cases=[];
try{
 {
  const {owner,observation}=fixture();await owner.prepare('streaming');const prepared=owner.inspect();assert(prepared.contextStates[0]==='running'&&prepared.trackStates[0]==='live','Real graph not running');
  owner.record();await until(()=>observation.packets>=3);assert(await owner.stop()===null,'Streaming produced a buffered URI');await owner.close();const settled=closed(owner),packets=observation.packets;await delay(80);
  assert(observation.packets===packets&&observation.nonzero>0&&observation.levels>0&&!observation.errors.length&&!observation.limits,'Real streaming observations failed');
  assert(JSON.stringify(observation.requests)==='[{"audio":true}]','Changed native default constraints');
  cases.push({name:'real-streaming',...observation,prepared,settled});
 }
 {
  const {owner,observation}=fixture();await owner.prepare('buffered');owner.record();await delay(1400);const pcm=await owner.stop();assert(pcm instanceof ArrayBuffer&&pcm.byteLength>0&&pcm.byteLength<=19200000,'Missing bounded real buffered PCM');
  let nonzero=0;for(const sample of new Float32Array(pcm)){assert(Number.isFinite(sample),'Nonfinite buffered PCM');if(sample!==0)nonzero++}assert(nonzero>0&&observation.packets===0&&!observation.errors.length,'Buffered delivery failed');
  const sha256=[...new Uint8Array(await crypto.subtle.digest('SHA-256',pcm))].map(value=>value.toString(16).padStart(2,'0')).join('');
  await owner.close();cases.push({name:'real-buffered',bytes:pcm.byteLength,sha256,nonzero,observation,settled:closed(owner)});
 }
 {
  const {owner,observation}=fixture();await owner.prepare('streaming');const first=owner.close();await first;await owner.close();let refused=false;try{owner.record()}catch(_){refused=true}assert(refused&&observation.packets===0,'Closed owner admitted capture');
  cases.push({name:'cancel-prepared',observation,settled:closed(owner)});
 }
 {
  const {owner,observation,entered}=fixture();const preparing=owner.prepare('streaming');const outcome=preparing.then(()=>false,error=>error.name==='AbortError');await entered;await owner.close();assert(await outcome,'Pending permission cancellation did not abort');
  assert(owner.inspect().contextStates.length===0&&observation.packets===0,'Late permission created a graph');cases.push({name:'cancel-pending-permission',observation,settled:closed(owner)});
 }
 {
  let entered,release;const waiting=new Promise(resolve=>entered=resolve),resume=new Promise(resolve=>release=resolve);
  const {owner,observation}=fixture('',{fetch:async uri=>{
   const response=await fetch(uri);return {arrayBuffer:async()=>{const bytes=await response.arrayBuffer();entered();await resume;return bytes}};
  }});
  await owner.prepare('buffered');owner.record();await delay(650);const stopping=owner.stop();const outcome=stopping.then(()=>false,error=>error.name==='AbortError');await waiting;await owner.close();release();assert(await outcome,'Cancelled buffered read published late audio');
  assert(observation.packets===0,'Buffered cancellation streamed audio');cases.push({name:'cancel-buffered-read',observation,settled:closed(owner)});
 }
 {
  const {owner,observation}=fixture('',{refusePcm:true});await owner.prepare('streaming');owner.record();await until(()=>owner.inspect().state==='closed');await owner.close();
  assert(observation.errors.length===1&&observation.packets===1,'Failed sink was reused');const packets=observation.packets;await delay(80);assert(observation.packets===packets,'Failed sink received late audio');cases.push({name:'failed-stream-sink',observation,settled:closed(owner)});
 }
 {
  const {owner,observation}=fixture('owned-fixture-missing-device');let failure;try{await owner.prepare('streaming')}catch(error){failure=error.name}assert(['OverconstrainedError','NotFoundError'].includes(failure),'Exact unavailable microphone was accepted');await owner.close();assert(observation.packets===0,'Failed permission produced audio');
  assert(JSON.stringify(observation.requests)==='[{"audio":{"deviceId":{"exact":"owned-fixture-missing-device"}}}]','Changed native exact device constraints');cases.push({name:'missing-exact-device',failure,observation,settled:closed(owner)});
 }
 window.pcmBrowser.complete({cases,scope:'Actual original capture resource, real Chromium getUserMedia/AudioContext/AudioWorklet with fake device only; no microphone, consent UI or server transport acceptance'});
}catch(error){window.pcmBrowser.failed(String(error&&error.stack||error))}
