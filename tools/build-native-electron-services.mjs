// Build the actual pinned services for a Node main process, with native builtins.
import fs from 'node:fs';
import crypto from 'node:crypto';
import {createRequire,stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const [nativeRoot,dependencies,platformDependencies,output]=process.argv.slice(2);
const require=createRequire(dependencies+'/package.json'),platform=createRequire(platformDependencies+'/package.json');
if(require('effect/package.json').version!=='4.1.2'||require('rolldown/package.json').version!=='1.2.16'||platform('@effect/platform-node/package.json').version!=='4.1.2')throw Error('Canonical pinned dependencies required');
const manifest=JSON.parse(fs.readFileSync(nativeRoot+'/SOURCE.json','utf8'));
if(manifest.revision!=='4cb6945dc60ed03f567dc5daafd083e52308e82b')throw Error('Pinned native revision required');
const source=name=>{const text=fs.readFileSync(nativeRoot+'/'+name,'utf8');if(crypto.createHash('sha256').update(text).digest('hex')!==manifest.files[name])throw Error('Changed native input '+name);return text};
const modules=new Map();
const base=source('packages/contracts/src/baseSchemas.ts'),start=base.indexOf('// Hermes copies'),end=base.indexOf('export const DpopFailureReason');
if(start<0||end<start)throw Error('Native schema boundaries changed');
modules.set('native:base','import * as Effect from "effect/Effect";import * as Schema from "effect/Schema";import * as SchemaTransformation from "effect/SchemaTransformation";\n'+base.slice(start,end));
modules.set('native:contract',source('packages/contracts/src/desktopBrowser.ts').replace('"./baseSchemas.ts"','"native:base"'));
modules.set('native:relay',source('apps/desktop/src/preview/CdpRelay.ts'));
modules.set('native:input',source('apps/desktop/src/preview/GuestInput.ts'));
modules.set('native:host',source('apps/desktop/src/preview/DesktopBrowserHost.ts').replace('"@t3tools/contracts"','"native:contract"').replace('"./CdpRelay.ts"','"native:relay"').replace('"./GuestInput.ts"','"native:input"'));
modules.set('native:dialog',source('apps/desktop/src/electron/ElectronDialog.ts'));
modules.set('native:session',source('apps/desktop/src/preview/BrowserSession.ts').replace('"../electron/ElectronDialog.ts"','"native:dialog"'));
modules.set('native:entry',`
import * as Effect from 'effect/Effect';
import * as Layer from 'effect/Layer';
import * as Stream from 'effect/Stream';
import * as NodeCrypto from '@effect/platform-node/NodeCrypto';
import * as Session from 'native:session';
import * as Dialog from 'native:dialog';
import {make as makeHost} from 'native:host';
import {sendPageCommand} from 'native:input';
export async function create(boundary) {
 const dialog=boundary.showMessageBox?{...Dialog.make,showMessageBox:(options,owner)=>Effect.tryPromise({
  try:()=>boundary.showMessageBox(options,owner),
  catch:cause=>new Dialog.ElectronDialogShowMessageBoxError({type:options.type??null,titleLength:options.title?.length??null,messageLength:options.message.length,detailLength:options.detail?.length??null,buttonCount:options.buttons?.length??0,cause})
 })}:Dialog.make;
 const sessions=await Effect.runPromise(Session.make.pipe(Effect.provide(Layer.merge(NodeCrypto.layerDigest,Layer.succeed(Dialog.ElectronDialog,dialog)))));
 let host,stop,admit,fail,closed=false;
 const lifetime=new Promise(resolve=>stop=resolve),ready=new Promise((resolve,reject)=>{admit=resolve;fail=reject});
 const running=Effect.runPromise(Effect.scoped(Effect.gen(function*(){
  host=yield*makeHost;
  yield*Effect.forkChild(Stream.runForEach(host.events,line=>Effect.tryPromise(()=>Promise.resolve(boundary.event(line)))));
  yield*Effect.forkChild(Stream.runForEach(host.painting,event=>Effect.sync(()=>boundary.painting?.(event))));
  yield*Effect.forkChild(Stream.runForEach(host.pointers,event=>Effect.sync(()=>boundary.pointer?.(event))));
  admit();yield*Effect.promise(()=>lifetime);
 })));
 running.catch(()=>{stop();const error=Error('native_browser_service_failed');fail(error);if(!closed)boundary.failed?.(error)});await ready;
 return Object.freeze({
  getPartition:(scope,persistent,namespace)=>Effect.runPromise(sessions.getPartition(scope,persistent,namespace)),
  getSession:(scope,persistent,namespace)=>Effect.runPromise(sessions.getSession(scope,persistent,namespace)),
  clearCookies:partitions=>Effect.runPromise(sessions.clearCookies(partitions)),
  clearCache:partitions=>Effect.runPromise(sessions.clearCache(partitions)),
  attach:(key,debuggee)=>host.attach(key,debuggee),detach:key=>host.detach(key),setShown:(key,shown)=>host.setShown(key,shown),
  command:line=>closed?Promise.reject(Error('native_browser_closed')):Effect.runPromise(host.handleCommandLine(line)),
  humanInput:(contents,debuggerBoundary,method,params)=>closed?Promise.reject(Error('native_browser_closed')):sendPageCommand(contents,debuggerBoundary,method,params),
  placeDownload:(source,item)=>host.placeDownload(source,item),humanStartedDownload:source=>host.humanStartedDownload(source),
  close:async()=>{if(!closed){closed=true;stop()}await running}
 });
}
`);
const {rolldown}=await import(pathToFileURL(require.resolve('rolldown')).href);
const bundle=await rolldown({input:'native:entry',platform:'node',plugins:[{name:'pinned-native-node-services',resolveId(spec){if(modules.has(spec))return spec;if(spec==='electron'||spec.startsWith('node:'))return {id:spec,external:true};if(spec==='effect'||spec.startsWith('effect/'))return require.resolve(spec);if(spec.startsWith('@effect/'))return platform.resolve(spec);},load(id){if(modules.has(id))return stripTypeScriptTypes(modules.get(id),{mode:'strip'})}}]});
const generated=await bundle.generate({format:'cjs',codeSplitting:false});if(generated.output.length!==1)throw Error('Single native service module required');
const code='// T3 Code4cb6945dc60ed03f567dc5daafd083e52308e82b; native BrowserSession/Host.\n// Licenses: T3-Code-MIT.txt, Effect-MIT.txt; Electron runtime carries its official licenses.\n'+generated.output[0].code;
fs.writeFileSync(output+'/native-electron-services.cjs',code);
fs.writeFileSync(output+'/native-electron-services.sources.json',JSON.stringify({revision:manifest.revision,sources:manifest.files,effect:'4.1.2',platformNode:'4.1.2',rolldown:'1.2.16',node:process.versions.node,bytes:Buffer.byteLength(code),dependencyInputs:Object.fromEntries(Object.keys(generated.output[0].modules).filter(id=>id.startsWith(dependencies+'/')||id.startsWith(platformDependencies+'/')).map(id=>[id.replace(dependencies+'/','effect-and-bundler/').replace(platformDependencies+'/','node-platform/'),crypto.createHash('sha256').update(fs.readFileSync(id)).digest('hex')]))},null,2)+'\n');
await bundle.close();
