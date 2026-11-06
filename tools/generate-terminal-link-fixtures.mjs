#!/usr/bin/env node
// Execute upstream test bodies and retain their native inputs/outputs as the Qt oracle.
import fs from 'node:fs';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {execFileSync} from 'node:child_process';
import {stripTypeScriptTypes} from 'node:module';
const [checkout,output]=process.argv.slice(2);
if(!checkout||!output)throw Error('usage: generate-terminal-link-fixtures.mjs T3_CHECKOUT OUTPUT');
const revision='c55db3e9c72fdf584f4b54192fb39beb88667b59';
const files=['apps/web/src/terminal-links.ts','apps/web/src/terminal-links.test.ts'];
const sources=files.map(file=>execFileSync('git',['-C',checkout,'show',`${revision}:${file}`],{encoding:'utf8'}));
const erase=source=>stripTypeScriptTypes(source).replace(/^import[^;]+;\n/gm,'').replace(/^export /gm,'');
const original=Function('isMacPlatform',erase(sources[0])+'\nreturn {extractTerminalLinks,isTerminalUrl,collectWrappedTerminalLinkLine,isTerminalLinkActivation};')(platform=>/mac|iphone|ipad|ipod/i.test(platform));
const pathsFile='packages/shared/src/path.ts',fileLinksFile='packages/shared/src/fileLinks.ts';
const paths=execFileSync('git',['-C',checkout,'show',`${revision}:${pathsFile}`],{encoding:'utf8'});
const fileLinks=execFileSync('git',['-C',checkout,'show',`${revision}:${fileLinksFile}`],{encoding:'utf8'});
original.resolvePathLinkTarget=Function(erase(paths)+'\n'+erase(fileLinks)+'\nreturn resolvePathLinkTarget;')();
const cases=[];
const wrap=name=>(...args)=>{const expected=original[name](...args);cases.push({method:name,args,expected});return expected;};
const wrapped=(number,getLine)=>{const expected=original.collectWrappedTerminalLinkLine(number,getLine);const lines=[];for(let index=0;index<100;index++){const line=getLine(index);if(!line)break;lines.push({text:line.translateToString(false),isWrapped:!!line.isWrapped});}cases.push({method:'collectWrappedTerminalLinkLine',args:[number,lines],expected});return expected;};
const it=(name,body)=>body();it.each=rows=>(name,body)=>rows.forEach(row=>body(...(Array.isArray(row)?row:[row])));
const describe=(name,body)=>body();
const expect=value=>({toEqual:expected=>assert.deepEqual(value,expected),toBe:expected=>assert.equal(value,expected),toBeNull:()=>assert.equal(value,null)});
Function('describe','it','expect','extractTerminalLinks','isTerminalUrl','collectWrappedTerminalLinkLine','resolvePathLinkTarget',erase(sources[1]))(describe,it,expect,wrap('extractTerminalLinks'),wrap('isTerminalUrl'),wrapped,wrap('resolvePathLinkTarget'));
// PCRE defaults differ from JavaScript for whitespace and decimal digits.
for(const separator of ['\t','\u0085','\u00a0','\u1680','\u180e','\u2003','\u2028','\u202f','\u3000','\ufeff'])wrap('extractTerminalLinks')('https://example.com/a'+separator+'src/main.ts:12');
for(const line of ['src/main.ts:١٢','src/main.ts:12:٣','GET /api?x=1\u0085next','see "src/main.ts":١٢'])wrap('extractTerminalLinks')(line);
for(const [raw,cwd] of [['~/src/main.ts:12:7','/home/dev/project'],['~\\src\\main.ts:12','C:\\Users\\Jane\\project'],['../src/main.ts:12','/workspace/project'],['C:/Users/Jane/main.ts:2:3','C:\\Users\\Jane\\project'],['src/main.ts:١٢','/workspace/project']])wrap('resolvePathLinkTarget')(raw,cwd);
for(const platform of ['Linux x86_64','MacIntel','iPad','Win32',''])for(const ctrlKey of [false,true])for(const metaKey of [false,true]){const args=[{ctrlKey,metaKey},platform];cases.push({method:'isTerminalLinkActivation',args,expected:original.isTerminalLinkActivation(...args)});}
const coreFile=path.join(path.dirname(output),'core.mjs');
const extraSources=[{file:pathsFile,sha256:crypto.createHash('sha256').update(paths).digest('hex')},{file:fileLinksFile,sha256:crypto.createHash('sha256').update(fileLinks).digest('hex')}],viewportCases=[],grids=[],gridIds=new Map();
if(!fs.existsSync(coreFile))throw Error('Generate the pinned core modules with generate-terminal-fixtures.mjs in the output directory first');
{
 globalThis.fetch=async url=>new Response(fs.readFileSync(new URL(url)));
 const {GhosttyTerminalCore}=await import(pathToFileURL(coreFile).href);
 const surfaceFile='apps/web/src/terminal/ghostty/surface.ts',testFile='apps/web/src/terminal/ghostty/surface.test.ts';
 const source=execFileSync('git',['-C',checkout,'show',`${revision}:${surfaceFile}`],{encoding:'utf8'});
 const tests=execFileSync('git',['-C',checkout,'show',`${revision}:${testFile}`],{encoding:'utf8'});
 extraSources.push({file:surfaceFile,sha256:crypto.createHash('sha256').update(source).digest('hex')},{file:testFile,sha256:crypto.createHash('sha256').update(tests).digest('hex')});
 const block=name=>{const at=source.search(new RegExp('(?:export )?function '+name+'\\('));return source.slice(at,source.indexOf('\n}',at)+2);};
 const helpers=['terminalCellText','terminalRowText','terminalColumnOffset','terminalColumnAtOffset','terminalLinkAtPositionWithRange'].map(block).join('\n');
 const range=Function('collectWrappedTerminalLinkLine','extractTerminalLinks',erase('const GHOSTTY_CELL_WIDE={narrow:0,wide:1,spacerTail:2,spacerHead:3};\n'+helpers)+'\nreturn terminalLinkAtPositionWithRange;')(original.collectWrappedTerminalLinkLine,original.extractTerminalLinks);
 const checked=(rows,row,column)=>{const expected=range(rows,row,column);
  const compact=rows.map(value=>({isWrapContinuation:value.isWrapContinuation,wrapsToNext:value.wrapsToNext,cells:value.cells.map(({text,wide})=>({text,wide}))}));
  const key=JSON.stringify(compact);let id=gridIds.get(key);if(id===undefined){id=grids.length;gridIds.set(key,id);grids.push(compact);}
  cases.push({method:'terminalLinkAtPositionWithRange',grid:true,args:[id,row,column],expected});return expected;
 };
 const start=tests.indexOf('describe("terminalLinkAtPositionWithRange"');const end=tests.indexOf('\ndescribe(',start+1);
 const cellStart=tests.indexOf('const cell =');const cellEnd=tests.indexOf('\ndescribe(',cellStart);
 const group=tests.slice(cellStart,cellEnd)+'\n'+tests.slice(start,end);
 const pending=[];const run=(name,body)=>pending.push(Promise.resolve().then(body));run.each=rows=>(name,body)=>rows.forEach(row=>run(name,()=>body(...(Array.isArray(row)?row:[row]))));
 Function('describe','it','expect','GhosttyTerminalCore','terminalLinkAtPositionWithRange',erase(group))((name,body)=>body(),run,expect,GhosttyTerminalCore,checked);
 await Promise.all(pending);
 for(const spec of [
  {cols:23,rows:3,vt:'https://example.com/日本 next'},
  {cols:20,rows:3,vt:'🙂https://t3.codes next'},
  {cols:16,rows:3,vt:'see "C:\\Users\\Jane Doe\\index.ts":12:7'},
  {cols:16,rows:3,vt:'https://example.com/'+ 'a'.repeat(80)},
  {cols:16,rows:3,vt:'src/日本/main.ts:3:1 next'}
 ]){
  const core=await GhosttyTerminalCore.create(spec.cols,spec.rows,8,16,{foreground:{r:255,g:255,b:255},background:{r:0,g:0,b:0},cursor:{r:255,g:255,b:255}},()=>{});
  try{core.write(spec.vt);const rows=core.snapshot().rowData,points=[];for(let row=0;row<spec.rows;row++)for(let column=0;column<spec.cols;column++)points.push({row,column,expected:range(rows,row,column)});viewportCases.push({...spec,points});}finally{core.dispose();}
 }

}
fs.writeFileSync(output,JSON.stringify({revision,sources:[...files.map((file,i)=>({file,sha256:crypto.createHash('sha256').update(sources[i]).digest('hex')})),...extraSources],cases,viewportCases,grids},null,2)+'\n');
console.log(`${cases.length} native terminal-link oracle cases`);
