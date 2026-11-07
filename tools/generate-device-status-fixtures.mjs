#!/usr/bin/env node
// Run the actual nightly 4275 host-status schema, including per-platform discovery failures.
import fs from 'node:fs';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {createRequire,stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const [checkout,contracts,output]=process.argv.slice(2);
if(!checkout||!contracts||!output)throw Error('usage: generate-device-status-fixtures.mjs CHECKOUT CONTRACTS OUTPUT');
const revision='6577c6b6185167c0e502fa01f9e8e8f2dee6ff25';
const paths=['packages/contracts/src/device.ts','packages/contracts/src/baseSchemas.ts'];
const sources=paths.map(path=>execFileSync('git',['-C',checkout,'show',`${revision}:${path}`],{encoding:'utf8'}));
const require=createRequire(contracts+'/package.json');
if(require('effect/package.json').version!=='4.1.2')throw Error('Effect 4.1.2 required');
let code='';const modules={};
for(const name of ['Effect','Option','Schema','SchemaTransformation']){
 const url=pathToFileURL(require.resolve('effect/'+name)).href;
 modules[name]=await import(url);code+='import * as '+name+' from '+JSON.stringify(url)+';\n';
}
function between(source,start,end){const a=source.indexOf(start),b=source.indexOf(end,a);if(a<0||b<a)throw Error('Pinned source boundary missing');return source.slice(a,b);}
code+=stripTypeScriptTypes(between(sources[1],'// Hermes copies','export const NonNegativeInt'))+'\n';
for(const name of ['DevicePlatform','DeviceHostId','DeviceHostStatus'])code+=stripTypeScriptTypes(between(sources[0],'export const '+name+' =','export type '+name+' ='))+'\n';
code+=stripTypeScriptTypes(between(sources[0],'export const DeviceHostStatuses =','const deviceServiceStateFields'));
const native=await import('data:text/javascript;base64,'+Buffer.from(code).toString('base64'));
const decode=modules.Schema.decodeUnknownOption(native.DeviceHostStatuses);
const inputs=[undefined,[],['ios'],['android'],['ios','android'],['android','ios','android'],Array(513).fill('ios'),null,'ios',{},1,false,[null],['web'],[1],['ios',false],['IOS'],[' android ']];
const cases=inputs.map(incompletePlatforms=>{
 const input={local:{status:'ready',androidDiscoveryIncomplete:true,...incompletePlatforms===undefined?{}:{incompletePlatforms}}};
 const decoded=decode(input);return {input,expected:modules.Option.isSome(decoded)?decoded.value:null};
});
fs.writeFileSync(output,JSON.stringify({revision,effect:'4.1.2',sources:paths.map((path,i)=>({path,sha256:crypto.createHash('sha256').update(sources[i]).digest('hex')})),cases},null,2)+'\n');
console.log(`${cases.length} actual native host-status cases`);
