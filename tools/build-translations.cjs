#!/usr/bin/env node
// Build-time only. Machine-authored batches; flat English-source catalogs per omarchy-i18n.
const fs=require('fs');const source=JSON.parse(fs.readFileSync('translations/source.json')).en;
const languages=['es','pt','fr','de','it','nl','pl','ru','uk','ja','ko','zh','ar','tr','sv','da','no','fi','cs'];
const keys=Object.keys(source),out={en:source};languages.forEach(l=>out[l]={});
const stable=['5H {percent}%','{activity} ({count})','{activity} · {title}','{shortcut} · {action}'];
for(const key of stable)for(const locale of languages)out[locale][key]=key;
for(const f of fs.readdirSync('translations').filter(f=>/^batch-.*\.tsv$/.test(f)).sort()){
 for(const [n,line] of fs.readFileSync('translations/'+f,'utf8').trim().split('\n').entries()){
  const [key,...values]=line.split('|');if(values.length!==languages.length)throw Error(`${f}:${n+1}: expected ${languages.length} columns, got ${values.length}`);
  if(source[key]===undefined)throw Error('Unknown English source key '+key);
  values.forEach((value,i)=>{if(!value.trim())throw Error('Empty translation '+key);out[languages[i]][key]=value});
 }
}
for(const locale of languages){const missing=keys.filter(k=>out[locale][k]===undefined);if(missing.length)throw Error(locale+' missing '+JSON.stringify(missing));out[locale]=Object.fromEntries(keys.map(k=>[k,out[locale][k]]));}
fs.writeFileSync('i18n.json',JSON.stringify(out,null,2)+'\n');console.log(`${Object.keys(out).length} locales × ${keys.length} source strings`);
