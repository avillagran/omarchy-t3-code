// Evaluate both whole original DSP modules in actual pinned Node. Only the
// typed device sample conversion boundary is supplied by the native corpus.
import fs from 'node:fs';import vm from 'node:vm';import crypto from 'node:crypto';import {stripTypeScriptTypes} from 'node:module';
const [native,driver,codecs]=process.argv.slice(2);
if(!fs.existsSync('/run/.containerenv')||process.versions.node!=='26.13.0')throw Error('Private pinned Node required');
const expected={'pcmResampler.ts':'933981998febcd721175c8e83b56570fb7cb42a0bc6d2a967f244ce1cc0881f5','pcmWorklet.ts':'cd93d59fc68414be05db6bae6808f20877245c79cec9253558e34040381e1983'},sources={};
for(const[name,hash]of Object.entries(expected)){const bytes=fs.readFileSync(native+'/apps/web/src/speech/'+name);if(crypto.createHash('sha256').update(bytes).digest('hex')!==hash)throw Error('Changed whole native DSP source');sources[name]=stripTypeScriptTypes(bytes.toString(),{mode:'strip'});}
vm.runInThisContext(fs.readFileSync(driver,'utf8'),{filename:driver});
const hashes=new Map(),lengths=new Map();
const digest={append(key,buffer){if(!hashes.has(key)){hashes.set(key,crypto.createHash('sha256'));lengths.set(key,0)}hashes.get(key).update(Buffer.from(buffer));lengths.set(key,lengths.get(key)+buffer.byteLength)},finish(key){if(!hashes.has(key))this.append(key,new ArrayBuffer(0));return lengths.get(key)+':'+hashes.get(key).digest('hex')}};
const bytes=fs.readFileSync(codecs),buffer=bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength);
const result=await runPcmIntegerOracle(async(kind,rate,channels,port)=>{
 let Processor;
 const context=vm.createContext({sampleRate:rate,AudioWorkletProcessor:class{constructor(){this.port=port}},registerProcessor:(name,value)=>{if(name!=='t3-pcm-capture')throw Error('Unexpected processor');Processor=value}});
 const resampler=new vm.SourceTextModule(sources['pcmResampler.ts'],{context}),worklet=new vm.SourceTextModule(sources['pcmWorklet.ts'],{context});
 await worklet.link(async name=>{if(name!=='./pcmResampler')throw Error('Unexpected native import');return resampler});await worklet.evaluate();
 const processor=new Processor();
 return {start(){port.onmessage({data:'start'})},feed(raw,normalized){processor.process([normalized])},stop(){port.onmessage({data:'stop'})}};
},digest,buffer);
process.stdout.write(JSON.stringify(result)+'\n');
