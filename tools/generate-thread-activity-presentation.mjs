#!/usr/bin/env node
// Execute whole native presentation and its official typed fixture with real DateTime.
import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';import{execFileSync}from'node:child_process';import{createRequire,stripTypeScriptTypes}from'node:module';import{pathToFileURL}from'node:url';
const[checkout,dependencies,output]=process.argv.slice(2);if(!output)throw Error('usage: generate-thread-activity-presentation.mjs CHECKOUT DEPS OUTPUT');
const revision='4cb6945dc60ed03f567dc5daafd083e52308e82b',require=createRequire(path.join(dependencies,'package.json'));
if(require('effect/package.json').version!=='4.1.2')throw Error('Effect4.1.2 required');
const runtime=path.join(output,'presentation-runtime'),sources={},staged=new Map();fs.mkdirSync(runtime,{recursive:true});
const isolated=path.join(runtime,'isolated.mjs');
fs.writeFileSync(isolated,'export const threadPullRequestsOf=()=>[];export const formatSubagentDisplayTitle=value=>value;\n');
function stage(filename){
 if(staged.has(filename))return staged.get(filename);
 if(!filename.startsWith('packages/')||!filename.endsWith('.ts'))throw Error('Unreviewed source path '+filename);
 const target=path.join(runtime,filename.replace(/\.ts$/,'.mjs')),uri=pathToFileURL(target).href;staged.set(filename,uri);fs.mkdirSync(path.dirname(target),{recursive:true});
 const original=execFileSync('git',['-C',checkout,'show',revision+':'+filename],{encoding:'utf8'});sources[filename]=crypto.createHash('sha256').update(original).digest('hex');
 const text=stripTypeScriptTypes(original).replace(/(^|\n)([ \t]*(?:import|export)[ \t]+(?:\*(?:[ \t]+as[ \t]+\w+)?|\{[^}]*\})\s+from\s*)(["'])([^"']+)\3/g,(match,newline,prefix,quote,specifier)=>{
  let url;
  if(filename==='packages/client-runtime/src/state/models.ts'&&['../threadPullRequests.ts','./subagentDisplay.ts'].includes(specifier))url=pathToFileURL(isolated).href;
  else if(specifier.startsWith('.'))url=stage(path.posix.normalize(path.posix.join(path.posix.dirname(filename),specifier)));
  else if(specifier==='@t3tools/contracts')url=stage('packages/contracts/src/index.ts');
  else if(specifier.startsWith('@t3tools/shared/'))url=stage('packages/shared/src/'+specifier.slice('@t3tools/shared/'.length)+'.ts');
  else if(specifier==='effect'||specifier.startsWith('effect/'))url=pathToFileURL(require.resolve(specifier)).href;
  else throw Error('Unreviewed dependency '+filename+': '+specifier);
  return newline+prefix+JSON.stringify(url);
 });fs.writeFileSync(target,text);return uri;
}
const models=await import(stage('packages/client-runtime/src/state/models.ts'));
const fixtures=await import(stage('packages/client-runtime/src/state/orchestrationV2TestFixtures.ts'));
const sort=await import(stage('packages/client-runtime/src/state/threadSort.ts'));
const DateTime=await import(pathToFileURL(require.resolve('effect/DateTime')).href);
const Contract=await import(stage('packages/contracts/src/index.ts'));
function wire(value){if(DateTime.isDateTime(value))return DateTime.formatIso(value);if(Array.isArray(value))return value.map(wire);if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).filter(([,v])=>v!==undefined).map(([k,v])=>[k,wire(v)]));return value;}
const stamp=hour=>DateTime.makeUnsafe(`2026-11-07T0${hour}:00:00.000Z`),cases=[];
for(const status of['idle','running','queued','completed','interrupted','failed','cancelled','rolled_back'])for(const hasRun of[false,true])for(const completion of['omitted','null','complete'])for(const scenario of['fallback','request','user']){
 const thread={...fixtures.v2ThreadShell,status,latestRunId:hasRun?Contract.RunId.make('run'):null,createdAt:stamp(0),updatedAt:stamp(5),latestRunRequestedAt:scenario==='fallback'?null:stamp(1),latestRunStartedAt:null,latestUserMessageAt:scenario==='user'?stamp(2):null};
 if(completion==='omitted')delete thread.latestRunCompletedAt;else thread.latestRunCompletedAt=completion==='null'?null:stamp(3);
 const presented=models.presentThreadShell(Contract.EnvironmentId.make('env'),thread);
 cases.push({input:wire(thread),presented:{latestUserMessageAt:presented.latestUserMessageAt,latestRun:presented.latestRun,updatedAt:presented.updatedAt,settledAt:presented.settledAt},activity:sort.resolveThreadActivityTimestamp(presented),settled:sort.resolveSettledThreadTimestamp(presented)});
}
fs.writeFileSync(path.join(output,'presentation-fixtures.json'),JSON.stringify({revision,effectVersion:'4.1.2',sources,isolatedDependencies:{threadPullRequestsOf:'Empty result; official fixture has no pull requests. No PR projection claim.',formatSubagentDisplayTitle:'Not exercised; official fixture has null subagent relationship. No title projection claim.'},cases},null,2)+'\n');
console.log(JSON.stringify({presentationCases:cases.length,actualSourceModules:staged.size}));
