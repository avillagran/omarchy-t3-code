#!/usr/bin/env node
// Execute the actual pinned native module against the generated trusted factory.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),vm=require('node:vm'),{stripTypeScriptTypes}=require('node:module');
const {build,revision}=require('./build-native-cdp.cjs');
const root=path.resolve(__dirname,'..'),nativeRoot=process.argv[2];
if(!nativeRoot)throw Error('Usage: test-native-cdp.cjs NATIVE_ROOT');
const generated=fs.readFileSync(path.join(root,'ui/browser/native-cdp-relay.js'),'utf8');
assert.equal(generated,build(nativeRoot));
const nativeSource=fs.readFileSync(path.join(nativeRoot,'apps/desktop/src/preview/CdpRelay.ts'),'utf8');
const settle=()=>new Promise(resolve=>setImmediate(resolve));
async function drive(factory) {
 const excessive='x'.repeat(64*1024*1024),escaped='\n'.repeat(11*1024*1024);
 const frames=[],calls=[],trace=[],detached=[],downloads=[],waiting=[];let closed=0;
 const relay=factory({
  send:(method,params,sessionId,owner)=>{calls.push({method,params,sessionId,owner});if(method==='Slow.fixture')return new Promise(resolve=>waiting.push(resolve));if(method==='Large.fixture')return Promise.resolve({value:excessive});if(method==='LargeEscaped.fixture')return Promise.resolve({value:escaped});if(method==='Fail.fixture')return Promise.reject(Error('fixture rejected'));return Promise.resolve({fixture:method});},
  targetId:()=>Promise.resolve('native-fixture-target'),url:()=> 'data:text/html,fixture',title:()=> 'fixture',userAgent:()=> 'fixture-agent',sessionDetached:id=>detached.push(id),setDownloadDirectory:value=>downloads.push(value)
 },raw=>frames.push(JSON.parse(raw)),()=>closed++);
 let id=0;
 const send=async(method,params={},sessionId)=>{trace.push({method,params,sessionId});relay.receive(JSON.stringify({id:++id,method,params,...(sessionId===undefined?{}:{sessionId})}));await settle();};
 relay.event('Page.fixtureBeforeAttach',{},undefined);
 for(const method of ['Target.setDiscoverTargets','Browser.grantPermissions','Browser.resetPermissions','Browser.cancelDownload','Browser.getVersion','Target.getTargetInfo','Target.getBrowserContexts','Target.getTargets'])await send(method);
 await send('Target.attachToTarget',{targetId:'wrong-target'});
 await send('Target.attachToBrowserTarget');const browser=frames.at(-1).result.sessionId;
 await send('Target.setAutoAttach',{},browser);await send('Target.setAutoAttach',{},browser);
 await send('Target.attachToTarget',{targetId:'native-fixture-target'},browser);const page=frames.at(-1).result.sessionId;
 await send('Runtime.runIfWaitingForDebugger',{},page);await send('Emulation.setFocusEmulationEnabled',{enabled:true},page);
 await send('Runtime.evaluate',{expression:'fixture'},page);
 await send('Runtime.evaluate',{expression:'child'},'actual-child-session');
 await send('Runtime.runIfWaitingForDebugger',{},'actual-child-session');
 await send('Fail.fixture',{},page);await send('Unsupported.fixture',{},browser);
 await send('Browser.setDownloadBehavior',{behavior:'allow',downloadPath:'/fixture/downloads',eventsEnabled:true},browser);
 relay.event('Page.downloadWillBegin',{guid:'fixture'},undefined);
 relay.event('Page.fixture',{value:1},undefined);relay.event('Runtime.fixture',{value:2},'actual-child-session');
 await send('Target.detachFromTarget',{sessionId:page},browser);relay.event('Page.afterDetach',{},undefined);
 await send('Target.detachFromTarget',{sessionId:'t3-preview-page'},browser);
 await send('Browser.setDownloadBehavior',{behavior:'deny',downloadPath:'/fixture/refused',eventsEnabled:false},browser);
 relay.event('Page.downloadProgress',{guid:'ignored'},undefined);
 await send('Slow.fixture',{},'t3-preview-page');const slowId=id;
 await send('Runtime.evaluate',{expression:'overtakes'},'t3-preview-page');waiting[0]({slow:true});await settle();
 assert(frames.findIndex(frame=>frame.id===slowId)>frames.findIndex(frame=>frame.id===slowId+1));
 for(const raw of ['not-json','null','{}','{"id":"wrong","method":"Target.getTargets"}']){trace.push({raw});relay.receive(raw);}
 // Oversized command results fail that command while ordinary events survive.
 await send('Large.fixture',{},'t3-preview-page');assert.equal(frames.at(-1).error.message,'CDP payload too large');
 await send('LargeEscaped.fixture',{},'t3-preview-page');assert.equal(frames.at(-1).error.message,'CDP payload too large');
 relay.event('Page.afterLargeReply',{},undefined);assert.equal(closed,0);
 // Oversized events retire the relay instead of silently desynchronizing it.
 relay.event('Page.oversized',{value:excessive},undefined);
 await send('Target.getTargets');
 return {trace,frames,calls,detached,downloads,closed};
}
(async()=>{
 const native=await import('data:text/javascript;base64,'+Buffer.from(stripTypeScriptTypes(nativeSource,{mode:'strip'})).toString('base64'));
 vm.runInThisContext(generated,{filename:'native-cdp-relay.js'});
 const expected=await drive(native.createCdpRelayConnection),actual=await drive(globalThis.__o3CreateCdpRelayConnection);
 assert.deepEqual(actual,expected);assert.equal(actual.closed,1);
 console.log('NATIVE_CDP_PASS '+JSON.stringify({revision,traces:actual.trace.length,frames:actual.frames.length,targetCalls:actual.calls.length,closed:actual.closed}));
})().catch(error=>{console.error(error);process.exitCode=1});
