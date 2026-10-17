#!/usr/bin/env node
// Build-only extraction of T3's preview reducers. Runtime remains native Qt JS.
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),crypto=require('node:crypto'),vm=require('node:vm');
const root=path.resolve(__dirname,'..'),source=path.resolve(process.argv[2]||path.join(root,'../t3code-nightly'));
const pin='5a4e9980533705774dc3f45b35cacf76c3d78dbc',inputs={};
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
if(cp.execFileSync('git',['-C',source,'rev-parse','HEAD'],{encoding:'utf8'}).trim()!==pin)throw Error('Wrong native reference pin');
function read(file){const s=fs.readFileSync(path.join(source,file),'utf8');if(cp.execFileSync('git',['-C',source,'show',pin+':'+file],{encoding:'utf8'})!==s)throw Error('Dirty native input '+file);inputs[file]=hash(s);return s;}
const original=read('apps/web/src/previewStateStore.ts'),constants=read('apps/web/src/components/preview/previewConstants.ts');
const limit=constants.match(/export const PREVIEW_RECENT_URL_LIMIT = (\d+);/)[1];
function between(start,end){const a=original.indexOf(start),b=original.indexOf(end,a);if(a<0||b<0)throw Error('Native extraction boundary changed');return original.slice(a,b);}
let extracted=between('const EMPTY_THREAD_PREVIEW_STATE:', 'const emptyPreviewStateAtom')+
 between('const dedupeRecentUrls =','export function useThreadPreviewState')+
 between('export function applyPreviewServerEvent(', '/**\n * Runs `action`')+
 between('export function rememberPreviewUrl(', 'export function isPreviewSupportedInRuntime(')+
 between('export function clearThreadPreviewState(', 'export function resetPreviewStateForTests(');
extracted=require('node:module').stripTypeScriptTypes(extracted,{mode:'strip'}).replace(/^export /gm,'').replace(/\.toSorted\(/g,'.slice().sort(').replace(/\.at\(-1\)/g,'.slice(-1)[0]');
// Two native object-rest declarations need an own-key copy on Qt's JS engine.
for(const [value,key,rest] of [['current.sessions','tabId','sessions'],['current.desktopByTabId','tabId','desktopByTabId']]){
 const pattern=new RegExp('const \\{ \\[tabId\\]: _'+(rest==='sessions'?'closed':'desktop')+', \\.\\.\\.'+rest+' \\} = '+value.replace('.','\\.')+';');
 if(!pattern.test(extracted))throw Error('Native object-rest declaration changed');
 extracted=extracted.replace(pattern,'const '+rest+' = Object.assign({}, '+value+'); delete '+rest+'['+key+'];');
}
// Lexically bounded object spread conversion. All other reducer syntax stays
// verbatim after type erasure. Reject unknown lexical forms rather than guessing.
function quotedEnd(s,i){const q=s[i];for(let j=i+1;j<s.length;j++){if(s[j]==='\\'){j++;continue;}if(s[j]===q)return j+1;}throw Error('Unclosed string');}
function parts(s){const out=[];let start=0,depth=0;for(let i=0;i<s.length;i++){const c=s[i];if(c==='"'||c==="'"){i=quotedEnd(s,i)-1;continue;}if(c==='`')throw Error('Unexpected template in spread');if('([{'.includes(c))depth++;if(')]}'.includes(c))depth--;if(c===','&&depth===0){out.push(s.slice(start,i));start=i+1;}}out.push(s.slice(start));return out;}
function qtSpreads(s){let out='';for(let i=0;i<s.length;){if(s[i]==='"'||s[i]==="'"){const e=quotedEnd(s,i);out+=s.slice(i,e);i=e;continue;}if(s.startsWith('//',i)){let e=s.indexOf('\n',i);if(e<0)e=s.length;out+=s.slice(i,e);i=e;continue;}if(s.startsWith('/*',i)){const e=s.indexOf('*/',i)+2;if(e<2)throw Error('Unclosed comment');out+=s.slice(i,e);i=e;continue;}if(s[i]!=='{'){out+=s[i++];continue;}let j=i+1,depth=1;for(;j<s.length&&depth;j++){if(s[j]==='"'||s[j]==="'"){j=quotedEnd(s,j)-1;continue;}if(s.startsWith('//',j)){const e=s.indexOf('\n',j);j=e<0?s.length:e;continue;}if(s.startsWith('/*',j)){j=s.indexOf('*/',j)+1;continue;}if(s[j]==='{')depth++;if(s[j]==='}')depth--;}if(depth)throw Error('Unclosed block');const inner=qtSpreads(s.slice(i+1,j-1));const chunks=parts(inner).map(x=>x.trim()).filter(Boolean);if(chunks.some(x=>x.startsWith('...'))){const args=['{}'];let fields=[];for(const chunk of chunks){if(chunk.startsWith('...')){if(fields.length){args.push('{'+fields.join(',')+'}');fields=[];}args.push(chunk.slice(3));}else fields.push(chunk);}if(fields.length)args.push('{'+fields.join(',')+'}');out+='Object.assign('+args.join(',')+')';}else out+='{'+inner+'}';i=j;}return out;}
extracted=qtSpreads(extracted).replace(/Object\.fromEntries\(/g,'nativeFromEntries(');
const names=[...extracted.matchAll(/^function (\w+)\(/gm)].map(m=>m[1]).filter(n=>!['isPreviewStateEqual'].includes(n));
const adapter=`
// The React atom registry is replaced only at its storage boundary. Keys are
// scoped tuples; prototype-like IDs and ambiguous environment/thread pairs
// cannot alias another thread. State and reducers retain native semantics.
function nativeFromEntries(entries){const result={};entries.forEach(entry=>Object.defineProperty(result,entry[0],{value:entry[1],enumerable:true,writable:true,configurable:true}));return result;}
const states=new Map(),listeners=new Map();
function threadKey(ref){return JSON.stringify([ref.environmentId,ref.threadId]);}
function readThreadPreviewState(ref){return states.get(threadKey(ref)) || EMPTY_THREAD_PREVIEW_STATE;}
function updateThreadPreviewState(ref,update){const key=threadKey(ref),previous=readThreadPreviewState(ref),next=update(previous);if(next===previous)return;states.set(key,next);if(onChanged)onChanged(ref,next);const callbacks=listeners.get(key);if(callbacks)callbacks.slice().forEach(fn=>fn(next));}
function subscribe(ref,callback){const key=threadKey(ref),callbacks=listeners.get(key)||[];callbacks.push(callback);listeners.set(key,callbacks);return function(){const i=callbacks.indexOf(callback);if(i>=0)callbacks.splice(i,1);if(!callbacks.length)listeners.delete(key);};}
function resetPreviewStateForTests(){states.clear();}
`;
const output='.pragma library\n// T3 Code '+pin+'; MIT, licenses/T3-Code-MIT.txt.\n// Generated by tools/build-preview-state.cjs. Do not edit reducers here.\nfunction createPreviewStore(onChanged){\nconst PREVIEW_RECENT_URL_LIMIT='+limit+';\n'+extracted+adapter+'\nreturn {'+names.join(',')+',readThreadPreviewState,subscribe,resetPreviewStateForTests,__testing:{EMPTY_THREAD_PREVIEW_STATE,RECENT_URL_LIMIT:PREVIEW_RECENT_URL_LIMIT}};\n}\n';
vm.runInNewContext(output.replace(/^\.pragma library\n/,''));
fs.writeFileSync(path.join(root,'ui/js/NativePreviewState.js'),output);
fs.writeFileSync(path.join(root,'ui/js/NativePreviewState.sources.json'),JSON.stringify({commit:pin,files:inputs,adaptations:['Original preview state/reducers extracted verbatim with TypeScript erasure and export removal','React atom storage boundary replaced by a factory with scoped tuple Map keys and change subscriptions','Qt object spread/rest becomes own-key Object.assign copies; toSorted/at become copying sort/slice; Object.fromEntries becomes own-property entries construction','React hooks, active-thread aggregate and timer-based whenPreviewTabKnown excluded; no transport or runtime-support claims'],outputSha256:hash(output)},null,2)+'\n');
console.log('Native preview state: '+names.length+' helpers/reducers; recent URL limit '+limit);
