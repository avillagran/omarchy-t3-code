#!/usr/bin/env node
// Original cleanup resolver fixture generation; never used at application runtime.
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),crypto=require('node:crypto'),vm=require('node:vm');
const root=path.resolve(__dirname,'..'),source=path.resolve(process.argv[2]||path.join(root,'../t3code-nightly'));
const commit=require('./upstream-pin.cjs').revision,file='packages/shared/src/projectSettings.ts';
const text=fs.readFileSync(path.join(source,file),'utf8');
if(cp.execFileSync('git',['-C',source,'rev-parse','HEAD'],{encoding:'utf8'}).trim()!==commit||cp.execFileSync('git',['-C',source,'show',commit+':'+file],{encoding:'utf8'})!==text)throw Error('Original resolver input changed');
const body=require('node:module').stripTypeScriptTypes(text.slice(text.indexOf('export function resolveWorktreeCleanup(')),{mode:'strip'}).replace(/^export /gm,'');
// The Rust adapter receives already-folded settings. Resolve only the original
// cleanup policy here; full project scope folding is checked separately.
const sandbox={resolveProjectSettings:settings=>({settings})};vm.createContext(sandbox);vm.runInContext(body+'\nthis.resolve=resolveWorktreeCleanup;',sandbox);
const cases=[];
for(const keep of ['any-local-files','uncommitted-changes','tracked-changes'])for(const mode of [null,'off','custom']){
 const settings={storageCleanup:{worktreeKeepWhen:keep,worktreeAfterDays:8,worktreeOnMerge:true,worktreeOnDelete:false,worktreeUnchanged:true},worktreeCleanup:mode===null?null:mode==='off'?{mode}:{mode,rules:{worktreeKeepWhen:'tracked-changes',worktreeAfterDays:3650,worktreeOnMerge:false,worktreeOnDelete:true,worktreeUnchanged:false}}};
 cases.push({settings,expected:sandbox.resolve(settings,null)});
}
fs.writeFileSync(path.join(root,'data/storage-rule-cases.json'),JSON.stringify({commit,source:file,sourceSha256:crypto.createHash('sha256').update(text).digest('hex'),adaptation:'Dependency adapter receives already-folded settings; original resolveWorktreeCleanup body unchanged after TypeScript erasure.',cases},null,2)+'\n');
console.log('Original storage resolver fixtures: '+cases.length);
