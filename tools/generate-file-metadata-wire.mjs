#!/usr/bin/env node
// Execute the complete pinned contracts and their real Effect codecs.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {createRequire, stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
const [checkout, dependencies, output] = process.argv.slice(2);
if (!output) throw Error('usage: generate-file-metadata-wire.mjs CHECKOUT DEPS OUTPUT');
const revision = '32a0531748c44b25072d7495ce0b9bd7e9450a87';
const require = createRequire(path.join(dependencies, 'package.json'));
if (require('effect/package.json').version !== '4.1.2') throw Error('Effect 4.1.2 required');
const runtime = path.join(output, 'wire-runtime');
fs.mkdirSync(runtime, {recursive:true});
const hashes = {};
function source(filename) {
  const text = execFileSync('git', ['-C',checkout,'show',revision+':packages/contracts/src/'+filename], {encoding:'utf8'});
  hashes[filename] = crypto.createHash('sha256').update(text).digest('hex');
  return stripTypeScriptTypes(text).replace(/(["'])(effect\/[^"']+)\1/g, (_,quote,specifier)=>JSON.stringify(pathToFileURL(require.resolve(specifier)).href));
}
const base = path.join(runtime,'baseSchemas.mjs');
fs.writeFileSync(base,source('baseSchemas.ts'));
const filesystem = path.join(runtime,'filesystem.mjs');
fs.writeFileSync(filesystem,source('filesystem.ts').replace('"./baseSchemas.ts"',JSON.stringify(pathToFileURL(base).href)));
const Schema = await import(pathToFileURL(require.resolve('effect/Schema')).href);
const contract = await import(pathToFileURL(filesystem).href);
const decode = Schema.decodeUnknownSync(contract.FilesystemGetMetadataResult);
const cases = [{},null,{entries:null},{entries:[]},{entries:[null]},{entries:[{}]}];
for (const kind of ['file','directory','other','symlink',null,1]) cases.push({entries:[{kind}]});
for (const byteLength of [null,-1,0,1,1.5,2**53-1,2**53,1e100,'1']) cases.push({entries:[{kind:'file',byteLength}]});
for (const mimeType of [null,'',' ','\u0085','\ufeff','\ufeff text/plain \u00a0','text/plain',42]) cases.push({entries:[{kind:'file',mimeType}]});
cases.push({entries:[{kind:'file',extra:1},null,{kind:'other'}]});
const observations = cases.map(input=>{try{return{input,result:decode(input),valid:true}}catch{return{input,valid:false}}});
fs.writeFileSync(path.join(output,'wire-fixtures.json'),JSON.stringify({revision,hashes,cases:observations},null,2)+'\n');
console.log(JSON.stringify({wireCases:observations.length}));
