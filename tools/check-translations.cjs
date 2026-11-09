#!/usr/bin/env node
const fs=require('fs'),assert=require('node:assert/strict');const {validate}=require('./validate-catalog.js');
const source=JSON.parse(fs.readFileSync('translations/source.json')),catalog=JSON.parse(fs.readFileSync('translations/i18n.json'));const keys=Object.keys(source.en);
const result=validate(source,catalog);assert.deepEqual(result.errors,[]);assert.deepEqual(result.warnings,[]);assert(Object.keys(catalog).length>=20);
for(const [l,c]of Object.entries(catalog))assert.deepEqual(Object.keys(c).sort(),keys.slice().sort(),l+' complete coverage');
// Validate the actual UI tree and only the translation namespace it uses.
// OmaBridge owns a separate local tr(key) catalog; its keys are not English.
const path=require('node:path');
const files=fs.readdirSync('.').filter(f=>f.endsWith('.qml'));
if(fs.existsSync('ui/qml'))files.push(...fs.readdirSync('ui/qml',{recursive:true}).filter(f=>f.endsWith('.qml')).map(f=>path.join('ui/qml',f)));
for(const f of files){
 const text=fs.readFileSync(f,'utf8');
 const delegates=path.basename(f)==='T3I18n.qml'||/function tr\([^)]*\)\s*\{\s*return T3I18n\.tr\(/.test(text);
 const calls=delegates?/(?<![\w.])(?:T3I18n\.|root\.)?tr\(("(?:[^"\\]|\\.)*")/g:/\bT3I18n\.tr\(("(?:[^"\\]|\\.)*")/g;
 const keys=Array.from(text.matchAll(calls),m=>JSON.parse(m[1]));
 // Backend errors use a bounded literal code-to-source map before tr(source).
 if(path.basename(f)==='T3I18n.qml'){
  const map=/var backendErrors=\{([^}]+)\};/.exec(text);
  assert(map,'Backend error translation map must remain inspectable');
  keys.push(...Array.from(map[1].matchAll(/:\s*("(?:[^"\\]|\\.)*")/g),m=>JSON.parse(m[1])));
 }
 for(const k of keys)assert(k in source.en,f+' unlisted UI string: '+k);
}
fs.writeFileSync('translations/COVERAGE.md','# Translation coverage\n\nFormat: [omarchy-i18n](https://github.com/avillagran/omarchy-i18n/tree/5df6970583f9987369d11f15bec1b6f573025fb1).\nEnglish source keys are stable, with named placeholders and full/base/English fallback.\n\n'+Object.entries(catalog).map(([l,c])=>'- '+l+': '+Object.keys(c).length+'/'+keys.length+' keys').join('\n')+'\n\nAll non-English values are machine-authored drafts, not human-approved translations.\nIntentional identical values include placeholder-only templates, the technical 5H window,\nand common borrowed terms such as Offline, Project, Status and Minimal where natural.\nCommands, paths, names, model identifiers and user/provider content remain data.\nThe validator is copied from the pinned guide revision; no translation service is used.\n');
console.log('Translation checks passed: '+Object.keys(catalog).length+' locales × '+keys.length+' keys; no warnings');
