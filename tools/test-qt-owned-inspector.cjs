'use strict';
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const tick=()=>new Promise(setImmediate);
function fixture(){
 const sent=[],incoming=[],hostCommands=[],hosts=[],retired=[];let transportClosed=false;
 const core={send:text=>{if(transportClosed)throw Error('closed');sent.push(JSON.parse(text));},drain:()=>({messages:incoming.splice(0),closed:transportClosed,failure:null}),close:()=>{transportClosed=true;}};
 const host={events:[],attach:(key,debuggee)=>{hosts.push({key,debuggee});host.events.push(JSON.stringify({type:'attached',...key}));},setShown:()=>{},command:line=>{hostCommands.push(line);return Promise.resolve();},take:()=>({events:host.events.splice(0),painting:[],pointers:[],closed:false}),close:async()=>{retired.push(true);}};
 const scope={__o3PrivateCdp:core,T3NativeDesktopHost:{create:async()=>host}};
 vm.createContext(scope);
 for(const file of ['qt-target-scope.js','qt-guest-input.js','qt-owned-inspector.js'])vm.runInContext(fs.readFileSync(process.argv[2]+'/'+file,'utf8'),scope);
 const state=scope.T3QtOwnedInspector.create({threadId:'thread',tabId:'tab',instance:'inspector',target:'owned',userAgent:'actual-agent'});
 return {state,sent,incoming,hostCommands,hosts,host,retired,scope};
}
(async()=>{
 const f=fixture();
 assert.equal(JSON.parse(f.state.drain()[0]).kind,'expected');assert.equal(f.sent[0].method,'Target.getTargetInfo');assert.equal(f.hosts.length,0);
 f.incoming.push(JSON.stringify({id:f.sent[0].id,result:{targetInfo:{targetId:'owned'}}}));f.state.drain();await tick();
 const proof=JSON.parse(f.state.drain()[0]);assert.equal(proof.kind,'proof');assert.equal(proof.viewTarget,proof.cdpTarget);assert.equal(f.hosts.length,0);
 const admitted={kind:'admitted',threadId:'thread',tabId:'tab',instance:'inspector',lease:'lease',agentTurn:true};
 assert.equal(f.state.accept({...admitted,instance:'stale'}),false);assert.equal(f.hosts.length,0);
 assert.equal(f.state.accept(admitted),true);await tick();assert.equal(f.hosts.length,1);assert.equal(JSON.parse(f.hostCommands[0]).type,'turn');assert.equal(JSON.parse(f.hostCommands[0]).active,true);assert.equal(f.hosts[0].debuggee.webContents.getUserAgent(),'actual-agent');
 const attached=JSON.parse(f.state.drain()[0]);assert.equal(attached.kind,'event');assert.equal(attached.lease,'lease');assert.equal(attached.event.type,'attached');
 const opaque='{"id":17,"method":"Runtime.evaluate","params":{"z":1,"a":{"unknown":2,"first":3}}}';
 const command={kind:'command',...admitted,command:{type:'cdp',threadId:'thread',tabId:'tab',message:opaque}};command.kind='command';
 assert.equal(f.state.accept({...command,lease:'old'}),false);
 assert.equal(f.state.accept(command),true);assert.equal(JSON.parse(f.hostCommands[1]).message,opaque);
 f.host.events.push(JSON.stringify({type:'cdp',threadId:'thread',tabId:'tab',message:opaque}));
 assert.equal(JSON.parse(f.state.drain()[0]).event.message,opaque);
 const pending=f.hosts[0].debuggee.debugger.sendCommand('Runtime.evaluate',{expression:'actual'}).catch(error=>error.message);await tick();
 f.state.close();assert.equal(await pending,'private_browser_closed');
 const closed=JSON.parse(f.state.drain()[0]);assert.equal(closed.kind,'retired');assert.equal(closed.code,'private_browser_released');assert.equal(closed.lease,'lease');
 assert.equal(f.state.accept(command),false);assert.equal(f.state.drain().length,0);assert.equal(f.hostCommands.length,2);assert.equal(f.retired.length,1);
 const mismatch=fixture();mismatch.state.drain();mismatch.incoming.push(JSON.stringify({id:999,result:{}}));const error=JSON.parse(mismatch.state.drain()[0]);assert.equal(error.kind,'retired');assert.equal(error.code,'private_browser_reply_mismatch');await tick();assert.equal(mismatch.hosts.length,0);
 console.log('QT_OWNED_INSPECTOR_BOUNDARY_PASS: expected/proof/admit ordering, stale identity/lease refusal, opaque command/event bytes, pending cancellation, late reply retirement');
})().catch(error=>{console.error(error);process.exitCode=1;});
