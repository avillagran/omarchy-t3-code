#!/usr/bin/env node
// Run the original pinned T3 test bodies against the Qt-adapted algorithms.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict'),cp=require('node:child_process'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..'),source=path.resolve(process.argv[2]||path.join(root,'../t3code-nightly'));
const manifest=JSON.parse(fs.readFileSync(path.join(root,'ui/js/NativeUsageLimits.sources.json'),'utf8'));
const generated=fs.readFileSync(path.join(root,'ui/js/NativeUsageLimits.js'),'utf8');
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');assert.equal(hash(generated),manifest.outputSha256);
for(const [file,digest] of Object.entries(manifest.files))assert.equal(hash(fs.readFileSync(path.join(source,file))),digest);
const testFile='packages/shared/src/usageLimits.test.ts',original=fs.readFileSync(path.join(source,testFile),'utf8');assert.equal(original,cp.execFileSync('git',['-C',source,'show',manifest.commit+':'+testFile],{encoding:'utf8'}));
const setup=`const EnvironmentId={make:x=>x},ProviderDriverKind={make:x=>x},ProviderInstanceId={make:x=>x},UsageLimitSourceId={make:x=>x};
function subset(value,expected){for(const [k,v] of Object.entries(expected)){if(v&&typeof v==='object')subset(value[k],v);else assert.deepEqual(value[k],v);}}
function expect(value){return {toBe:x=>assert.equal(value,x),toEqual:x=>assert.deepEqual(value,x),toBeNull:()=>assert.equal(value,null),toHaveLength:x=>assert.equal(value.length,x),toBeCloseTo:(x,d=2)=>assert.ok(Math.abs(value-x)<Math.pow(10,-d)/2),toMatchObject:x=>subset(value,x)};}
`;
let passed=0;const it=(name,fn)=>{try{fn();passed++;}catch(e){throw Error(name+': '+e.message,{cause:e});}};it.each=rows=>(name,fn)=>rows.forEach(row=>it(name,()=>fn(...row)));
vm.runInNewContext(generated.replace(/^\.pragma library\n/,'')+setup+require('node:module').stripTypeScriptTypes(original.replace(/^import[\s\S]*?;\n/gm,''),{mode:'strip'}),{assert,describe:(_n,fn)=>fn(),it});
console.log('USAGE_LIMITS_PASS: '+passed+' unchanged native cases; generated output and source provenance verified');
