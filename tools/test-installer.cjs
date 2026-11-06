#!/usr/bin/env node
// Installer workflow tests. All OS/package/Git/service commands are private stubs.
const fs=require('fs'),os=require('os'),path=require('path'),assert=require('assert'),{spawnSync}=require('child_process');
const repo=path.resolve(__dirname,'..'),root=fs.mkdtempSync(path.join(os.tmpdir(),'t3-installer-test-'));fs.chmodSync(root,0o700);
const bin=path.join(root,'bin'),home=path.join(root,'home'),log=path.join(root,'calls');fs.mkdirSync(bin);fs.mkdirSync(home);fs.mkdirSync(path.join(home,'.t3','userdata'),{recursive:true});fs.mkdirSync(path.join(home,'.t3','bin'),{recursive:true});fs.writeFileSync(path.join(home,'.t3','bin','t3'),'#!/bin/sh\nexit 1\n',{mode:0o700});
fs.mkdirSync(path.join(home,'.config','hypr'),{recursive:true});const bindings=path.join(home,'.config','hypr','bindings.lua');fs.writeFileSync(bindings,'-- existing user setting\n');
const body=`const fs=require('fs'),path=require('path');const name=path.basename(process.argv[1]),args=process.argv.slice(2),root=process.env.INSTALL_TEST_ROOT;fs.appendFileSync(path.join(root,'calls'),JSON.stringify({name,args})+'\\n');
if(name==='pacman')process.exit(args[1]==='wl-clipboard'?1:0);
if(name==='git'){
 if(args[0]==='clone'){const dir=args.at(-1);fs.mkdirSync(path.join(dir,'.git'));fs.mkdirSync(path.join(dir,'bin'));fs.mkdirSync(path.join(dir,'ui','qml'),{recursive:true});fs.copyFileSync(path.join(process.env.INSTALL_TEST_REPO,'ui','qml','T3TerminalViewport.qml'),path.join(dir,'ui','qml','T3TerminalViewport.qml'));fs.mkdirSync(path.join(dir,'integrations','hyprland'),{recursive:true});fs.copyFileSync(path.join(process.env.INSTALL_TEST_REPO,'manifest.json'),path.join(dir,'manifest.json'));fs.copyFileSync(path.join(process.env.INSTALL_TEST_REPO,'integrations','hyprland','bindings.lua'),path.join(dir,'integrations','hyprland','bindings.lua'));for(const name of ['build-client','ot3','o3'])fs.copyFileSync(path.join(root,'bin',name),path.join(dir,'bin',name));for(const name of ['fetch-omabridge','fetch-o3-server'])fs.copyFileSync(path.join(process.env.INSTALL_TEST_REPO,'bin',name),path.join(dir,'bin',name));fs.mkdirSync(path.join(dir,'integrations','omabridge'));fs.copyFileSync(path.join(root,'omabridge-release'),path.join(dir,'integrations','omabridge','release'));fs.mkdirSync(path.join(dir,'integrations','o3-server'));fs.copyFileSync(path.join(root,'o3-server-release'),path.join(dir,'integrations','o3-server','release'));}
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
 const out=args.indexOf('-o'),url=args.find(a=>a.startsWith('https://'))||'';
 if(out<0)console.log(JSON.stringify([{tag_name:'v0.3.9',draft:true},{tag_name:'v0.2.30',draft:false},{tag_name:'v0.3.1',draft:false,prerelease:true},{tag_name:'v0.2.1',draft:false}]));
 else {const name=args[out+1].split('/').at(-1);fs.copyFileSync(path.join(root,'assets',url.includes('/o3-server/')?'o3':'',name),args[out+1]);}
}
if(name==='build-client'){const dir=path.dirname(process.argv[1]);fs.mkdirSync(path.join(dir,'qml','O3','Terminal'),{recursive:true});fs.writeFileSync(path.join(dir,'qml','O3','Terminal','libo3terminal.so'),'fixture-terminal');for(const name of ['Ghostty-LICENSE','GHOSTTY_VERSION'])fs.writeFileSync(path.join(dir,'qml','O3','Terminal',name),'fixture-engine');fs.writeFileSync(path.join(dir,'qml','O3','Terminal','qmldir'),'module O3.Terminal\\nplugin o3terminal\\n');fs.copyFileSync(path.join(root,'bin','omarchy-t3-client'),path.join(dir,'omarchy-t3-client'));for(const file of ['libt3-document.so','libt3-qml-preview.so','libt3-browser.so','t3-browser-secret'])fs.writeFileSync(path.join(dir,file),'fixture');}
if(name==='omarchy-t3-client'){if(args[0]==='backend-status')console.log(JSON.stringify({owner:process.env.INSTALL_TEST_OWNER||'stopped'}));else console.log(JSON.stringify({status:'starting'}));}
`;
for(const name of ['systemctl','pacman','sudo','git','cargo','rustup','omarchy','omarchy-shell','hyprctl','build-client','omarchy-t3-client','ot3','o3','curl','ldd','uname'])fs.writeFileSync(path.join(bin,name),'#!'+process.execPath+'\n'+body,{mode:0o700});
const base={...process.env,HOME:home,XDG_CONFIG_HOME:path.join(home,'.config'),PATH:bin+':'+process.env.PATH,T3CODE_HOME:path.join(home,'.t3'),INSTALL_TEST_ROOT:root,INSTALL_TEST_REPO:repo};
function run(extra={}){fs.writeFileSync(log,'');const result=spawnSync('bash',[path.join(repo,'install.sh')],{env:{...base,...extra},encoding:'utf8'});return {result,calls:fs.readFileSync(log,'utf8').trim().split('\n').filter(Boolean).map(JSON.parse)}}
function called(calls,name,...parts){return calls.some(c=>c.name===name&&parts.every(p=>c.args.includes(p)))}
try{
 fs.mkdirSync(path.join(root,'assets'));fs.mkdirSync(path.join(root,'payload','bin'),{recursive:true});
 fs.copyFileSync(path.join(bin,'omarchy-t3-client'),path.join(root,'payload','bin','omarchy-t3-client'));for(const file of ['libt3-document.so','libt3-qml-preview.so','libt3-browser.so','t3-browser-secret'])fs.writeFileSync(path.join(root,'payload','bin',file),'fixture');
 const terminalPayload=path.join(root,'payload','bin','qml','O3','Terminal');fs.mkdirSync(terminalPayload,{recursive:true});fs.writeFileSync(path.join(terminalPayload,'libo3terminal.so'),'fixture-terminal');for(const name of ['Ghostty-LICENSE','GHOSTTY_VERSION'])fs.writeFileSync(path.join(terminalPayload,name),'fixture-engine');fs.writeFileSync(path.join(terminalPayload,'qmldir'),'module O3.Terminal\nplugin o3terminal\n');
 const asset='omarchy-t3-code-v0.3.1-linux-aarch64.tar.gz';
 function pack(commit='fixture-commit'){fs.writeFileSync(path.join(root,'payload','COMMIT'),commit);assert.equal(spawnSync('tar',['-czf',path.join(root,'assets',asset),'-C',path.join(root,'payload'),'bin','COMMIT']).status,0);const hash=require('crypto').createHash('sha256').update(fs.readFileSync(path.join(root,'assets',asset))).digest('hex');fs.writeFileSync(path.join(root,'assets',asset+'.sha256'),hash+'  '+asset+'\n');}
 pack();
 // Bundled OmaBridge: a fixture release, pinned by hash and listed in SHA256SUMS.
 const bridgeAsset='omabridge-v0.1.0-aarch64-linux.tar.gz',bridgeDir=path.join(root,'bridge','omabridge-v0.1.0-aarch64-linux');fs.mkdirSync(bridgeDir,{recursive:true});
 fs.writeFileSync(path.join(bridgeDir,'omabridge'),'#!/bin/sh\necho "omabridge 0.1.0"\n',{mode:0o755});fs.writeFileSync(path.join(bridgeDir,'omabridge-gui'),'gui');
 assert.equal(spawnSync('tar',['-czf',path.join(root,'assets',bridgeAsset),'-C',path.join(root,'bridge'),'omabridge-v0.1.0-aarch64-linux']).status,0);
 const bridgeHash=require('crypto').createHash('sha256').update(fs.readFileSync(path.join(root,'assets',bridgeAsset))).digest('hex');
 const installed=()=>path.join(home,'.config','omarchy','plugins','omarchy-t3-code','bin','omabridge');
 function pin(hash=bridgeHash,listed=bridgeHash){for(const file of [path.join(root,'omabridge-release'),path.join(path.dirname(installed()),'..','integrations','omabridge','release')])if(fs.existsSync(path.dirname(file)))fs.writeFileSync(file,`version=v0.1.0\nsha256_aarch64=${hash}\n`);fs.writeFileSync(path.join(root,'assets','SHA256SUMS'),`${listed}  ${bridgeAsset}\n`);}
 pin();
 const o3Asset='o3-server-v0.1.0-aarch64-linux.tar.gz';fs.mkdirSync(path.join(root,'assets','o3'));
 function o3Release(version='0.1.0',hash,listed){const name=`o3-server-v${version}-aarch64-linux`,dir=path.join(root,'o3',name);fs.mkdirSync(dir,{recursive:true});fs.writeFileSync(path.join(dir,'o3-server'),`#!/bin/sh\necho "o3-server ${version}"\n`,{mode:0o755});assert.equal(spawnSync('tar',['-czf',path.join(root,'assets','o3',name+'.tar.gz'),'-C',path.join(root,'o3'),name]).status,0);const real=require('crypto').createHash('sha256').update(fs.readFileSync(path.join(root,'assets','o3',name+'.tar.gz'))).digest('hex');fs.writeFileSync(path.join(root,'assets','o3','SHA256SUMS'),`${listed||real}  ${name}.tar.gz\n`);fs.writeFileSync(path.join(root,'o3-server-release'),`version=v${version}\nsha256_aarch64=${hash||real}\n`);return real;}
 o3Release();
 let {result,calls}=run();assert(!called(calls,'curl',`https://github.com/avillagran/o3-server/releases/download/v0.1.0/${o3Asset}`),'a T3 setup keeps T3');assert(!fs.existsSync(path.join(home,'.config','omarchy-t3-code','backends.json')));assert(result.stdout.includes('Local server: T3 Code nightly.'));assert(fs.existsSync(installed()),'bundled OmaBridge installed');assert.equal(fs.statSync(installed()).mode&0o777,0o755);assert(called(calls,'curl',`https://github.com/avillagran/omabridge/releases/download/v0.1.0/${bridgeAsset}`));assert.equal(result.status,0,result.stderr);const installedTerminal=path.join(path.dirname(installed()),'qml','O3','Terminal');assert.equal(fs.readFileSync(path.join(installedTerminal,'libo3terminal.so'),'utf8'),'fixture-terminal');assert.equal(fs.statSync(path.join(installedTerminal,'qmldir')).mode&0o777,0o644);for(const name of ['Ghostty-LICENSE','GHOSTTY_VERSION'])assert.equal(fs.readFileSync(path.join(installedTerminal,name),'utf8'),'fixture-engine');assert(called(calls,'ldd',path.join(calls.find(c=>c.name==='ldd').args.find(a=>a.endsWith('omarchy-t3-client')),'..','qml','O3','Terminal','libo3terminal.so')));assert(called(calls,'sudo','pacman','wl-clipboard'));assert(called(calls,'git','clone'));assert(!called(calls,'build-client'));assert(!called(calls,'cargo'));assert(!called(calls,'rustup'));assert(called(calls,'omarchy','validate'));assert(called(calls,'omarchy','enable'));assert(called(calls,'o3','install-desktop'));assert(called(calls,'o3','restart'));assert(called(calls,'omarchy-t3-client','backend-control','start'));assert(called(calls,'omarchy','restart','shell'));assert(fs.readFileSync(bindings,'utf8').startsWith('-- existing user setting\n'));assert.equal((fs.readFileSync(bindings,'utf8').match(/dofile/g)||[]).length,1);assert.equal(fs.readdirSync(path.dirname(bindings)).filter(f=>f.includes('.t3-backup-')).length,1);
 ({result,calls}=run({INSTALL_TEST_OWNER:'gui'}));assert.equal(result.status,0,result.stderr);assert(called(calls,'git','fetch','v0.3.1'));assert(called(calls,'git','merge','--ff-only'));assert(!called(calls,'omarchy-t3-client','backend-control'));assert.equal((fs.readFileSync(bindings,'utf8').match(/dofile/g)||[]).length,1);
 ({result,calls}=run({INSTALL_TEST_REWRITTEN:'same-tree'}));assert.equal(result.status,0,result.stderr);assert(called(calls,'git','reset','--hard','rewritten-commit'));assert(called(calls,'git','merge','--ff-only'));
 ({result,calls}=run({INSTALL_TEST_REWRITTEN:'diverged'}));assert.notEqual(result.status,0);assert(result.stderr.includes('diverges'));assert(!called(calls,'git','reset'));assert(!called(calls,'omarchy','restart'));
 ({result,calls}=run({INSTALL_TEST_DIRTY:'1'}));assert.notEqual(result.status,0);assert(result.stderr.includes('local changes'));assert(!called(calls,'build-client'));assert(!called(calls,'git','fetch'));
 ({result,calls}=run({OT3_BUILD_FROM_SOURCE:'1'}));assert.equal(result.status,0,result.stderr);assert(called(calls,'build-client'));
 // Already current: no download. A hash that disagrees with the pin is refused, but ot3 still installs.
 ({result,calls}=run());assert.equal(result.status,0,result.stderr);assert(!called(calls,'curl',`https://github.com/avillagran/omabridge/releases/download/v0.1.0/${bridgeAsset}`));
 fs.rmSync(installed());pin('0'.repeat(64));({result,calls}=run());assert.equal(result.status,0,result.stderr);assert(result.stderr.includes('does not match the pinned hash'));assert(!fs.existsSync(installed()));
 pin(bridgeHash,'1'.repeat(64));({result,calls}=run());assert.equal(result.status,0,result.stderr);assert(!fs.existsSync(installed()));pin();
 // A device published with the bundled binary is restarted on it; another unit, or none, leaves systemd alone.
 assert(!called(calls,'systemctl'));const unit=path.join(home,'.config','systemd','user','omabridge-serve.service');fs.mkdirSync(path.dirname(unit),{recursive:true});fs.writeFileSync(unit,'ExecStart=/usr/bin/omabridge serve\n');
 ({result,calls}=run());assert.equal(result.status,0,result.stderr);assert(!called(calls,'systemctl'));
 fs.writeFileSync(unit,`ExecStart="${path.dirname(installed())}/omabridge" manage-serve\n`);
 ({result,calls}=run());assert.equal(result.status,0,result.stderr);assert(called(calls,'systemctl','--user','try-restart','omabridge-serve.service'));fs.rmSync(unit);
 pack('wrong-commit');({result,calls}=run());assert.notEqual(result.status,0);assert(result.stderr.includes('revisions differ'));assert(!called(calls,'omarchy','restart'));
 pack();fs.appendFileSync(path.join(root,'assets',asset),'corruption');({result,calls}=run());assert.notEqual(result.status,0);assert(result.stderr.includes('checksum failed'));
 pack();({result,calls}=run({INSTALL_TEST_LIBS:'1'}));assert.notEqual(result.status,0);assert(result.stderr.includes('Runtime libraries'));assert(!called(calls,'omarchy','restart'));
 // O3 Server: a new device without T3. No T3 package or window; the pinned server, its unit and the registry.
 const home2=path.join(root,'home-o3'),cfg2=path.join(home2,'.config');fs.mkdirSync(path.join(cfg2,'hypr'),{recursive:true});fs.writeFileSync(path.join(cfg2,'hypr','bindings.lua'),'-- existing\n');
 const o3env={HOME:home2,XDG_CONFIG_HOME:cfg2,T3CODE_HOME:path.join(home2,'.t3')};
 const o3bin=path.join(home2,'.local','share','o3-server','bin','o3-server'),o3unit=path.join(cfg2,'systemd','user','o3-server.service'),registry=path.join(cfg2,'omarchy-t3-code','backends.json'),o3pin=path.join(cfg2,'omarchy','plugins','omarchy-t3-code','integrations','o3-server','release');
 const o3url=v=>`https://github.com/avillagran/o3-server/releases/download/v${v}/o3-server-v${v}-aarch64-linux.tar.gz`;
 function runNoTty(extra={}){fs.writeFileSync(log,'');const result=spawnSync('setsid',['-w','bash',path.join(repo,'install.sh')],{env:{...base,...extra},encoding:'utf8',stdio:['ignore','pipe','pipe']});return {result,calls:fs.readFileSync(log,'utf8').trim().split('\n').filter(Boolean).map(JSON.parse)}}
 ({result,calls}=run({...o3env,O3_SERVER:'bad'}));assert.notEqual(result.status,0);assert(result.stderr.includes('O3_SERVER must be o3 or t3'));assert(!called(calls,'git','clone'));
 const hasT3=spawnSync('bash',['-c','command -v t3code-nightly || command -v t3code']).status===0;
 ({result,calls}=hasT3?run({...o3env,O3_SERVER:'o3'}):runNoTty(o3env));assert.equal(result.status,0,result.stderr);
 assert(result.stdout.includes('Local server: O3 Server.'),'no terminal and no T3: O3 Server by default');assert(!called(calls,'sudo','pacman','t3code-nightly-bin'));assert(!called(calls,'yay'));assert(!fs.existsSync(path.join(home2,'.t3')));
 assert(called(calls,'curl',o3url('0.1.0')));assert.equal(spawnSync(o3bin,['--version'],{encoding:'utf8'}).stdout.trim(),'o3-server 0.1.0');
 const unitText=fs.readFileSync(o3unit,'utf8');assert(unitText.startsWith('# Written by the O3 Code installer.\n'));const profile2=path.join(home2,'.local','share','o3-server','profile');
 assert(unitText.includes(`Environment="T3CODE_HOME=${profile2}"`));assert(unitText.includes(`ExecStart="${o3bin}" serve --no-bridge --base-dir "${profile2}"`));assert(!unitText.includes('--host'),'loopback only: no port is exposed');assert(fs.existsSync(profile2));
 assert(called(calls,'systemctl','--user','daemon-reload'));assert(called(calls,'systemctl','--user','enable','o3-server.service'));
 assert.deepEqual(JSON.parse(fs.readFileSync(registry,'utf8')),{version:1,default:'o3',servers:[{id:'o3',unit:'o3-server.service',profile:profile2,cli:o3bin}]});
 assert(called(calls,'omarchy-t3-client','backend-control','start'));assert(called(calls,'o3','install-desktop'));assert(result.stdout.includes('Installed with O3 Server.'));
 // Repeat: the registry keeps the choice; nothing is downloaded, enabled or restarted again.
 const kept=fs.readFileSync(registry,'utf8');
 ({result,calls}=run(o3env));assert.equal(result.status,0,result.stderr);assert(result.stdout.includes('Local server: O3 Server.'));assert(!called(calls,'curl',o3url('0.1.0')));assert(!called(calls,'systemctl','--user','enable','o3-server.service'));assert(!called(calls,'systemctl','--user','try-restart','o3-server.service'));assert.equal(fs.readFileSync(registry,'utf8'),kept);
 // A new pinned version replaces the binary and restarts the installer's unit.
 o3Release('0.1.1');fs.copyFileSync(path.join(root,'o3-server-release'),o3pin);
 ({result,calls}=run(o3env));assert.equal(result.status,0,result.stderr);assert(called(calls,'curl',o3url('0.1.1')));assert(called(calls,'systemctl','--user','try-restart','o3-server.service'));assert.equal(spawnSync(o3bin,['--version'],{encoding:'utf8'}).stdout.trim(),'o3-server 0.1.1');
 // A unit written elsewhere is never rewritten or restarted.
 fs.writeFileSync(o3unit,'[Service]\nExecStart=/custom/o3-server serve\n');o3Release('0.1.2');fs.copyFileSync(path.join(root,'o3-server-release'),o3pin);
 ({result,calls}=run(o3env));assert.equal(result.status,0,result.stderr);assert(!called(calls,'systemctl','--user','try-restart','o3-server.service'));assert.equal(fs.readFileSync(o3unit,'utf8'),'[Service]\nExecStart=/custom/o3-server serve\n');
 // A hash that disagrees with the pin stops the install: there would be no local server.
 o3Release('0.1.3','0'.repeat(64));fs.copyFileSync(path.join(root,'o3-server-release'),o3pin);
 ({result,calls}=run(o3env));assert.notEqual(result.status,0);assert(result.stderr.includes('does not match the pinned hash'));assert(result.stderr.includes('O3 Server was not installed'));assert.equal(spawnSync(o3bin,['--version'],{encoding:'utf8'}).stdout.trim(),'o3-server 0.1.2');assert(!called(calls,'omarchy','restart','shell'));
 // A registry written by the user without O3 Server is not rewritten.
 fs.writeFileSync(registry,'{"version":1,"default":"t3","servers":[{"id":"t3","unit":"t3code.service","profile":"/p","cli":"/c"}]}');
 ({result,calls}=run({...o3env,O3_SERVER:'o3'}));assert.notEqual(result.status,0);assert(result.stderr.includes('does not register O3 Server'));assert(!called(calls,'git','fetch'));
 // OT3_RELEASE installs exactly that tag; anything else is refused.
 ({result,calls}=run({OT3_RELEASE:'v0.3.1'}));assert.equal(result.status,0,result.stderr);assert(!called(calls,'curl','https://api.github.com/repos/avillagran/omarchy-t3-code/releases?per_page=50'));
 ({result,calls}=run({OT3_RELEASE:'main;rm'}));assert.notEqual(result.status,0);assert(result.stderr.includes('OT3_RELEASE must be a release tag'));
 console.log('INSTALLER_PASS: dependencies, verified prebuilt install/enable and explicit source build, checksum/revision/library rejection, headless start, server choice (T3 kept, O3 Server by default without a terminal, pinned O3 Server install/update/restart, foreign unit and registry preserved, hash refusal), O3 release lookup and OT3_RELEASE, bundled OmaBridge pinned by hash, optional and restarted when published, preserved GUI server, repeat-safe binding/backup, rewritten-history migration and dirty-checkout refusal; no real system changes');
}finally{console.log('Installer fixture directory: '+root)}
