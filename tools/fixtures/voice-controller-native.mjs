// Execute whole immutable T3 modules with their real Effect schema dependency.
import fs from 'node:fs';
import vm from 'node:vm';
import crypto from 'node:crypto';
import {createRequire,stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const [native,dependencies,driver]=process.argv.slice(2);
if(!fs.existsSync('/run/.containerenv')||process.versions.node!=='26.13.0')throw Error('Private pinned Node required');
const require=createRequire(dependencies+'/package.json');
if(require('effect/package.json').version!=='4.1.2')throw Error('Canonical Effect required');
const Schema=await import(pathToFileURL(require.resolve('effect/Schema')).href);
const hashes={
 controller:['packages/client-runtime/src/voice-input/controller.ts','b9fc6eaeabba788714f71ef4c9ad72a63ff0d1f649ebdae5b3648b830db7a7b5'],
 transcription:['packages/client-runtime/src/voice-input/transcription.ts','f8e2c63545266219eea91e3316252c532a6120ceb8fecc8b60438bb8ba828ee6'],
 composer:['packages/shared/src/composerTrigger.ts','26a4eb53ce8cb8cc38ba1c4e9be42c4f58143b2e82a3210d26cb1b508a534aee']};
const modules=new Map([['effect/Schema',new vm.SyntheticModule(Object.keys(Schema),function(){for(const[key,value]of Object.entries(Schema))this.setExport(key,value)})]]);
for(const[name,[file,digest]]of Object.entries(hashes)){
 const bytes=fs.readFileSync(native+'/'+file);if(crypto.createHash('sha256').update(bytes).digest('hex')!==digest)throw Error('Native source changed');
 modules.set(name,new vm.SourceTextModule(stripTypeScriptTypes(bytes.toString(),{mode:'strip'}),{identifier:name}));
}
await modules.get('controller').link(async(spec)=>{
 const name=spec==='effect/Schema'?spec:spec==='./transcription.ts'?'transcription':spec==='@t3tools/shared/composerTrigger'?'composer':null;
 if(!name)throw Error('Unreviewed native import '+spec);return modules.get(name);
});
await modules.get('controller').evaluate();
vm.runInThisContext(fs.readFileSync(driver,'utf8'),{filename:driver});
const controller=modules.get('controller').namespace,transcription=modules.get('transcription').namespace;
const output=await runVoiceOracle(()=>{controller.resetVoiceInputGlobalsForTests();return {...controller,...transcription,errorPredicate:Schema.is(transcription.VoiceTranscriptionError)}});
process.stdout.write(JSON.stringify(output)+'\n');
