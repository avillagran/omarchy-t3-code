#!/usr/bin/env node
// Execute the complete pinned sort module; preserve its original label clocks for Qt.
import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';import {stripTypeScriptTypes} from 'node:module';import {pathToFileURL} from 'node:url';
const [checkout,output]=process.argv.slice(2);if(!output)throw Error('usage: generate-thread-activity-fixtures.mjs CHECKOUT OUTPUT');
const revision='4cb6945dc60ed03f567dc5daafd083e52308e82b',filename='packages/client-runtime/src/state/threadSort.ts';
fs.mkdirSync(output,{recursive:true});
const source=execFileSync('git',['-C',checkout,'show',revision+':'+filename],{encoding:'utf8'});
const runtime=path.join(output,'thread-sort.mjs');fs.writeFileSync(runtime,stripTypeScriptTypes(source));
const native=await import(pathToFileURL(runtime).href);
const stamps=['2026-11-07T00:00:00.000Z','2026-11-07T01:00:00.000Z','2026-11-07T02:00:00.000Z','2026-11-07T03:00:00.000Z'];
const inputs=[];
for(let mask=0;mask<16;mask++)inputs.push({latestUserMessageAt:mask&1?stamps[0]:null,latestRun:{requestedAt:mask&2?stamps[1]:null,startedAt:mask&4?stamps[2]:null,completedAt:mask&8?stamps[3]:null},updatedAt:'2026-11-07T23:00:00.000Z'});
for(let winner=0;winner<4;winner++){
 const values=stamps.map((value,index)=>stamps[(index+3-winner)%4]);
 inputs.push({latestUserMessageAt:values[0],latestRun:{requestedAt:values[1],startedAt:values[2],completedAt:values[3]},updatedAt:stamps[0]});
}
for(const value of [null,'','invalid','2026-11-07T01:00:00.000Z','2026-11-07T02:00:00+01:00','1970-01-01T00:00:00.000Z','1969-12-31T23:00:00.000Z']){
 inputs.push({latestUserMessageAt:value,latestRun:null,updatedAt:stamps[3]});
 inputs.push({latestUserMessageAt:null,latestRun:{requestedAt:value,startedAt:'invalid',completedAt:null},updatedAt:stamps[3]});
}
inputs.push({latestUserMessageAt:'2026-11-07T01:00:00.000Z',latestRun:{requestedAt:'2026-11-07T02:00:00+01:00',startedAt:'2026-11-07T01:00:00Z',completedAt:'2026-11-07T01:00:00.000Z'},updatedAt:stamps[3]});
const cases=[];
for(const input of inputs)for(const settledAt of [null,'invalid','2026-11-06T00:00:00.000Z']){
 const thread={...input,settledAt};cases.push({input:thread,activity:native.resolveThreadActivityTimestamp(thread),settled:native.resolveSettledThreadTimestamp(thread)});
}
const dateDiagnostics=['2026-11-07T00:00:00.000Z','2026-02-30T00:00:00.000Z','2026-13-01T00:00:00.000Z','2026-01-01','2026-01-01T00:00:00','2026-01-01T00:00:00+03:30','0000-01-01T00:00:00.000Z','+010000-01-01T00:00:00.000Z','1969-12-31T23:59:59.999Z','invalid','',null].map(value=>({value,result:native.toSortableTimestamp(value??undefined)}));
function extract(name){const start=source.indexOf('export function '+name+'(');if(start<0)throw Error(name);const end=source.indexOf('\n}',start);return source.slice(start,end+2);}
const helper='.pragma library\n// Native nightly 4301 label clocks; sorting and reply clocks remain separate.\n'+stripTypeScriptTypes(['toSortableTimestamp','resolveSettledThreadTimestamp','resolveThreadActivityTimestamp'].map(extract).join('\n')).replace(/^export /gm,'')+'\n';
const hashes={[filename]:crypto.createHash('sha256').update(source).digest('hex')};
for(const name of ['apps/web/src/components/sidebar/SidebarThreadRow.tsx','packages/client-runtime/src/state/models.ts'])hashes[name]=crypto.createHash('sha256').update(execFileSync('git',['-C',checkout,'show',revision+':'+name])).digest('hex');
fs.writeFileSync(path.join(output,'NativeThreadActivity.js'),helper);fs.writeFileSync(path.join(output,'fixtures.json'),JSON.stringify({revision,hashes,cases,dateDiagnostics},null,2)+'\n');
fs.writeFileSync(path.join(output,'SOURCE.json'),JSON.stringify({revision,hashes,execution:'Whole native threadSort.ts imported after TypeScript erasure; native Date.parse',qtAdaptations:['TypeScript erasure','ES module exports become Qt library exports'],outputSha256:crypto.createHash('sha256').update(helper).digest('hex')},null,2)+'\n');
console.log(JSON.stringify({threadClockCases:cases.length,dateDiagnostics:dateDiagnostics.length}));
