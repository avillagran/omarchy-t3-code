#!/usr/bin/env node
// Run the original pinned T3 nextThreadAnnouncement tests against ui/js/ThreadAnnouncement.js.
//   node tools/test-thread-announcement.cjs [T3_CHECKOUT]   (default ../t3code-nightly)
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict'),cp=require('node:child_process');
const root=path.resolve(__dirname,'..'),source=path.resolve(process.argv[2]||path.join(root,'../t3code-nightly'));
const pin=require('./upstream-pin.cjs').revision;
const original=cp.execFileSync('git',['-C',source,'show',pin+':apps/web/src/components/chat/threadActivityAnnouncement.test.ts'],{encoding:'utf8'});
const erased=require('node:module').stripTypeScriptTypes(original,{mode:'strip'});
// The helpers and the describe body, without imports; announcedRun is not ported (ot3 has no queued-run shell).
const helpers=erased.slice(erased.indexOf('const firstRun'),erased.indexOf('describe("nextThreadAnnouncement"'));
const body=erased.slice(erased.indexOf('describe("nextThreadAnnouncement"'));
const cases=body.split(/\n(?=  it\()/).slice(1).map(c=>c.replace(/\n\}\);\s*$/,''));
const port=fs.readFileSync(path.join(root,'ui/js/ThreadAnnouncement.js'),'utf8').replace(/^\.pragma library\n/,'');
let run=0,skipped=0;
for(const c of cases){
  const name=(c.match(/it\("([^"]+)"/)||[])[1]||'?';
  if(/announcedRun|isDetailBehindShell/.test(c)){skipped++;continue}
  const sandbox={RunId:{make:x=>x},expect:v=>({toEqual:e=>assert.deepEqual(v,e,name),toBe:e=>assert.equal(v,e,name)}),it:(n,f)=>f()};
  vm.createContext(sandbox);vm.runInContext(port+'\n'+helpers+'\n'+c+'\n',sandbox,{filename:name});run++;
}
assert(run>=8,'expected the original cases, ran '+run);
console.log(`Thread announcement: ${run} original T3 cases pass (${skipped} skipped: announcedRun is not ported)`);
