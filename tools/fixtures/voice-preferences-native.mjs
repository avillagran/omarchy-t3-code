// Execute complete original declarations without the Qt flatMap substitution.
import fs from 'node:fs';import path from 'node:path';import vm from 'node:vm';import {createRequire,stripTypeScriptTypes} from 'node:module';import {pathToFileURL} from 'node:url';import {createHash} from 'node:crypto';
if(!fs.existsSync('/run/.containerenv')||process.versions.node!=='26.13.0')throw Error('Private original runtime required');
const [native,compiler,manifestPath,driverPath]=process.argv.slice(2),manifest=JSON.parse(fs.readFileSync(manifestPath));
const require=createRequire(path.join(compiler,'package.json'));const Babel=await import(pathToFileURL(require.resolve('@babel/core')).href);const selected=new Map();
function visit(node){if(!node||typeof node!=='object')return;let name;
 if(node.type==='FunctionDeclaration'&&['cleanupContext','resolveSpeechVocabulary'].includes(node.id?.name))name=node.id.name;
 if(node.type==='VariableDeclarator'&&['vocabularyKey','postProcess','recordingPreferences'].includes(node.id?.name))name=node.id.name;
 if(node.type==='ObjectProperty'&&node.key?.name==='getTranscriptionOptions'){name='getTranscriptionOptions';node=node.value;}
 if(name){if(selected.has(name))throw Error('Original declaration ambiguous');selected.set(name,node);}
 for(const [key,value]of Object.entries(node)){if(['loc','start','end','extra'].includes(key))continue;if(Array.isArray(value))value.forEach(visit);else if(value&&typeof value==='object')visit(value);}
}
for(const [name,expected]of Object.entries(manifest.sourceFiles)){const bytes=fs.readFileSync(path.join(native,name));if(createHash('sha256').update(bytes).digest('hex')!==expected)throw Error('Changed original source');visit(Babel.parseSync(stripTypeScriptTypes(bytes.toString(),{mode:'strip'}),{sourceType:'module',configFile:false,babelrc:false}).program);}
const order=['vocabularyKey','resolveSpeechVocabulary','cleanupContext','recordingPreferences','getTranscriptionOptions','postProcess'];if(selected.size!==6)throw Error('Original callbacks unavailable');
const program=Babel.parseSync('',{sourceType:'script',configFile:false,babelrc:false});program.program.body=order.map(name=>{let node=selected.get(name);if(node.type==='FunctionDeclaration')return node;if(node.type!=='VariableDeclarator')node={type:'VariableDeclarator',id:{type:'Identifier',name},init:node};return {type:'VariableDeclaration',kind:name==='recordingPreferences'?'let':'const',declarations:[node]};});
const original=Babel.transformFromAstSync(program,null,{configFile:false,babelrc:false,plugins:[]}).code;
const factory=new Function('platform',manifest.dependencies.filter(name=>name!=='flatMap').map(name=>'var '+name+'=platform.'+name+';').join('\n')+original+'\nreturn {getTranscriptionOptions,postProcess,cleanupContext,resolveSpeechVocabulary,getRecordingPreferences:()=>recordingPreferences};');
vm.runInThisContext(fs.readFileSync(driverPath,'utf8'));const result=await runVoicePreferencesOracle(factory,(array,mapper)=>array.flatMap(mapper));console.log(JSON.stringify(result));
