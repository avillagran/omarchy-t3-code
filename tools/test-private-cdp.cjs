#!/usr/bin/env node
// Pure protocol boundary tests: no browser, filesystem fixtures, or deletion.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(require('node:path').join(__dirname,'../ui/browser/private-cdp-bridge.js'),'utf8');
function fixture() {
 const sent=[],frontend=[];
 const context=vm.createContext({InspectorFrontendHost:{sendMessageToBackend:raw=>sent.push(JSON.parse(raw))},DevToolsAPI:{dispatchMessage:raw=>frontend.push(JSON.parse(raw))}});
 vm.runInContext(source,context);
 return {context,sent,frontend,bridge:context.__o3PrivateCdp,api:context.DevToolsAPI,host:context.InspectorFrontendHost};
}
const request=(id,method='Runtime.evaluate')=>JSON.stringify({id,method,params:{expression:'fixture'}});
{
 const f=fixture();
 f.host.sendMessageToBackend(request(1));f.bridge.send(request(1));
 assert.notEqual(f.sent[0].id,f.sent[1].id);
 f.api.dispatchMessage(JSON.stringify({id:f.sent[1].id,result:{value:'private'}}));
 f.api.dispatchMessage(JSON.stringify({id:f.sent[0].id,result:{value:'frontend'}}));
 assert.equal(f.frontend.length,1);assert.equal(f.frontend[0].id,1);assert.equal(f.frontend[0].result.value,'frontend');
 const out=f.bridge.drain();assert.equal(out.messages.length,1);assert.equal(JSON.parse(out.messages[0]).result.value,'private');assert.equal(out.closed,false);
 f.api.dispatchMessage('{"method":"Runtime.fixture","params":{"value":1}}');
 assert.equal(f.frontend.length,2);assert.equal(f.bridge.drain().messages.length,1);
}
{
 const f=fixture();f.bridge.send(request(4));
 const raw=JSON.stringify({id:f.sent[0].id,result:{value:'chunks'}});
 f.api.dispatchMessageChunk(raw.slice(0,7),raw.length);f.api.dispatchMessageChunk(raw.slice(7),0);
 assert.equal(JSON.parse(f.bridge.drain().messages[0]).id,4);assert.equal(f.frontend.length,0);
 f.bridge.close();assert.equal(f.bridge.drain().closed,true);assert.throws(()=>f.bridge.send(request(2)),/closed/);
}
{
 const f=fixture();assert.throws(()=>f.api.dispatchMessageChunk('x',17*1024*1024),/chunk_limit/);assert.equal(f.bridge.drain().closed,true);
}
{
 const f=fixture();assert.throws(()=>f.api.dispatchMessage('{"id":999,"result":{}}'),/unknown_reply/);assert.equal(f.bridge.drain().closed,true);
}
{
 const f=fixture();for(let i=0;i<4096;i++) f.bridge.send(request(i));
 assert.throws(()=>f.bridge.send(request(4096)),/request_limit/);assert.equal(f.bridge.drain().closed,true);
}
{
 const f=fixture();assert.throws(()=>f.api.dispatchMessage('null'),/invalid_message/);assert.equal(f.bridge.drain().closed,true);
}
{
 const f=fixture();assert.throws(()=>f.bridge.send('{'));assert.equal(f.bridge.drain().closed,true);
}
console.log('PRIVATE_CDP_PASS: collision-safe owners, out-of-order replies, events, chunks, bounds, release');
