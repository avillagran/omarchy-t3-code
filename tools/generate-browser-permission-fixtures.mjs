#!/usr/bin/env node
// Execute the pinned permission handlers and global prompt effect. Only the
// Electron dialog/window platform boundaries are supplied by the fixture.
import fs from 'node:fs';
import crypto from 'node:crypto';
import {stripTypeScriptTypes,createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
const [source,dependencies,output] = process.argv.slice(2);
const revision='4cb6945dc60ed03f567dc5daafd083e52308e82b';
const manifest=JSON.parse(fs.readFileSync(source+'/SOURCE.json','utf8'));
if(manifest.revision!==revision)throw Error('Pinned source required');
const name='apps/desktop/src/preview/BrowserSession.ts',text=fs.readFileSync(source+'/'+name,'utf8');
if(crypto.createHash('sha256').update(text).digest('hex')!==manifest.files[name])throw Error('Changed native input');
const require=createRequire(dependencies+'/package.json');
if(require('effect/package.json').version!=='4.1.2')throw Error('Pinned Effect 4.1.2 required');
const Effect=await import(pathToFileURL(require.resolve('effect/Effect')).href);
function section(start,end){const a=text.indexOf(start),b=text.indexOf(end,a);if(a<0||b<a)throw Error('Native extraction changed');return stripTypeScriptTypes(text.slice(a,b));}
const allowed=section('const ALLOWED_PREVIEW_PERMISSIONS:','const NEVER_EXTERNAL_PROTOCOLS:');
const helpers=section('const MAX_PROMPT_URL_LENGTH =','export class BrowserSessionPartitionDerivationError');
const prompt=section('let permissionPromptOpen = false;','  /** Electron launches external protocols');
const handlers=section('const clipboardReadDecisions =','          browserSession.webRequest.onBeforeSendHeaders(');
const dialogs=[];
const promptPermission=new Function('Effect','electronDialog','previewHostWindow','runFork',prompt+'\nreturn promptPermission;')(Effect,{
 showMessageBox:props=>Effect.callback(resume=>{dialogs.push({props,answer:response=>resume(Effect.succeed({response})),fail:()=>resume(Effect.fail('owned-dialog-failure'))});})
},()=>undefined,Effect.runFork);
const make=new Function('browserSession','promptPermission','confirmOpenExternal',allowed+helpers+handlers);
function session(){let request,check;make({setPermissionRequestHandler:f=>request=f,setPermissionCheckHandler:f=>check=f},promptPermission,(_requester,_url,callback)=>callback(false));return {request,check};}
const flush=()=>new Promise(resolve=>setImmediate(resolve));
const a=session(),b=session();
const policy=[];
for(const permission of ['clipboard-sanitized-write','geolocation','fullscreen','pointerLock','loopback-network','notifications','local-fonts','media','camera','microphone','unknown']){
 const row={permission};a.request({},permission,answer=>row.request=answer,{});row.check=a.check({},permission,'https://example.test');policy.push(row);
}
const trace=[];
async function request(s,label,url){const row={label,url};trace.push(row);s.request({},'clipboard-read',answer=>row.answer=answer,{requestingUrl:url});await flush();return row;}
await request(a,'first','https://Example.test/path');
await request(b,'busy-other-session','https://busy.test/path');
if(dialogs.length!==1)throw Error('Native singleflight changed');
dialogs[0].answer(1);await flush();
await request(a,'remembered-block','https://example.test:443/other');
await request(b,'retry-after-busy','https://busy.test/path');
if(dialogs.length!==2)throw Error('Native busy refusal was remembered');
dialogs[1].fail();await flush();
await request(b,'retry-after-dialog-failure','https://busy.test/path');
if(dialogs.length!==3)throw Error('Native dialog failure was remembered');
dialogs[2].answer(0);await flush();
await request(b,'remembered-allow','https://busy.test/other');
await request(a,'invalid-origin','data:text/plain,owned');
const checks={blocked:a.check({},'clipboard-read','https://example.test'),allowed:b.check({},'clipboard-read','https://busy.test'),otherSession:a.check({},'clipboard-read','https://busy.test'),unknown:a.check({},'clipboard-read','https://unknown.test')};
fs.writeFileSync(output,JSON.stringify({revision,effectVersion:'4.1.2',sources:manifest.files,policy,trace,checks,prompts:dialogs.map(({props})=>props)},null,2)+'\n');
console.error('NATIVE_BROWSER_PERMISSIONS_PASS: '+policy.length+' policy cases, '+trace.length+' actual handler steps, '+dialogs.length+' actual prompt effects');
