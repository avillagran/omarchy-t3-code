// Build-only native controller intake. Runtime adapters do not replace its state machine.
import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';
import {createRequire,stripTypeScriptTypes} from 'node:module';import {fileURLToPath,pathToFileURL} from 'node:url';
if(!fs.existsSync('/run/.containerenv'))throw Error('Private build container required');
if(process.versions.node!=='26.13.0'||process.versions.unicode!=='17.0')throw Error('Pinned Node/Unicode required');
const [native,compiler]=process.argv.slice(2),root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const revision='83a3674ffd84ffb33de768210c806cb1ac53fd80';
const hashes={
 'packages/client-runtime/src/voice-input/controller.ts':'b9fc6eaeabba788714f71ef4c9ad72a63ff0d1f649ebdae5b3648b830db7a7b5',
 'packages/client-runtime/src/voice-input/transcription.ts':'f8e2c63545266219eea91e3316252c532a6120ceb8fecc8b60438bb8ba828ee6',
 'packages/shared/src/composerTrigger.ts':'26a4eb53ce8cb8cc38ba1c4e9be42c4f58143b2e82a3210d26cb1b508a534aee',
};
const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
if(fs.readFileSync(path.join(native,'REVISION'),'utf8').trim()!==revision)throw Error('Changed admitted native revision');
const sources=Object.fromEntries(Object.entries(hashes).map(([name,digest])=>{const bytes=fs.readFileSync(path.join(native,name));if(hash(bytes)!==digest)throw Error('Changed native source '+name);return[name,stripTypeScriptTypes(bytes.toString(),{mode:'strip'})];}));
const lock=fs.readFileSync(path.join(root,'tools/voice-compiler/package-lock.json'));
if(hash(lock)!==hash(fs.readFileSync(path.join(compiler,'package-lock.json'))))throw Error('Changed compiler lock');
const require=createRequire(path.join(compiler,'package.json'));const Babel=await import(pathToFileURL(require.resolve('@babel/core')).href);
const asyncPlugin=(await import(pathToFileURL(require.resolve('@babel/plugin-transform-async-to-generator')).href)).default;
const fieldsPlugin=(await import(pathToFileURL(require.resolve('@babel/plugin-transform-class-properties')).href)).default;
const catchPlugin=(await import(pathToFileURL(require.resolve('@babel/plugin-transform-optional-catch-binding')).href)).default;
const bodies=[];
for(const [name,source]of Object.entries(sources)){
 const program=Babel.parseSync(source,{sourceType:'module',configFile:false,babelrc:false});
 for(const statement of program.program.body){
  if(statement.type==='ImportDeclaration')continue;
  const declaration=statement.type==='ExportNamedDeclaration'?statement.declaration:statement;
  if(!declaration)throw Error('Unexpected native export');
  if(name.endsWith('composerTrigger.ts')&&declaration.id?.name!=='replaceTextRange')continue;
  if(declaration.id?.name==='resetVoiceInputGlobalsForTests')continue;
  bodies.push({name,declaration});
 }
}
// The transcription error type must be defined before the controller's predicate.
bodies.sort((a,b)=>Number(a.name.endsWith('controller.ts'))-Number(b.name.endsWith('controller.ts')));
const program=Babel.parseSync('',{sourceType:'script',configFile:false,babelrc:false});program.program.body=bodies.map(row=>row.declaration);
const patterns=new Map(),ranges={lettersMarksNumbers:[],latinNumbers:[]};
const propertyPlugin=()=>({visitor:{RegExpLiteral(p){const{pattern,flags}=p.node;if(!pattern.includes('\\p{'))return;if(flags!=='u')throw Error('Unexpected native Unicode flags');let key;
 if(pattern.startsWith('(?:\\p{L}'))key='left';else if(pattern.startsWith('^[\\p{L}'))key='right';else if(pattern.startsWith('^[\\p{Script=Latin}'))key='first';else if(pattern.startsWith('(?:\\p{Script=Latin}'))key='last';else throw Error('Unknown native Unicode expression');
 if(patterns.has(key))throw Error('Duplicate native property expression');patterns.set(key,new RegExp(pattern,flags));p.replaceWith({type:'CallExpression',callee:{type:'Identifier',name:'__nativeVoiceRegexp'},arguments:[{type:'StringLiteral',value:key}]});
}}});
const compiled=Babel.transformFromAstSync(program,null,{configFile:false,babelrc:false,sourceType:'script',comments:true,plugins:[propertyPlugin,asyncPlugin,fieldsPlugin,catchPlugin]}).code;
if(patterns.size!==4)throw Error('All four original Unicode expressions are required');
const tests={lettersMarksNumbers:/[\p{L}\p{M}\p{N}]/u,latinNumbers:/[\p{Script=Latin}\p{N}]/u},opened={lettersMarksNumbers:null,latinNumbers:null};
const golden=Buffer.alloc(0x110000);
for(let value=0;value<0x110000;value++){
 const character=String.fromCodePoint(value);
 for(const [key,test]of Object.entries(tests)){const accepted=test.test(character);if(accepted&&opened[key]===null)opened[key]=value;if(!accepted&&opened[key]!==null){ranges[key].push(opened[key],value-1);opened[key]=null;}}
 for(const [index,key]of ['left','right','first','last'].entries())if(patterns.get(key).test(character))golden[value]|=1<<index;
}
for(const key of Object.keys(tests))if(opened[key]!==null)ranges[key].push(opened[key],0x10ffff);
const unicode=`var __voiceRanges=${JSON.stringify(ranges)};
function __voiceContains(ranges,point){var low=0,high=ranges.length/2-1;while(low<=high){var mid=(low+high)>>1,start=ranges[mid*2],end=ranges[mid*2+1];if(point<start)high=mid-1;else if(point>end)low=mid+1;else return true;}return false;}
function __voiceLastPoint(text){var end=text.length-1,value=text.charCodeAt(end);if(value>=56320&&value<=57343&&end>0){var high=text.charCodeAt(end-1);if(high>=55296&&high<=56319)return (high-55296)*1024+value-56320+65536;}return value;}
function __nativeVoiceRegexp(kind){return {test:function(value){var text=String(value);if(!text.length)return false;var point=(kind==='left'||kind==='last')?__voiceLastPoint(text):text.codePointAt(0);if(kind==='first'||kind==='last')return __voiceContains(__voiceRanges.latinNumbers,point);if(__voiceContains(__voiceRanges.lettersMarksNumbers,point))return true;var punctuation=kind==='left'?'.!?,:;)]}\\\'"':'([{\\\'"';return punctuation.indexOf(String.fromCodePoint(point))>=0;}};}
`;
const output='.pragma library\n// Original T3 Code '+revision+'. MIT; licenses/T3-Code-MIT.txt.\n// Babel8.1.0 helpers: MIT; licenses/Babel-helpers-MIT.txt.\n// Generated by tools/build-native-voice.mjs; Node/Babel are build-only.\nfunction create(platform){\nvar Schema=platform.Schema,AbortController=platform.AbortController;\n'+unicode+compiled+'\nreturn {VoiceInputController:VoiceInputController,VoiceTranscriptionError:VoiceTranscriptionError,throwIfVoiceTranscriptionAborted:throwIfVoiceTranscriptionAborted,resolveTranscriptCommit:resolveTranscriptCommit,voiceInputBlocksSubmission:voiceInputBlocksSubmission,recordingLimitSeconds:VOICE_RECORDING_LIMIT_SECONDS,errorPredicate:isVoiceTranscriptionError,propertyPredicate:__nativeVoiceRegexp};\n}\n';
fs.writeFileSync(path.join(root,'ui/js/NativeVoice.js'),output);
fs.writeFileSync(path.join(root,'tests/fixtures/voice-unicode-4314.bin'),golden);
fs.writeFileSync(path.join(root,'ui/js/NativeVoice.sources.json'),JSON.stringify({revision,sourceFiles:hashes,node:process.versions.node,unicode:process.versions.unicode,compilerLockSha256:hash(lock),compilerVersions:{core:'8.1.0',async:'8.0.1',fields:'8.0.1',catch:'8.0.1'},unicodeRanges:Object.fromEntries(Object.entries(ranges).map(([name,value])=>[name,value.length/2])),unicodeScalarObservations:golden.length,unicodeGoldenSha256:hash(golden),outputSha256:hash(output),adaptations:['TypeScript erasure and import/export linkage; complete controller/helpers retained','Standard Babel async-to-generator/class-property/optional-catch-binding compilation for Qt','Four native Unicode property expressions use tables generated by original Node17 Unicode predicates over every code point','Platform abort/error-schema dependencies are explicit adapters; no global prototype changes'],scope:'Generated UI controller only; no recording, network transport or server capability activation'},null,2)+'\n');
console.log(JSON.stringify({outputBytes:Buffer.byteLength(output),unicodeObservations:golden.length,ranges:Object.fromEntries(Object.entries(ranges).map(([name,value])=>[name,value.length/2]))}));
