#!/usr/bin/env node
// Exercise the production Rust action path without touching the user's clipboard.
const fs=require('fs'),os=require('os'),path=require('path'),crypto=require('crypto'),{spawn}=require('child_process'),assert=require('assert');
const repo=path.resolve(__dirname,'..'),root=fs.mkdtempSync(path.join(os.tmpdir(),'t3-copy-test-'));fs.chmodSync(root,0o700);
const bin=path.join(root,'bin');fs.mkdirSync(bin);const offer=path.join(root,'offer'),type=path.join(root,'type'),fail=path.join(root,'fail');
fs.writeFileSync(path.join(bin,'wl-copy'),`#!${process.execPath}
const fs=require('fs');if(fs.existsSync(${JSON.stringify(fail)}))process.exit(1);let chunks=[];process.stdin.on('data',c=>chunks.push(c));process.stdin.on('end',()=>{fs.writeFileSync(${JSON.stringify(offer)},Buffer.concat(chunks));fs.writeFileSync(${JSON.stringify(type)},process.argv.at(-1));});
`,{mode:0o700});
const cache=path.join(root,'cache','omarchy-t3-code','images');fs.mkdirSync(cache,{recursive:true,mode:0o700});const png=fs.readFileSync(path.join(repo,'tests/fixtures/image.png'));
fs.writeFileSync(path.join(cache,crypto.createHash('sha256').update('fixture//image/').digest('hex')+'.png'),png,{mode:0o600});
const env={...process.env,HOME:root,PATH:bin+':'+process.env.PATH,XDG_DATA_HOME:path.join(root,'data'),XDG_STATE_HOME:path.join(root,'state'),XDG_CONFIG_HOME:path.join(root,'config'),XDG_CACHE_HOME:path.join(root,'cache'),T3CODE_HOME:path.join(root,'native')};delete env.OMARCHY_T3_FIXTURE_ENVIRONMENTS;delete env.KIMI_CODING_API_KEY;
const child=spawn(path.join(repo,'target/release/omarchy-t3-client'),['watch'],{env,stdio:['pipe','pipe','pipe']});let pending=new Map(),buffer='';child.stderr.resume();child.stdout.on('data',c=>{buffer+=c;let at;while((at=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,at);buffer=buffer.slice(at+1);let e;try{e=JSON.parse(line)}catch{continue}if(e.type==='action'&&e.data.op==='copy'){const p=pending.get(e.data.requestId);if(p){pending.delete(e.data.requestId);p(e.data)}}}});
let serial=0;function copy(input){return new Promise((resolve,reject)=>{const id='fixture-'+(++serial),timer=setTimeout(()=>{pending.delete(id);reject(Error('copy timed out'))},5000);pending.set(id,v=>{clearTimeout(timer);resolve(v)});child.stdin.write(JSON.stringify({op:'copy',requestId:id,...input})+'\n')})}
(async()=>{try{
 const text='**Literal Markdown**\n<b>HTML-looking source</b> $HOME `command` á';let result=await copy({kind:'text',text});assert(!result.error);assert.equal(fs.readFileSync(offer,'utf8'),text);assert.equal(fs.readFileSync(type,'utf8'),'text/plain;charset=utf-8');
 result=await copy({kind:'image',environment:'fixture',attachment:{id:'image',name:'fixture.png',mimeType:'image/png'}});assert(!result.error,JSON.stringify(result));assert(fs.readFileSync(offer).equals(png));assert.equal(fs.readFileSync(type,'utf8'),'image/png');
 fs.writeFileSync(fail,'');result=await copy({kind:'text',text:'must report failure'});assert.equal(result.error,'clipboard_unavailable');
 console.log('COPY_PROCESS_PASS: exact UTF-8/Markdown, cached native attachment bytes with image/png, clipboard failure surfaced; real clipboard untouched');
 }finally{child.kill('SIGTERM');await new Promise(r=>child.once('exit',r));fs.rmSync(root,{recursive:true,force:true})}})().catch(e=>{console.error(e);process.exitCode=1});
