// Whole original modules in their actual pinned Node engine; no copied kernels.
import fs from 'node:fs';import vm from 'node:vm';import crypto from 'node:crypto';import {stripTypeScriptTypes} from 'node:module';
const [native,driver]=process.argv.slice(2);
if(!fs.existsSync('/run/.containerenv')||process.versions.node!=='26.13.0')throw Error('Private pinned Node required');
const sources={
 'pcmResampler.ts':'933981998febcd721175c8e83b56570fb7cb42a0bc6d2a967f244ce1cc0881f5',
 'pcmWorklet.ts':'cd93d59fc68414be05db6bae6808f20877245c79cec9253558e34040381e1983'};
const modules={};for(const[name,expected]of Object.entries(sources)){const bytes=fs.readFileSync(native+'/apps/web/src/speech/'+name);if(crypto.createHash('sha256').update(bytes).digest('hex')!==expected)throw Error('Changed source');modules[name]=stripTypeScriptTypes(bytes.toString(),{mode:'strip'});}
vm.runInThisContext(fs.readFileSync(driver,'utf8'),{filename:driver});
const hashes=new Map(),lengths=new Map();
const digest={append(key,buffer){if(!hashes.has(key)){hashes.set(key,crypto.createHash('sha256'));lengths.set(key,0);}hashes.get(key).update(Buffer.from(buffer));lengths.set(key,lengths.get(key)+buffer.byteLength)},finish(key){if(!hashes.has(key))this.append(key,new ArrayBuffer(0));return lengths.get(key)+':'+hashes.get(key).digest('hex')}};
const result=await runPcmOracle(async(rate,port)=>{
 let Processor;
 const context=vm.createContext({sampleRate:rate,AudioWorkletProcessor:class{constructor(){this.port=port}},registerProcessor:(name,value)=>{if(name!=='t3-pcm-capture')throw Error('Unexpected processor');Processor=value}});
 const resampler=new vm.SourceTextModule(modules['pcmResampler.ts'],{context,identifier:'whole-resampler'});
 const worklet=new vm.SourceTextModule(modules['pcmWorklet.ts'],{context,identifier:'whole-worklet'});
 await worklet.link(async specifier=>{if(specifier!=='./pcmResampler')throw Error('Unexpected import');return resampler});await worklet.evaluate();
 return {PcmResampler:resampler.namespace.PcmResampler,Processor,math:vm.runInContext('Math',context)};
},digest);
process.stdout.write(JSON.stringify(result)+'\n');
