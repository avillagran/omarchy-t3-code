// Build-only intake of the whole native consent/storage producers and codecs.
import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';
import {createRequire,stripTypeScriptTypes} from 'node:module';import {pathToFileURL,fileURLToPath} from 'node:url';
if(!fs.existsSync('/run/.containerenv')||process.versions.node!=='26.13.0')throw Error('Private pinned build runtime required');
const [native,dependencies,compiler]=process.argv.slice(2),root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const require=createRequire(dependencies+'/package.json'),babelRequire=createRequire(compiler+'/package.json');
if(require('effect/package.json').version!=='4.1.2'||require('rolldown/package.json').version!=='1.2.16')throw Error('Changed consent dependencies');
const Babel=await import(pathToFileURL(babelRequire.resolve('@babel/core')).href);if(Babel.version!=='8.1.0')throw Error('Changed AST compiler');
const {rolldown}=await import(pathToFileURL(require.resolve('rolldown')).href);
const revision='83a3674ffd84ffb33de768210c806cb1ac53fd80';
if(fs.readFileSync(native+'/REVISION','utf8').trim()!==revision)throw Error('Changed native revision');
const names=['apps/web/src/speech/voiceConsent.ts','apps/web/src/hooks/useLocalStorage.ts','apps/web/src/lib/utils.ts','packages/contracts/src/baseSchemas.ts','packages/contracts/src/speech.ts','packages/contracts/src/settings.ts'];
const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex'),sources={};
const source=name=>{const bytes=fs.readFileSync(native+'/'+name);sources[name]=hash(bytes);return stripTypeScriptTypes(bytes.toString(),{mode:'strip'})};
function declarations(name,requested,imports){
 const parsed=Babel.parseSync(source(name),{sourceType:'module',configFile:false,babelrc:false});
 const selected=[];
 for(const statement of parsed.program.body){
  const declaration=statement.type==='ExportNamedDeclaration'?statement.declaration:statement;
  const id=declaration?.id?.name??(declaration?.type==='VariableDeclaration'&&declaration.declarations.length===1?declaration.declarations[0].id.name:undefined);
  if(requested.includes(id))selected.push({type:'ExportNamedDeclaration',declaration,specifiers:[],source:null});
 }
 if(selected.length!==requested.length)throw Error('Changed complete native declarations '+name);
 const program=Babel.parseSync(imports,{sourceType:'module',configFile:false,babelrc:false});program.program.body.push(...selected);
 return Babel.transformFromAstSync(program,null,{configFile:false,babelrc:false,comments:true}).code;
}
const modules=new Map();
modules.set('native:base',declarations(names[3],['trim','TrimmedString','isNonBlank','TrimmedNonEmptyString','makeEntityId','EnvironmentId'],`import * as Effect from 'effect/Effect';import * as Schema from 'effect/Schema';import * as SchemaTransformation from 'effect/SchemaTransformation';`));
modules.set('native:cloud',declarations(names[4],['CloudSpeechModelId'],`import * as Schema from 'effect/Schema';`));
modules.set('native:contract',declarations(names[5],['VoiceTranscriptionDestination'],`import * as Schema from 'effect/Schema';import {EnvironmentId} from 'native:base';import {CloudSpeechModelId} from 'native:cloud';`));
modules.set('native:uuid',declarations(names[2],['randomUUID'],`import * as Hex from 'effect/encoding/Hex';`));
modules.set('native:storage',`import {window,CustomEvent,console} from 'adapter:platform';\n`+source(names[1]));
modules.set('native:consent',`import {window} from 'adapter:platform';\n`+source(names[0]));
modules.set('adapter:platform',`export let window,CustomEvent,console;let initialized=false;
export function initialize(platform){if(initialized)throw Error('Consent realm already initialized');initialized=true;window=platform.window;CustomEvent=platform.CustomEvent;console=platform.console;}`);
modules.set('adapter:hooks',`// Qt uses an external storage-change signal and requests a fresh snapshot.
export function useSyncExternalStore(_subscribe,getSnapshot){return getSnapshot();}`);
modules.set('native:entry',`import {initialize} from 'adapter:platform';import * as consent from 'native:consent';
export function create(platform){initialize(platform);return consent;}`);
const bundle=await rolldown({input:'native:entry',platform:'node',plugins:[{name:'whole-native-consent',resolveId(spec,importer){
 if(modules.has(spec))return spec;
 if(spec==='react')return 'adapter:hooks';
 if(spec==='@t3tools/contracts')return 'native:contract';
 if(spec==='../lib/utils'&&importer==='native:consent')return 'native:uuid';
 if(spec==='../hooks/useLocalStorage'&&importer==='native:consent')return 'native:storage';
 if(spec.startsWith('effect/'))return require.resolve(spec);
},load(id){if(modules.has(id))return modules.get(id);}}]});
const generated=await bundle.generate({format:'cjs',codeSplitting:false});if(generated.output.length!==1)throw Error('Unexpected consent bundle');
const banner='// Original T3 Code '+revision+'. MIT; licenses/T3-Code-MIT.txt.\n// Effect4.1.2: licenses/Effect-MIT.txt. Generated by tools/build-native-voice-consent.mjs.\n';
const output=banner+generated.output[0].code;fs.writeFileSync(root+'/ui/voice/native-consent.cjs',output);
// Retain the unbundled whole modules for the independent pinned-Node oracle.
fs.mkdirSync(root+'/oracle',{recursive:true});
for(const [name,source]of modules)fs.writeFileSync(root+'/oracle/'+name.replace(':','-')+'.mjs',source);
const inputs=Object.keys(generated.output[0].modules).filter(id=>id.startsWith(dependencies+'/')).sort();
fs.writeFileSync(root+'/ui/voice/native-consent.sources.json',JSON.stringify({revision,sourceFiles:sources,node:process.versions.node,effect:'4.1.2',rolldown:'1.2.16',astCompiler:'8.1.0',outputSha256:hash(output),dependencyInputs:Object.fromEntries(inputs.map(id=>[id.slice(dependencies.length+1),hash(fs.readFileSync(id))])),adaptations:['Whole original consent and local-storage modules retained','Whole codec and UUID declarations selected without body changes','Actual original Effect codecs/equivalence; no handwritten consent state or schema parser','Framework-only useSyncExternalStore adapter reads fresh snapshots; Qt storage notifications are a separate consumer','Explicit realm-local window/CustomEvent/console bindings; no global/prototype mutation'],scope:'Original consent data producer only; filesystem, helper lifetime, UI and audio-send consumers require separate acceptance'},null,2)+'\n');
await bundle.close();console.log(JSON.stringify({wholeModules:2,wholeDeclarations:9,bytes:Buffer.byteLength(output),dependencyInputs:inputs.length}));
