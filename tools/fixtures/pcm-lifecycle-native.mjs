// The complete original producer, evaluated without Babel in pinned Node.
import fs from 'node:fs';import vm from 'node:vm';import crypto from 'node:crypto';import {stripTypeScriptTypes} from 'node:module';
const [native,driver]=process.argv.slice(2);
if(!fs.existsSync('/run/.containerenv')||process.versions.node!=='26.13.0')throw Error('Private pinned Node required');
const bytes=fs.readFileSync(native+'/apps/web/src/speech/browserPcmCapture.ts');
if(crypto.createHash('sha256').update(bytes).digest('hex')!=='964d7cd563915d9e79900ee9940367d3ed990387a1c92f33be37e11dadae49af')throw Error('Changed whole capture source');
const code=stripTypeScriptTypes(bytes.toString(),{mode:'strip'}),contexts=[];
// Modules evaluate asynchronously, so prepare factories first. Each case gets
// independent globals; there is no ambient navigator/URL replacement in Qt.
vm.runInThisContext(fs.readFileSync(driver,'utf8'),{filename:driver});
const factories=[];
for(let index=0;index<25;index++){
 const context=vm.createContext({});
 const module=new vm.SourceTextModule(code,{context});
 await module.link(async specifier=>{if(specifier!=='./pcmWorklet.ts?worker&url')throw Error('Unexpected capture dependency');return new vm.SyntheticModule(['default'],function(){this.setExport('default','owned:whole-native-worklet')},{context})});
 await module.evaluate();contexts.push(context);
 factories.push(platform=>{Object.assign(context,platform);return module.namespace});
}
let next=0;
const result=await runCaptureLifecycle(platform=>factories[next++](platform));
if(next!==25)throw Error('Changed lifecycle case count');
process.stdout.write(JSON.stringify(result)+'\n');
