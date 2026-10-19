#!/usr/bin/env node
// Installer workflow tests. All OS/package/Git/service commands are private stubs.
const fs=require('fs'),os=require('os'),path=require('path'),assert=require('assert'),{spawnSync}=require('child_process');
const repo=path.resolve(__dirname,'..'),root=fs.mkdtempSync(path.join(os.tmpdir(),'t3-installer-test-'));fs.chmodSync(root,0o700);
const bin=path.join(root,'bin'),home=path.join(root,'home'),log=path.join(root,'calls');fs.mkdirSync(bin);fs.mkdirSync(home);fs.mkdirSync(path.join(home,'.t3','userdata'),{recursive:true});fs.mkdirSync(path.join(home,'.t3','bin'),{recursive:true});fs.writeFileSync(path.join(home,'.t3','bin','t3'),'#!/bin/sh\nexit 1\n',{mode:0o700});
fs.mkdirSync(path.join(home,'.config','hypr'),{recursive:true});const bindings=path.join(home,'.config','hypr','bindings.lua');fs.writeFileSync(bindings,'-- existing user setting\n');
const body=`const fs=require('fs'),path=require('path');const name=path.basename(process.argv[1]),args=process.argv.slice(2),root=process.env.INSTALL_TEST_ROOT;fs.appendFileSync(path.join(root,'calls'),JSON.stringify({name,args})+'\\n');
if(name==='pacman')process.exit(args[1]==='wl-clipboard'?1:0);
if(name==='git'){
 if(args[0]==='clone'){const dir=args.at(-1);fs.mkdirSync(path.join(dir,'.git'));fs.mkdirSync(path.join(dir,'bin'));fs.mkdirSync(path.join(dir,'integrations','hyprland'),{recursive:true});fs.copyFileSync(path.join(process.env.INSTALL_TEST_REPO,'manifest.json'),path.join(dir,'manifest.json'));fs.copyFileSync(path.join(process.env.INSTALL_TEST_REPO,'integrations','hyprland','bindings.lua'),path.join(dir,'integrations','hyprland','bindings.lua'));fs.copyFileSync(path.join(root,'bin','build-client'),path.join(dir,'bin','build-client'));}
 else if(args.includes('merge-base')&&process.env.INSTALL_TEST_REWRITTEN)process.exit(1);
 else if(args.includes('log')&&process.env.INSTALL_TEST_REWRITTEN)console.log(process.env.INSTALL_TEST_REWRITTEN==='same-tree'?'rewritten-commit fixture-tree':'other-commit other-tree');
 else if(args.includes('HEAD^{tree}'))console.log('fixture-tree');
 else if(args.includes('rev-parse'))console.log('fixture-commit');
 else if(args.includes('get-url'))console.log('https://github.com/avillagran/omarchy-t3-code.git');
 else if(args.includes('status')&&process.env.INSTALL_TEST_DIRTY)console.log(' M modified.qml');
}
if(name==='uname')console.log('aarch64');
if(name==='ldd')console.log(process.env.INSTALL_TEST_LIBS ? 'libfixture => not found' : 'libfixture => /fixture');
if(name==='curl'){
 const out=args.indexOf('-o');
 if(out<0)console.log(JSON.stringify({tag_name:'v0.2.1'}));
 else {const name=args[out+1].split('/').at(-1);fs.copyFileSync(path.join(root,'assets',name),args[out+1]);}
}
if(name==='build-client'){const dir=path.dirname(process.argv[1]);fs.copyFileSync(path.join(root,'bin','omarchy-t3-client'),path.join(dir,'omarchy-t3-client'));for(const file of ['libt3-document.so','libt3-browser.so','t3-browser-secret'])fs.writeFileSync(path.join(dir,file),'fixture');}
if(name==='omarchy-t3-client'){if(args[0]==='backend-status')console.log(JSON.stringify({owner:process.env.INSTALL_TEST_OWNER||'stopped'}));else console.log(JSON.stringify({status:'starting'}));}
`;
for(const name of ['pacman','sudo','git','cargo','rustup','omarchy','omarchy-shell','hyprctl','build-client','omarchy-t3-client','curl','ldd','uname'])fs.writeFileSync(path.join(bin,name),'#!'+process.execPath+'\n'+body,{mode:0o700});
const base={...process.env,HOME:home,PATH:bin+':'+process.env.PATH,T3CODE_HOME:path.join(home,'.t3'),INSTALL_TEST_ROOT:root,INSTALL_TEST_REPO:repo};
function run(extra={}){fs.writeFileSync(log,'');const result=spawnSync('bash',[path.join(repo,'install.sh')],{env:{...base,...extra},encoding:'utf8'});return {result,calls:fs.readFileSync(log,'utf8').trim().split('\n').filter(Boolean).map(JSON.parse)}}
function called(calls,name,...parts){return calls.some(c=>c.name===name&&parts.every(p=>c.args.includes(p)))}
try{
 fs.mkdirSync(path.join(root,'assets'));fs.mkdirSync(path.join(root,'payload','bin'),{recursive:true});
 fs.copyFileSync(path.join(bin,'omarchy-t3-client'),path.join(root,'payload','bin','omarchy-t3-client'));for(const file of ['libt3-document.so','libt3-browser.so','t3-browser-secret'])fs.writeFileSync(path.join(root,'payload','bin',file),'fixture');
 const asset='omarchy-t3-code-v0.2.1-linux-aarch64.tar.gz';
 function pack(commit='fixture-commit'){fs.writeFileSync(path.join(root,'payload','COMMIT'),commit);assert.equal(spawnSync('tar',['-czf',path.join(root,'assets',asset),'-C',path.join(root,'payload'),'bin','COMMIT']).status,0);const hash=require('crypto').createHash('sha256').update(fs.readFileSync(path.join(root,'assets',asset))).digest('hex');fs.writeFileSync(path.join(root,'assets',asset+'.sha256'),hash+'  '+asset+'\n');}
 pack();
 let {result,calls}=run();assert.equal(result.status,0,result.stderr);assert(called(calls,'sudo','pacman','wl-clipboard'));assert(called(calls,'git','clone'));assert(!called(calls,'build-client'));assert(!called(calls,'cargo'));assert(!called(calls,'rustup'));assert(called(calls,'omarchy','validate'));assert(called(calls,'omarchy','enable'));assert(called(calls,'omarchy-t3-client','backend-control','start'));assert(called(calls,'omarchy','restart','shell'));assert(fs.readFileSync(bindings,'utf8').startsWith('-- existing user setting\n'));assert.equal((fs.readFileSync(bindings,'utf8').match(/dofile/g)||[]).length,1);assert.equal(fs.readdirSync(path.dirname(bindings)).filter(f=>f.includes('.t3-backup-')).length,1);
 ({result,calls}=run({INSTALL_TEST_OWNER:'gui'}));assert.equal(result.status,0,result.stderr);assert(called(calls,'git','fetch','v0.2.1'));assert(called(calls,'git','merge','--ff-only'));assert(!called(calls,'omarchy-t3-client','backend-control'));assert.equal((fs.readFileSync(bindings,'utf8').match(/dofile/g)||[]).length,1);
 ({result,calls}=run({INSTALL_TEST_REWRITTEN:'same-tree'}));assert.equal(result.status,0,result.stderr);assert(called(calls,'git','reset','--hard','rewritten-commit'));assert(called(calls,'git','merge','--ff-only'));
 ({result,calls}=run({INSTALL_TEST_REWRITTEN:'diverged'}));assert.notEqual(result.status,0);assert(result.stderr.includes('diverges'));assert(!called(calls,'git','reset'));assert(!called(calls,'omarchy','restart'));
 ({result,calls}=run({INSTALL_TEST_DIRTY:'1'}));assert.notEqual(result.status,0);assert(result.stderr.includes('local changes'));assert(!called(calls,'build-client'));assert(!called(calls,'git','fetch'));
 ({result,calls}=run({OT3_BUILD_FROM_SOURCE:'1'}));assert.equal(result.status,0,result.stderr);assert(called(calls,'build-client'));
 pack('wrong-commit');({result,calls}=run());assert.notEqual(result.status,0);assert(result.stderr.includes('revisions differ'));assert(!called(calls,'omarchy','restart'));
 pack();fs.appendFileSync(path.join(root,'assets',asset),'corruption');({result,calls}=run());assert.notEqual(result.status,0);assert(result.stderr.includes('checksum failed'));
 pack();({result,calls}=run({INSTALL_TEST_LIBS:'1'}));assert.notEqual(result.status,0);assert(result.stderr.includes('Runtime libraries'));assert(!called(calls,'omarchy','restart'));
 console.log('INSTALLER_PASS: dependencies, verified prebuilt install/enable and explicit source build, checksum/revision/library rejection, headless start, preserved GUI server, repeat-safe binding/backup, rewritten-history migration and dirty-checkout refusal; no real system changes');
}finally{fs.rmSync(root,{recursive:true,force:true})}
