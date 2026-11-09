#!/usr/bin/env node
// Actual nightly fallback selection and provider enablement; private oracle only.
import fs from 'node:fs';
import crypto from 'node:crypto';
import vm from 'node:vm';
import {stripTypeScriptTypes,createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
const [source,output]=process.argv.slice(2),revision='4cb6945dc60ed03f567dc5daafd083e52308e82b';
const metadata=JSON.parse(fs.readFileSync(source+'/SOURCE.json','utf8'));
const hash=v=>crypto.createHash('sha256').update(v).digest('hex');
if(metadata.revision!==revision)throw Error('Wrong native revision');
for(const [file,digest] of Object.entries(metadata.files))if(hash(fs.readFileSync(source+'/'+file))!==digest)throw Error('Changed native source '+file);
const require=createRequire(source+'/package.json');
if(JSON.parse(fs.readFileSync(require.resolve('effect/package.json'),'utf8')).version!=='4.1.2')throw Error('Wrong Effect version');
const native=await import(pathToFileURL(source+'/packages/shared/src/usageLimits.ts').href);
const settings=await import(pathToFileURL(source+'/packages/contracts/src/settings.ts').href);
const model=await import(pathToFileURL(source+'/packages/contracts/src/providerInstance.ts').href);
const text=fs.readFileSync(source+'/packages/contracts/src/settings.ts','utf8');
const branding=text.slice(text.indexOf('const DEFAULT_DISABLED_PROVIDER_DRIVERS:'),text.indexOf('export const isUnconfiguredDefaultInstanceEnabled'));
const context=vm.createContext({ProviderDriverKind:model.ProviderDriverKind,ProviderInstanceId:model.ProviderInstanceId});
// Resolve nominal ID constructors from the actual canonical contracts, including
// their validation. These arrays are literal native constants, not runtime input.
vm.runInContext(stripTypeScriptTypes(branding,{mode:'strip'}),context);
const constants=vm.runInContext('({disabledDrivers:[...DEFAULT_DISABLED_PROVIDER_DRIVERS],enabledInstances:[...DEFAULT_ENABLED_DEFAULT_INSTANCES]})',context);
const now=Date.parse('2026-11-07T12:00:00Z'),cases=[];
const selection=(instanceId,model)=>({instanceId,model,options:[]});
const provider=(instanceId,driver,windows)=>({instanceId,driver,enabled:true,installed:true,availability:'available',auth:{status:'authenticated'},usageLimits:{checkedAt:'2026-11-07T12:00:00Z',windows}});
const window=(id='session',resetsAt='2026-11-07T13:00:00Z',usedPercent=100)=>({id,label:id,kind:'session',usedPercent,...(resetsAt===null?{}:{resetsAt})});
const add=input=>cases.push({input,result:native.resolveNewThreadModelSelection(input)});
for(const driver of ['codex','claudeAgent','cursor','antigravity','opencode','future'])for(const modelName of ['auto','gpt-test','claude-test','gemini-test','opencode-go/model','composer'])for(const shape of ['session','cursor-api','cursor-auto','cursor-both','gpt','3p','go','other']){
 const windows=shape==='cursor-api'?[window('apiPercentUsed')]:shape==='cursor-auto'?[window('autoPercentUsed')]:shape==='cursor-both'?[window('apiPercentUsed'),window('autoPercentUsed')]:[window(shape==='gpt'?'gpt-pool':shape==='3p'?'3p-pool':shape==='go'?'go_pool':shape)];
 add({defaultSelection:selection('primary',modelName),fallbackSelection:selection('claudeAgent','fallback'),providers:[provider('primary',driver,windows),provider('claudeAgent','claudeAgent',[])],now});
}
for(const reset of [null,'invalid','2026-11-07T11:59:59Z','2026-11-07T12:00:00Z','2026-11-07T12:00:01Z'])for(const used of [0,99,100,101])for(const gate of ['allowed','disabled','uninstalled','unavailable','unauthenticated','fallback-exhausted','missing-fallback','missing-primary','explicit','no-default','no-fallback','project-disabled','global-disabled','config-disabled','project-enable','unconfigured-custom']){
 const primary=provider('primary','codex',[window('session',reset,used)]),fallback=provider('claudeAgent','claudeAgent',[]);
 let input={defaultSelection:selection('primary','model'),fallbackSelection:selection('claudeAgent','fallback'),providers:[primary,fallback],now};
 if(gate==='disabled')fallback.enabled=false;
 if(gate==='uninstalled')fallback.installed=false;
 if(gate==='unavailable')fallback.availability='unavailable';
 if(gate==='unauthenticated')fallback.auth.status='unauthenticated';
 if(gate==='fallback-exhausted')fallback.usageLimits.windows=[window()];
 if(gate==='missing-fallback')input.providers=[primary];
 if(gate==='missing-primary')input.providers=[fallback];
 if(gate==='explicit')input.explicitSelection=selection('primary','explicit');
 if(gate==='no-default')input.defaultSelection=null;
 if(gate==='no-fallback')input.fallbackSelection=null;
 if(['project-disabled','global-disabled','config-disabled','project-enable','unconfigured-custom'].includes(gate)){
  input.settings={providerInstances:{},projectSettingsOverrides:{}};input.projectId='project';
  if(gate==='project-disabled')input.settings.projectSettingsOverrides.project={providerInstances:{claudeAgent:{enabled:false}}};
  if(gate==='global-disabled')input.settings.providerInstances.claudeAgent={driver:'claudeAgent',enabled:false};
  if(gate==='config-disabled')input.settings.providerInstances.claudeAgent={driver:'claudeAgent',config:{enabled:false}};
  if(gate==='project-enable'){input.settings.providerInstances.claudeAgent={driver:'claudeAgent',enabled:false};input.settings.projectSettingsOverrides.project={providerInstances:{claudeAgent:{enabled:true}}};}
  if(gate==='unconfigured-custom'){fallback.instanceId='custom';input.fallbackSelection=selection('custom','fallback');}
 }
 add(input);
}
const files=['packages/shared/src/usageLimits.ts','packages/contracts/src/settings.ts','packages/contracts/src/model.ts','packages/contracts/src/server.ts','packages/contracts/src/providerInstance.ts'];
fs.writeFileSync(output,JSON.stringify({revision,effectVersion:'4.1.2',sources:Object.fromEntries(files.map(file=>[file,metadata.files[file]])),constants,cases})+'\n');
console.log('NATIVE_NEW_THREAD_MODEL_PASS '+cases.length+' complete decisions');
