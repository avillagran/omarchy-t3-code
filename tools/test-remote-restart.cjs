#!/usr/bin/env node
// Run the production remote restart in a private fake remote home. No real SSH/systemd.
const fs=require('fs'),path=require('path'),os=require('os'),assert=require('assert'),{spawnSync}=require('child_process');
const root=fs.mkdtempSync(path.join(os.tmpdir(),'t3-restart-test-'));const home=path.join(root,'home'),bin=path.join(root,'bin'),base=path.join(home,'.t3'),unit=path.join(home,'.config/systemd/user/t3code.service'),log=path.join(root,'log'),running=path.join(root,'running');
fs.mkdirSync(path.join(base,'userdata'),{recursive:true});fs.mkdirSync(path.join(base,'bin'));fs.mkdirSync(bin);fs.mkdirSync(path.dirname(unit),{recursive:true});fs.writeFileSync(path.join(base,'userdata/environment-id'),'fixture-id');
const stub='#!'+process.execPath+'\n'+`const fs=require('fs');let a=process.argv.slice(2);fs.appendFileSync(${JSON.stringify(log)},a.join(' ')+'\\n');if(process.env.T3_TEST_FAIL&&a.includes(process.env.T3_TEST_FAIL))process.exit(1);if(a[0]==='__ssh-helper'&&a[1]==='runtime-port'){if(!fs.existsSync(${JSON.stringify(running)}))process.exit(1);process.stdout.write('123 8643')}else if(a.includes('restart')||a[0]==='service'){fs.writeFileSync(${JSON.stringify(running)},'')}else if(a.includes('kill')){try{fs.unlinkSync(${JSON.stringify(running)})}catch{}}`;
fs.writeFileSync(path.join(base,'bin/t3'),stub,{mode:0o700});fs.writeFileSync(path.join(bin,'systemctl'),stub,{mode:0o700});
function run(id='fixture-id',extra={}){fs.writeFileSync(log,'');const r=spawnSync('sh',['-s','--',id],{input:fs.readFileSync(path.join(__dirname,'../src/remote-restart.sh')),env:Object.assign({},process.env,{HOME:home,PATH:bin+':'+process.env.PATH},extra),encoding:'utf8',timeout:90000});return Object.assign(r,{log:fs.readFileSync(log,'utf8')})}
try{
 fs.writeFileSync(unit,'Environment=T3CODE_HOME='+base+'\n');fs.writeFileSync(running,'');fs.writeFileSync(path.join(base,'userdata/server-runtime.json'),'{"port":8643}');
 let r=run();assert.equal(r.status,0,r.stderr);const lines=r.log.split('\n');
 const kill=lines.findIndex(l=>l==='--user kill --signal=KILL t3code.service'),restart=lines.findIndex(l=>l==='--user restart t3code.service');
 assert(kill>=0&&restart>kill,'kills the whole service first, then restarts it: '+r.log);assert.equal(r.stdout.trim(),'restarted 8643');
 assert(!fs.existsSync(path.join(base,'userdata/server-runtime.json')),'a stale runtime file is not trusted');
 r=run('wrong-id');assert.equal(r.status,7);assert.equal(r.log,'');
 fs.writeFileSync(unit,'Environment=T3CODE_HOME=/other-profile\n');r=run();assert.equal(r.status,8);assert(!r.log.includes('kill'),'never kills another profile');
 fs.writeFileSync(unit,'Environment=T3CODE_HOME='+base+'\n');r=run('fixture-id',{T3_TEST_FAIL:'restart'});assert.equal(r.status,9);
 fs.unlinkSync(unit);r=run();assert.equal(r.status,0,r.stderr);assert(r.log.includes('service install --base-dir '+base),'no service: installs and starts it');
 console.log('REMOTE_RESTART_PASS: service killed then restarted and awaited; wrong environment and other profiles refused; start failure reported; unmanaged server reinstalled');
}finally{fs.rmSync(root,{recursive:true,force:true})}
