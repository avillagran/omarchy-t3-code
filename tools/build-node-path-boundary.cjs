// Build-only extraction of Node's licensed POSIX string-path implementation.
const crypto=require('node:crypto');
function build() {
 if(process.versions.node!=='26.13.0')throw Error('Node26.13.0 required for pinned POSIX path boundary');
 const source=process.binding('natives').path;
 if(crypto.createHash('sha256').update(source).digest('hex')!=='f05ab2a1f33102880bb8c5052cb0e634a001112883da897f3498492bef67f3f6')throw Error('Pinned Node path source mismatch');
 const extract=name=>{const start=source.indexOf('function '+name+'('),end=source.indexOf('\n}',start);if(start<0||end<start)throw Error('Pinned Node helper missing '+name);return source.slice(start,end+2);};
 const native=require('node:path').posix;
 const header=source.slice(0,source.indexOf("'use strict';"));
 const code=header+'\n// String-only POSIX join boundary extracted from Node26.13.0.\n'+`
const CHAR_FORWARD_SLASH=47,CHAR_DOT=46;
const uncurry=fn=>Function.prototype.call.bind(fn);
const StringPrototypeCharCodeAt=uncurry(String.prototype.charCodeAt),StringPrototypeSlice=uncurry(String.prototype.slice),StringPrototypeLastIndexOf=uncurry(String.prototype.lastIndexOf),ArrayPrototypeJoin=uncurry(Array.prototype.join);
const validateString=(value)=>{if(typeof value!=="string")throw TypeError("path must be a string");};
`+extract('isPosixPathSeparator')+'\n'+extract('normalizeString')+'\nconst posix={'+native.normalize.toString()+','+native.join.toString()+'};\nexport const join=posix.join;\n';
 return {code,sourceSha256:crypto.createHash('sha256').update(source).digest('hex')};
}
module.exports={build};
if(require.main===module){const fs=require('node:fs'),path=require('node:path');const data=build();fs.writeFileSync(process.argv[2],data.code);fs.writeFileSync(path.dirname(process.argv[2])+'/node-path-source.json',JSON.stringify({node:process.versions.node,sourceSha256:data.sourceSha256},null,2)+'\n');}
