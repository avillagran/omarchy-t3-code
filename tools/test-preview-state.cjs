#!/usr/bin/env node
// Execute the original pinned T3 tests against the Qt-adapted reducers.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict'),cp=require('node:child_process'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..'),source=path.resolve(process.argv[2]||path.join(root,'../t3code-nightly'));
const provenance=JSON.parse(fs.readFileSync(path.join(root,'ui/js/NativePreviewState.sources.json'),'utf8'));
const generated=fs.readFileSync(path.join(root,'ui/js/NativePreviewState.js'),'utf8');
assert.equal(crypto.createHash('sha256').update(generated).digest('hex'),provenance.outputSha256);
const testFile='apps/web/src/previewStateStore.test.ts',original=fs.readFileSync(path.join(source,testFile),'utf8');
assert.equal(original,cp.execFileSync('git',['-C',source,'show',provenance.commit+':'+testFile],{encoding:'utf8'}));
for(const [file,digest] of Object.entries(provenance.files))assert.equal(crypto.createHash('sha256').update(fs.readFileSync(path.join(source,file))).digest('hex'),digest);
let before=()=>{},passed=0;
// Only framework assertions are adapted; each original test body is unchanged.
const setup=`
const store=createPreviewStore();
const {__testing,applyPreviewDesktopState,applyPreviewServerEvent:applyPreviewServerEventImpl,applyPreviewServerSnapshot,beginPreviewSessionClose,cancelPreviewSessionClose,readThreadPreviewState,reconcilePreviewServerSessions,rememberPreviewUrl,resetPreviewStateForTests,setActivePreviewTab,updatePreviewServerSnapshot}=store;
const ThreadId={make:x=>x},scopeThreadRef=(environmentId,threadId)=>({environmentId,threadId}),scopedThreadKey=ref=>JSON.stringify([ref.environmentId,ref.threadId]);
const previewStateAtom=key=>key,appAtomRegistry={subscribe:(key,fn)=>{const [environmentId,threadId]=JSON.parse(key);return store.subscribe({environmentId,threadId},fn);}};
function expect(value){return {toBe:expected=>assert.equal(value,expected),toEqual:expected=>assert.deepEqual(value,expected),toContain:expected=>assert.ok(value.includes(expected)),toBeNull:()=>assert.equal(value,null),toBeUndefined:()=>assert.equal(value,undefined),toBeLessThanOrEqual:expected=>assert.ok(value<=expected),not:{toBe:expected=>assert.notEqual(value,expected)}};}
`;
const test=require('node:module').stripTypeScriptTypes(original.replace(/^import[\s\S]*?;\n/gm,''),{mode:'strip'});
const sandbox={assert,beforeEach:fn=>before=fn,describe:(_name,fn)=>fn(),it:(name,fn)=>{before();try{fn();passed++;}catch(e){throw Error(name+': '+e.message,{cause:e});}}};
vm.runInNewContext(generated.replace(/^\.pragma library\n/,'')+setup+test,sandbox);
console.log('PREVIEW_STATE_PASS: '+passed+' unchanged native tests against generated Qt reducers; source/output provenance verified');
