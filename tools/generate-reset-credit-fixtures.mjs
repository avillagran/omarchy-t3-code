#!/usr/bin/env node
// Private native oracle: actual module exports and the complete UI summary function.
import fs from 'node:fs';
import crypto from 'node:crypto';
import {pathToFileURL} from 'node:url';
import {stripTypeScriptTypes} from 'node:module';
import vm from 'node:vm';
import {createRequire} from 'node:module';
const [source,output]=process.argv.slice(2),revision='4cb6945dc60ed03f567dc5daafd083e52308e82b';
const provenance=JSON.parse(fs.readFileSync(source+'/SOURCE.json','utf8'));
if(provenance.revision!==revision)throw Error('Wrong native revision');
const hash=value=>crypto.createHash('sha256').update(value).digest('hex');
for(const [file,digest] of Object.entries(provenance.files))if(hash(fs.readFileSync(source+'/'+file))!==digest)throw Error('Changed source '+file);
const require=createRequire(source+'/package.json');
if(JSON.parse(fs.readFileSync(require.resolve('effect/package.json'),'utf8')).version!=='4.1.2')throw Error('Native Effect version mismatch');
const {PROVIDER_DISPLAY_NAMES}=await import(pathToFileURL(source+'/packages/contracts/src/model.ts').href);
const native=await import(pathToFileURL(source+'/packages/shared/src/usageLimits.ts').href);
const file='apps/web/src/components/usage/UsageLimits.tsx',text=fs.readFileSync(source+'/'+file,'utf8');
if(text.indexOf('export function resetCreditsSummary(')<0 || text.indexOf('\nexport function ResetCredits(')<0)throw Error('Native summary boundaries changed');
const body=text.slice(text.indexOf('export function resetCreditsSummary('),text.indexOf('\nexport function ResetCredits('));
const context=vm.createContext({formatDuration:native.formatDuration});vm.runInContext(stripTypeScriptTypes(body,{mode:'strip'}).replace(/^export /gm,''),context);
const summaries=[];
for(const count of [0,1,2,100])for(const expiry of [undefined,'2026-11-07T11:00:00Z','2026-11-07T12:00:00Z','2026-11-07T13:31:00Z','2026-11-10T13:31:00Z','invalid'])for(const compact of [false,true]){
 const credits={availableCount:count,...(expiry===undefined?{}:{nextExpiresAt:expiry})},now=Date.parse('2026-11-07T12:00:00Z');
 summaries.push({credits,now,compact,result:context.resetCreditsSummary(credits,now,compact)});
}
const accounts=[];
for(const order of [false,true])for(const displayName of ['Codex','Personal',undefined])for(const plan of ['new-plan',undefined])for(const hubCredits of [false,true])for(const workspace of ['one','two']){
 const limits={checkedAt:'2026-11-07T12:00:00Z',windows:[{id:'session',kind:'session',label:'Session',usedPercent:100}],resetCredits:{availableCount:1}};
 const provider=(id,time)=>({instanceId:id,driver:'codex',enabled:true,installed:true,displayName,auth:{status:'authenticated',email:'same@example.test',workspaceId:'one',label:'old-plan'},usageLimits:{...limits,checkedAt:time}});
 const hub={id:'account.json',driver:'codex',email:'same@example.test',workspaceId:workspace,plan,usageLimits:{...limits,checkedAt:'2026-11-07T13:00:00Z',...(hubCredits?{resetCredits:{availableCount:2,nextCreditId:'hub-credit'}}:{resetCredits:undefined})}};
 let presentations=[['a',{entry:{target:{label:'Host A'}},serverConfig:{providers:[provider('first','2026-11-07T11:00:00Z')],usageLimitSources:[{id:'source',label:'Usage hub',accounts:[hub]}]}}],['b',{entry:{target:{label:'Host B'}},serverConfig:{providers:[provider('second','2026-11-07T12:00:00Z')],usageLimitSources:[]}}]];
 if(order)presentations.reverse();
 accounts.push({presentations,result:native.collectLimitAccounts(new Map(presentations))});
}
fs.writeFileSync(output,JSON.stringify({revision,effectVersion:'4.1.2',sources:{[file]:provenance.files[file],'packages/shared/src/usageLimits.ts':provenance.files['packages/shared/src/usageLimits.ts'],'packages/contracts/src/model.ts':provenance.files['packages/contracts/src/model.ts']},providerDisplayNames:PROVIDER_DISPLAY_NAMES,summaries,accounts},null,2)+'\n');
console.log('NATIVE_RESET_CREDIT_PASS '+summaries.length+' summaries '+accounts.length+' account presentations');
