#!/usr/bin/env node
// Compare the Qt adapter's helpers with the pinned native surface source.
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const {execFileSync}=require('node:child_process'),{stripTypeScriptTypes}=require('node:module');
const checkout=process.argv[2];if(!checkout)throw Error('usage: test-terminal-scrollbar.cjs T3_CHECKOUT');
const revision=JSON.parse(fs.readFileSync('ui/js/NativeTerminalLinks.sources.json')).revision;
const source=execFileSync('git',['-C',checkout,'show',revision+':apps/web/src/terminal/ghostty/surface.ts'],{encoding:'utf8'});
function block(name){const at=source.search(new RegExp('function '+name+'\\('));assert(at>=0);return source.slice(at,source.indexOf('\n}',at)+2)}
const original=Function(stripTypeScriptTypes('const MIN_SCROLLBAR_THUMB_HEIGHT=18;\n'+block('terminalScrollbarGeometry')+'\n'+block('terminalScrollbarOffsetAtPointer'))+'\nreturn {terminalScrollbarGeometry,terminalScrollbarOffsetAtPointer}')();
const context=vm.createContext({});vm.runInContext(fs.readFileSync('ui/js/NativeTerminalLinks.js','utf8').replace(/^\.pragma.*$/gm,''),context);const actual=context.create(()=>[]);
const clone=value=>JSON.parse(JSON.stringify(value));let count=0;
for(const state of [{total:0,len:0,offset:0},{total:30,len:30,offset:0},{total:100,len:20,offset:0},{total:100,len:20,offset:40},{total:100,len:20,offset:80},{total:10000,len:1,offset:9999},{total:10,len:20,offset:-20},{total:100,len:20,offset:500}])for(const height of [-1,0,4,18,100,601]){
 assert.deepEqual(clone(actual.terminalScrollbarGeometry(state,height)),original.terminalScrollbarGeometry(state,height));count++;
 for(const y of [-20,0,height/2,height,height+20])for(const offset of [0,9,18]){assert.equal(actual.terminalScrollbarOffsetAtPointer(state,height,y,offset),original.terminalScrollbarOffsetAtPointer(state,height,y,offset));count++}
}
console.log('TERMINAL_SCROLLBAR_PASS: '+count+' pinned native geometry/pointer cases');
