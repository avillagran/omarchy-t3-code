import fs from 'node:fs';
import crypto from 'node:crypto';
import {createRequire,stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const [nativeRoot,dependencies,output]=process.argv.slice(2);
const require=createRequire(dependencies+'/package.json');
const local=createRequire(import.meta.url);
const nodePath=local('./build-node-path-boundary.cjs').build();
if(require('effect/package.json').version!=='4.1.2')throw Error('Effect4.1.2 required');
if(require('rolldown/package.json').version!=='1.2.16')throw Error('Rolldown1.2.16 required');
const {rolldown}=await import(pathToFileURL(require.resolve('rolldown')).href);
const hashes={
 'apps/desktop/src/preview/DesktopBrowserHost.ts':'5665eceec4b4b2d0e3b02d1c00888a4f32d6b8f994b299713a1eb7d7388d3c08',
 'apps/desktop/src/preview/GuestInput.ts':'59ec3f63327719dd7fb24d6b73bbc5a4578eaba8790c83736273e53c913d2f94',
 'apps/desktop/src/preview/CdpRelay.ts':'9c7aa34d4b110777c896f412be1f4eff6c60f08d39fe9b3bae2ad69f22f45754',
 'packages/contracts/src/baseSchemas.ts':'edd4d2c73cc79ce5e50bffd35b9c861fe091de23a1ac792db864d3b71b09a7af',
 'packages/contracts/src/desktopBrowser.ts':'6c2247f812d70b0f0e7856ac38c1413a2d45afbc50b137e14e19288d2f4cb2ad'
};
const source=name=>{const text=fs.readFileSync(nativeRoot+'/'+name,'utf8');if(crypto.createHash('sha256').update(text).digest('hex')!==hashes[name])throw Error('Native source mismatch '+name);return text;};
const modules=new Map();
const base=source('packages/contracts/src/baseSchemas.ts');
const start=base.indexOf('// Hermes copies'),end=base.indexOf('export const DpopFailureReason');
if(start<0||end<start)throw Error('Pinned base schema boundaries missing');
modules.set('native:base','import * as Effect from "effect/Effect";import * as Schema from "effect/Schema";import * as SchemaTransformation from "effect/SchemaTransformation";\n'+base.slice(start,end));
modules.set('native:contract',source('packages/contracts/src/desktopBrowser.ts').replace('"./baseSchemas.ts"','"native:base"'));
modules.set('native:CdpRelay',source('apps/desktop/src/preview/CdpRelay.ts'));
modules.set('native:GuestInput',source('apps/desktop/src/preview/GuestInput.ts'));
modules.set('native:host',source('apps/desktop/src/preview/DesktopBrowserHost.ts').replace('"@t3tools/contracts"','"native:contract"').replace('"./CdpRelay.ts"','"native:CdpRelay"').replace('"./GuestInput.ts"','"native:GuestInput"'));
modules.set('native:path',nodePath.code);
modules.set('native:entry',`
import * as Effect from 'effect/Effect';
import * as Stream from 'effect/Stream';
import {make} from 'native:host';
export async function create() {
 const events=[],painting=[],pointers=[],attached=new Map();
 const wireLimit=256*1024*1024,recordLimit=4096;
 let queued=0,closed=false,failure=null,stop,admit,fail,host;
 const lifetime=new Promise(resolve=>stop=resolve),ready=new Promise((resolve,reject)=>{admit=resolve;fail=reject;});
 const retire=reason=>{if(closed)return;closed=true;failure=reason;for(const key of attached.values())host?.detach(key);attached.clear();events.length=0;painting.length=0;pointers.length=0;queued=0;stop();};
 const record=(queue,value)=>{if(closed)return;if(queue.length>=recordLimit){retire('private_browser_output_limit');return;}queue.push(value);};
 const running=Effect.runPromise(Effect.scoped(Effect.gen(function*(){
  host=yield*make;
  yield*Effect.forkChild(Stream.runForEach(host.events,line=>Effect.sync(()=>{
   if(closed)return;
   if(line.byteLength>wireLimit||queued+line.byteLength>wireLimit||events.length>=recordLimit){retire('private_browser_output_limit');return;}
   queued+=line.byteLength;events.push(new TextDecoder().decode(line));
  })));
  yield*Effect.forkChild(Stream.runForEach(host.painting,event=>Effect.sync(()=>record(painting,event))));
  yield*Effect.forkChild(Stream.runForEach(host.pointers,event=>Effect.sync(()=>record(pointers,event))));
  admit(host);
  yield*Effect.promise(()=>lifetime);
 })));
 running.catch(()=>{retire('private_browser_runtime_failed');fail(Error('private_browser_runtime_failed'));});
 await ready;
 return Object.freeze({
  attach:(key,debuggee)=>{if(closed)throw Error('private_browser_closed');attached.set(JSON.stringify([key.threadId,key.tabId]),key);host.attach(key,debuggee);},
  detach:key=>{attached.delete(JSON.stringify([key.threadId,key.tabId]));host.detach(key);},
  setShown:(key,shown)=>{if(!closed)host.setShown(key,shown);},
  command:line=>closed?Promise.reject(Error('private_browser_closed')):Effect.runPromise(host.handleCommandLine(line)),
  placeDownload:(source,item)=>{if(closed)throw Error('private_browser_closed');return host.placeDownload(source,item);},
  humanStartedDownload:source=>{if(closed)throw Error('private_browser_closed');return host.humanStartedDownload(source);},
  take:()=>{const result={closed,failure,events:events.splice(0),painting:painting.splice(0),pointers:pointers.splice(0)};queued=0;return result;},
  close:async()=>{retire(null);await running;}
 });
}
`);
const bundle=await rolldown({input:'native:entry',platform:'browser',plugins:[{name:'pinned-native-source',resolveId(spec){if(modules.has(spec))return spec;if(spec==='node:path')return 'native:path';if(spec.startsWith('node:'))throw Error('Unexpected Node dependency '+spec);if(spec.startsWith('effect/'))return require.resolve(spec);},load(id){if(modules.has(id))return stripTypeScriptTypes(modules.get(id),{mode:'strip'});}}]});
const generated=await bundle.generate({format:'iife',name:'T3NativeDesktopHost',codeSplitting:false});
if(generated.output.length!==1)throw Error('Unexpected bundle shape');
const banner='// Pinned T3 Code32a0531748c44b25072d7495ce0b9bd7e9450a87; generated by tools/build-native-browser-host.mjs.\n// Licenses: licenses/T3-Code-MIT.txt, licenses/Effect-MIT.txt, licenses/Node-path-MIT.txt.\n';
fs.writeFileSync(output+'/native-browser-host.js',banner+generated.output[0].code);
fs.writeFileSync(output+'/native-browser-host.sources.json',JSON.stringify({revision:'32a0531748c44b25072d7495ce0b9bd7e9450a87',sources:hashes,effect:'4.1.2',rolldown:'1.2.16',node:process.versions.node,nodePathSourceSha256:nodePath.sourceSha256,modules:Object.keys(generated.output[0].modules).length,dependencyInputs:Object.fromEntries(Object.keys(generated.output[0].modules).filter(id=>id.startsWith(dependencies+'/')).map(id=>[id.slice(dependencies.length+1),crypto.createHash('sha256').update(fs.readFileSync(id)).digest('hex')])),bytes:Buffer.byteLength(banner+generated.output[0].code)},null,2)+'\n');
await bundle.close();
