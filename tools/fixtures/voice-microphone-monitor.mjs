// Synthetic event/metadata plumbing only; no hardware, media grants or streams.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createMicrophoneMonitor} from '../../ui/voice/browser-microphone-monitor.mjs';
if(!fs.existsSync('/run/.containerenv')||process.versions.node!=='26.13.0')throw Error('Private pinned container runtime required');
const wait=milliseconds=>new Promise(resolve=>setTimeout(resolve,milliseconds));
const results=[];
function deferred(){let resolve,reject;return{promise:new Promise((yes,no)=>{resolve=yes;reject=no}),resolve,reject}}
function media(){const listeners=new Set();return{listeners,addEventListener:(name,callback)=>{assert.equal(name,'devicechange');listeners.add(callback)},removeEventListener:(name,callback)=>{assert.equal(name,'devicechange');listeners.delete(callback)},change:()=>{for(const callback of [...listeners])callback()}}}
const device=label=>[{deviceId:'opaque-device',label}];
async function run(name,test){await test();results.push(name)}

await run('original helper and initial read-only lifecycle',async()=>{
 const devices=media();let enumerations=0,streams=0;const published=[];
 devices.enumerateDevices=async()=>{enumerations++;return[{kind:'audioinput',deviceId:'',label:''},{kind:'videoinput',deviceId:'camera',label:'Camera'},{kind:'audioinput',deviceId:'opaque-device',label:'<b>Literal</b>'}]};
 devices.getUserMedia=()=>{streams++;throw Error('No synthetic audio stream is allowed')};
 const previous=Object.getOwnPropertyDescriptor(globalThis,'navigator');
 Object.defineProperty(globalThis,'navigator',{value:{mediaDevices:devices},configurable:true});
 const monitor=createMicrophoneMonitor({mediaDevices:devices,publish:value=>published.push(value)});
 try{
  await assert.rejects(monitor.refresh(),/capture_cancelled/);assert.equal(enumerations,0);
  assert.deepEqual(await monitor.start(),[{deviceId:'',label:'Microphone 1'},{deviceId:'opaque-device',label:'<b>Literal</b>'}]);
  assert.equal(devices.listeners.size,1);assert.equal(enumerations,1);assert.equal(streams,0);assert.equal(published.length,0);
 }finally{monitor.stop();if(previous)Object.defineProperty(globalThis,'navigator',previous);else delete globalThis.navigator}
 assert.equal(devices.listeners.size,0);
});

await run('burst and manual refresh coalesce with one native enumeration',async()=>{
 const devices=media(),second=deferred(),third=deferred(),published=[],starts=[];
 let calls=0,inFlight=0,maximum=0;
 const monitor=createMicrophoneMonitor({mediaDevices:devices,publish:value=>published.push(value),enumerate:()=>{
  calls++;starts.push(Date.now());inFlight++;maximum=Math.max(maximum,inFlight);
  return (calls===1?Promise.resolve(device('initial')):calls===2?second.promise:third.promise).finally(()=>inFlight--);
 }});
 try{
  await monitor.start();const refresh=monitor.refresh();await wait(300);assert.equal(calls,2);
  for(let index=0;index<100;index++)devices.change();
  assert.equal(monitor.refresh(),refresh);assert.equal(calls,2);
  second.resolve(device('second'));await refresh;await wait(300);assert.equal(calls,3);
  third.resolve(device('third'));await wait(0);
  assert.equal(maximum,1);assert.equal(published.length,2);assert.equal(published[1][0].label,'third');
  assert.ok(starts[1]-starts[0]>=225);assert.ok(starts[2]-starts[1]>=225);
 }finally{monitor.stop()}
});

await run('stop cancels scheduled refresh and removes event listener',async()=>{
 const devices=media(),published=[];let calls=0;
 const monitor=createMicrophoneMonitor({mediaDevices:devices,publish:value=>published.push(value),enumerate:async()=>{calls++;return device('initial')}});
 await monitor.start();const refresh=monitor.refresh();const rejected=assert.rejects(refresh,error=>error.name==='AbortError');
 monitor.stop();devices.change();await rejected;await wait(300);
 assert.equal(calls,1);assert.equal(published.length,0);assert.equal(devices.listeners.size,0);
});

await run('late native completion after stop cannot publish metadata',async()=>{
 const devices=media(),held=deferred(),published=[];let calls=0;
 const monitor=createMicrophoneMonitor({mediaDevices:devices,publish:value=>published.push(value),enumerate:()=>++calls===1?Promise.resolve(device('initial')):held.promise});
 await monitor.start();const refresh=monitor.refresh();const rejected=assert.rejects(refresh,error=>error.name==='AbortError');
 await wait(300);assert.equal(calls,2);monitor.stop();held.resolve(device('late'));await rejected;await wait(0);
 assert.equal(published.length,0);assert.equal(devices.listeners.size,0);
});

await run('enumeration error can be retried without opening media',async()=>{
 const devices=media(),published=[];let calls=0;
 const monitor=createMicrophoneMonitor({mediaDevices:devices,publish:value=>published.push(value),enumerate:async()=>{if(++calls===2)throw Error('Synthetic enumeration failure');return device('valid')}});
 try{
  await monitor.start();await assert.rejects(monitor.refresh(),/microphones_unavailable/);
  assert.equal(devices.listeners.size,1);assert.deepEqual(await monitor.refresh(),device('valid'));
  assert.equal(published.length,1);
 }finally{monitor.stop()}
});

await run('hung enumeration disables metadata instead of accumulating native requests',async()=>{
 const devices=media(),held=deferred();let calls=0,published=0;
 const monitor=createMicrophoneMonitor({mediaDevices:devices,publish:()=>published++,enumerate:()=>{calls++;return held.promise}});
 await assert.rejects(monitor.start(),/microphones_unavailable/);
 for(let index=0;index<100;index++)devices.change();
 await assert.rejects(monitor.refresh(),/microphones_unavailable/);
 held.resolve(device('late after timeout'));await wait(0);monitor.stop();
 assert.equal(calls,1);assert.equal(published,0);assert.equal(devices.listeners.size,0);
});
process.stdout.write(JSON.stringify({complete:true,cases:results.length,results,scope:'Synthetic event/metadata coalescing, original enumerator and cancellation only; no physical hotplug or audio capture'})+'\n');
