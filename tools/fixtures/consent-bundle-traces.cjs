'use strict';
const fs=require('node:fs');
if(process.versions.electron!=='44.10.1'||process.versions.node!=='24.22.1')throw Error('Changed native runtime');
import('./consent-traces.mjs').then(({exercise})=>{
 fs.writeFileSync('/state/bundle-traces.json',JSON.stringify(exercise(require('./native-consent.cjs').create),null,2)+'\n');
}).catch(()=>{process.exitCode=1;});
