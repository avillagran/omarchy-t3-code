#!/usr/bin/env node
// Execute the actual native preferred-editor resolver and verify catalog order.
const fs=require('node:fs'),assert=require('node:assert/strict'),{stripTypeScriptTypes}=require('node:module'),{execFileSync}=require('node:child_process');
const checkout=process.argv[2];if(!checkout)throw Error('usage: test-terminal-editor-preference.cjs T3_CHECKOUT');
const revision='32a0531748c44b25072d7495ce0b9bd7e9450a87';
const read=file=>execFileSync('git',['-C',checkout,'show',revision+':'+file],{encoding:'utf8'});
const contract=read('packages/contracts/src/editor.ts');
const start=contract.indexOf('export const EDITORS = '),end=contract.indexOf('\nexport const EditorId',start);
const editors=Function(stripTypeScriptTypes(contract.slice(start,end)).replace('export ','')+'\nreturn EDITORS')();
const catalog=JSON.parse(fs.readFileSync('vendor/t3-protocol/src/native_editors.json'));
assert.deepEqual(catalog,editors.map(({id,label})=>({id,label})));
const source=read('apps/web/src/editorPreferences.ts');
const at=source.indexOf('function resolveAndPersistPreferredEditor('),block=source.slice(at,source.indexOf('\n}',at)+2);
let stored=null,writes=[];
const resolve=Function('EDITORS','getLocalStorageItem','setLocalStorageItem','EditorId',stripTypeScriptTypes('const LAST_EDITOR_KEY="t3code:last-editor";\n'+block)+'\nreturn resolveAndPersistPreferredEditor')(editors,()=>stored,(key,value)=>writes.push(value),{});
let count=0;
for(const available of [[],catalog.map(e=>e.id),catalog.map(e=>e.id).reverse(),['zed','vscode'],['not-installed'],...catalog.map(e=>[e.id])])for(const previous of [null,'missing',...catalog.map(e=>e.id)]){
 stored=previous;writes=[];const actual=resolve(available);
 const offered=catalog.filter(e=>available.includes(e.id));const expected=offered.find(e=>e.id===previous)?.id||offered[0]?.id||null;
 assert.equal(actual,expected);assert.deepEqual(writes,expected&&previous!==expected?[expected]:[]);count++;
}
console.log('TERMINAL_EDITOR_PREFERENCE_PASS: '+count+' native resolver/catalog-order cases at '+revision);
