#!/usr/bin/env node
// Execute pinned producer functions; fixtures retain compact inputs and full-output hashes.
import fs from 'node:fs';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {stripTypeScriptTypes} from 'node:module';
const [checkout,output]=process.argv.slice(2);
if(!checkout||!output)throw Error('usage: generate-terminal-context-fixtures.mjs T3_CHECKOUT OUTPUT');
const revision='c55db3e9c72fdf584f4b54192fb39beb88667b59';
const paths=['apps/web/src/lib/composerContextRecords.ts','apps/web/src/lib/terminalContext.ts','packages/shared/src/composerContextReferences.ts'];
const sources=paths.map(path=>execFileSync('git',['-C',checkout,'show',`${revision}:${path}`],{encoding:'utf8'}));
const modules=sources.map(source=>stripTypeScriptTypes(source));
function block(source,name){const re=new RegExp(`(?:export )?function ${name}\\(`);const at=source.search(re);if(at<0)throw Error(name);const end=source.indexOf('\n}',at);return source.slice(at,end+2);}
let code='const COMPOSER_CONTEXT_LABEL_MAX_CHARS=200,COMPOSER_CONTEXT_TERMINAL_TEXT_MAX_CHARS=64000,CONTEXT_ID_PATTERN=/^[a-z0-9_-]{1,128}$/i,TRUNCATION_MARKER="\\n… truncated …";\n';
for(const name of ['fnv1a64','toComposerContextId','toKindScopedComposerContextId','sanitizeComposerContextLabel'])code+=block(modules[2],name)+'\n';
for(const name of ['normalizeTerminalContextText','formatTerminalContextRange','formatTerminalContextLabel'])code+=block(modules[1],name)+'\n';
for(const name of ['clampContextText','terminalContextRecord'])code+=block(modules[0],name)+'\n';
code=stripTypeScriptTypes(code);
const native=await import('data:text/javascript;base64,'+Buffer.from(code).toString('base64'));
const cases=[];
const descriptors=[{prefix:'\r\n\n',repeat:0,suffix:'hello\r\nworld\n\n'},...Array.from({length:15},(_,i)=>({prefix:'',repeat:63975+i,suffix:'😀'+ 'tail'.repeat(12)})),{prefix:'',repeat:63998,suffix:'😀'},{prefix:'',repeat:63999,suffix:'😀'},{prefix:'',repeat:64000,suffix:''},{prefix:'',repeat:64001,suffix:''}];
for(const descriptor of descriptors){
 const text=descriptor.prefix+'a'.repeat(descriptor.repeat)+descriptor.suffix;
 const selection={id:'fixture',threadId:'thread',terminalId:'one',terminalLabel:'Terminal 1',lineStart:2,lineEnd:50,text,createdAt:'2026-11-06T00:00:00Z'};
 const record=native.terminalContextRecord(selection);
 cases.push({input:descriptor,expected:{length:record.text.length,md5:crypto.createHash('md5').update(record.text).digest('hex'),lineStart:record.lineStart,lineEnd:record.lineEnd,label:record.label,contextId:record.contextId}});
}
fs.writeFileSync(output,JSON.stringify({revision,sources:paths.map((path,i)=>({path,sha256:crypto.createHash('sha256').update(sources[i]).digest('hex')})),cases},null,2)+'\n');
console.log(`${cases.length} native terminal producer cases`);
