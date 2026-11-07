// Native oracle: unbundled complete producers/codecs and actual Effect4.1.2.
import {registerHooks,createRequire} from 'node:module';
import fs from 'node:fs';
import {pathToFileURL} from 'node:url';
const require=createRequire('/dependencies/package.json');
// Resolve before registration: require.resolve itself participates in hooks.
const effectPaths=new Map(['Effect','Schema','SchemaTransformation','encoding/Hex','Record'].map(name=>['effect/'+name,pathToFileURL(require.resolve('effect/'+name)).href]));
registerHooks({resolve(specifier,context,next){
 if(/^(native|adapter):/.test(specifier))return {url:pathToFileURL('/oracle/'+specifier.replace(':','-')+'.mjs').href,shortCircuit:true};
 if(specifier==='react')return {url:pathToFileURL('/oracle/adapter-hooks.mjs').href,shortCircuit:true};
 if(specifier==='@t3tools/contracts')return {url:pathToFileURL('/oracle/native-contract.mjs').href,shortCircuit:true};
 if(specifier==='../lib/utils'&&context.parentURL?.endsWith('/native-consent.mjs'))return {url:pathToFileURL('/oracle/native-uuid.mjs').href,shortCircuit:true};
 if(specifier==='../hooks/useLocalStorage'&&context.parentURL?.endsWith('/native-consent.mjs'))return {url:pathToFileURL('/oracle/native-storage.mjs').href,shortCircuit:true};
 if(effectPaths.has(specifier))return {url:effectPaths.get(specifier),shortCircuit:true};
 return next(specifier,context);
}});
const native=await import('native:entry');
const {exercise}=await import('/source/consent-traces.mjs');
fs.writeFileSync('/state/native-traces.json',JSON.stringify(exercise(native.create),null,2)+'\n');
