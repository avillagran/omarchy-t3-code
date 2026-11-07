#!/usr/bin/env node
// Execute whole pinned project contracts and their transitive real schema modules.
import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';import {createRequire,stripTypeScriptTypes} from 'node:module';import {pathToFileURL} from 'node:url';
const [checkout,dependencies,output]=process.argv.slice(2);
if(!output)throw Error('usage: generate-file-metadata-project-wire.mjs CHECKOUT DEPS OUTPUT');
const revision='32a0531748c44b25072d7495ce0b9bd7e9450a87';
const require=createRequire(path.join(dependencies,'package.json'));
if(require('effect/package.json').version!=='4.1.2')throw Error('Effect 4.1.2 required');
const runtime=path.join(output,'project-wire-runtime'),hashes={};fs.mkdirSync(runtime,{recursive:true});
const staged=new Map();
function stage(filename){
 if(staged.has(filename))return staged.get(filename);
 if(!filename.startsWith('packages/contracts/src/')||!filename.endsWith('.ts'))throw Error('Unexpected contract import '+filename);
 const target=path.join(runtime,path.relative('packages/contracts/src',filename).replace(/\.ts$/,'.mjs')),uri=pathToFileURL(target).href;
 staged.set(filename,uri);fs.mkdirSync(path.dirname(target),{recursive:true});
 const original=execFileSync('git',['-C',checkout,'show',revision+':'+filename],{encoding:'utf8'});hashes[filename]=crypto.createHash('sha256').update(original).digest('hex');
 let source=stripTypeScriptTypes(original);
 source=source.replace(/\bfrom\s*(["'])([^"']+)\1/g,(match,quote,specifier)=>{
  const url=specifier.startsWith('.')?stage(path.posix.normalize(path.posix.join(path.posix.dirname(filename),specifier))):pathToFileURL(require.resolve(specifier)).href;
  return 'from '+JSON.stringify(url);
 });
 fs.writeFileSync(target,source);return uri;
}
const project=await import(stage('packages/contracts/src/project.ts'));
const Schema=await import(pathToFileURL(require.resolve('effect/Schema')).href);
const cases=[];
const validRead={relativePath:'src/a.ts',contents:'fixture',byteLength:7,truncated:false};
for(const input of [null,{},validRead,{...validRead,relativePath:'\ufeff src/a.ts \u00a0'},{...validRead,relativePath:' '},{...validRead,contents:null},{...validRead,byteLength:2**53},{...validRead,byteLength:1.5},{...validRead,byteLength:-1},{...validRead,byteLength:null},{...validRead,truncated:null},{...validRead,truncated:1},{...validRead,exists:false},{...validRead,exists:null},{...validRead,exists:true,extra:'ignored'},{relativePath:'absent',contents:'',byteLength:0,truncated:false,exists:false}])cases.push({schema:'read',input});
const validEntries={entries:[{path:'src/a.ts',kind:'file'},{path:'directory',kind:'directory',ignored:true}],truncated:false};
for(const input of [null,{},validEntries,{entries:[],truncated:true},{entries:[{path:'\ufeff src/a.ts \u00a0',kind:'file',ignored:false,extra:1}],truncated:false},{entries:[{path:' ',kind:'file'}],truncated:false},{entries:[{path:'src/a.ts',kind:'other'}],truncated:false},{entries:[null],truncated:false},{entries:[{path:'src/a.ts',kind:'file',ignored:null}],truncated:false},{entries:[{path:'src/a.ts',kind:'file',ignored:1}],truncated:false},{entries:[],truncated:null},{entries:[]}])for(const schema of['search','list'])cases.push({schema,input});
const schemas={read:project.ProjectReadFileResult,search:project.ProjectSearchEntriesResult,list:project.ProjectListEntriesResult};
const observations=cases.map(test=>{try{return{...test,valid:true,result:Schema.decodeUnknownSync(schemas[test.schema])(test.input)}}catch{return{...test,valid:false}}});
fs.writeFileSync(path.join(output,'project-wire-fixtures.json'),JSON.stringify({revision,effectVersion:'4.1.2',hashes,cases:observations},null,2)+'\n');
console.log(JSON.stringify({projectWireCases:observations.length,sourceModules:staged.size}));
