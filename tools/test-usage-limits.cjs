#!/usr/bin/env node
// Run the original pinned T3 test bodies against the Qt-adapted algorithms.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict'),cp=require('node:child_process'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..'),source=path.resolve(process.argv[2]||path.join(root,'../t3code-nightly'));
const manifest=JSON.parse(fs.readFileSync(path.join(root,'ui/js/NativeUsageLimits.sources.json'),'utf8'));
const generated=fs.readFileSync(path.join(root,'ui/js/NativeUsageLimits.js'),'utf8');
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');assert.equal(hash(generated),manifest.outputSha256);
const pinned=file=>cp.execFileSync('git',['-C',source,'show',manifest.commit+':'+file],{encoding:'utf8'});
for(const [file,digest] of Object.entries(manifest.files))assert.equal(hash(pinned(file)),digest);
const testFile='packages/shared/src/usageLimits.test.ts',original=pinned(testFile);
const setup=`const EnvironmentId={make:x=>x},ProviderDriverKind={make:x=>x},ProviderInstanceId={make:x=>x},UsageLimitSourceId={make:x=>x};
function subset(value,expected){for(const [k,v] of Object.entries(expected)){if(v&&typeof v==='object')subset(value[k],v);else assert.deepEqual(value[k],v);}}
function expect(value){const m={toBe:x=>assert.equal(value,x),toEqual:x=>assert.deepEqual(value,x),toStrictEqual:x=>assert.deepEqual(value,x),toBeNull:()=>assert.equal(value,null),toBeUndefined:()=>assert.equal(value,undefined),toBeDefined:()=>assert.notEqual(value,undefined),toBeTruthy:()=>assert.ok(value),toBeFalsy:()=>assert.ok(!value),toBeGreaterThan:x=>assert.ok(value>x),toBeGreaterThanOrEqual:x=>assert.ok(value>=x),toBeLessThan:x=>assert.ok(value<x),toBeLessThanOrEqual:x=>assert.ok(value<=x),toContain:x=>assert.ok(value.includes(x)),toHaveLength:x=>assert.equal(value.length,x),toBeCloseTo:(x,d=2)=>assert.ok(Math.abs(value-x)<Math.pow(10,-d)/2),toMatchObject:x=>subset(value,x)};m.not=Object.fromEntries(Object.entries(m).map(([k,f])=>[k,(...a)=>{let failed=false;try{f(...a)}catch{failed=true}assert.ok(failed,"not."+k)}]));return m;}
expect.soft=expect;
const structuredClone=v=>Array.isArray(v)?v.map(structuredClone):v&&typeof v==="object"?Object.fromEntries(Object.entries(v).map(([k,x])=>[k,structuredClone(x)])):v;
`;
let passed=0;const it=(name,fn)=>{try{fn();passed++;}catch(e){throw Error(name+': '+e.message,{cause:e});}};it.each=rows=>(name,fn)=>rows.forEach(row=>it(name,()=>fn(...(Array.isArray(row)?row:[row]))));
vm.runInNewContext(generated.replace(/^\.pragma library\n/,'')+setup+require('node:module').stripTypeScriptTypes(original.replace(/^import[\s\S]*?;\n/gm,''),{mode:'strip'}),{assert,describe:(_n,fn)=>fn(),it});
console.log('USAGE_LIMITS_PASS: '+passed+' unchanged native cases; generated output and source provenance verified');
