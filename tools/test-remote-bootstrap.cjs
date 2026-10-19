#!/usr/bin/env node
// Run the production SSH bootstrap in a private fake remote home. No real SSH/systemd.
const fs=require('fs'),path=require('path'),os=require('os'),assert=require('assert'),{spawnSync}=require('child_process');
const root=fs.mkdtempSync(path.join(os.tmpdir(),'t3-remote-test-'));fs.chmodSync(root,0o700);const home=path.join(root,'home'),bin=path.join(root,'bin'),base=path.join(home,'.t3'),unit=path.join(home,'.config/systemd/user/t3code.service'),log=path.join(root,'calls'),running=path.join(root,'running');
fs.mkdirSync(path.join(base,'userdata'),{recursive:true});fs.mkdirSync(path.join(base,'bin'));fs.mkdirSync(bin);fs.mkdirSync(path.dirname(unit),{recursive:true});fs.writeFileSync(path.join(base,'userdata/environment-id'),'fixture-id');fs.writeFileSync(path.join(base,'userdata/server-runtime.json'),'{}');
const stub='#!'+process.execPath+'\n'+`const fs=require('fs');let a=process.argv.slice(2);fs.appendFileSync(${JSON.stringify(log)},a.join(' ')+'\\n');if(process.env.T3_TEST_FAIL&&a.includes(process.env.T3_TEST_FAIL))process.exit(1);if(a[0]==='__ssh-helper'&&a[1]==='runtime-port'){if(!fs.existsSync(${JSON.stringify(running)}))process.exit(1);process.stdout.write('123 3773')}else if(a[0]==='service'||a[0]==='--user'){fs.writeFileSync(${JSON.stringify(running)},'')}else if(a[0]==='auth'){console.log('fixture-token')}`;
fs.writeFileSync(path.join(base,'bin/t3'),stub,{mode:0o700});fs.writeFileSync(path.join(bin,'systemctl'),stub,{mode:0o700});
function run(id='fixture-id',extra={},client='0123456789abcdef'){fs.writeFileSync(log,'');const r=spawnSync('sh',['-s','--',id,client],{input:fs.readFileSync(path.join(__dirname,'../src/remote-bootstrap.sh')),env:{...process.env,HOME:home,PATH:bin+':'+process.env.PATH,...extra},encoding:'utf8'});return {...r,log:fs.readFileSync(log,'utf8')}}
try{
 fs.writeFileSync(running,'');let r=run();assert.equal(r.status,0,r.stderr);assert(!r.log.includes('service install')&&!r.log.includes('--user start'));assert(r.stdout.includes('T3_AUTH'));
 fs.unlinkSync(running);fs.writeFileSync(unit,'Environment=T3CODE_HOME='+base+'\n');r=run();assert.equal(r.status,0,r.stderr);assert(r.log.includes('--user start t3code.service'));assert(r.log.includes('wait-ready 3773'));
 fs.unlinkSync(running);fs.unlinkSync(unit);r=run();assert.equal(r.status,0,r.stderr);assert(r.log.includes('service install --base-dir '+base));
 fs.unlinkSync(running);fs.writeFileSync(unit,'Environment=T3CODE_HOME=/other-profile\n');r=run();assert.equal(r.status,8);assert(!r.log.includes('--user start'));
 r=run('wrong-id');assert.equal(r.status,7);assert.equal(r.log,'');
 fs.writeFileSync(running,'');r=run('fixture-id',{T3_TEST_FAIL:'wait-ready'});assert.equal(r.status,9);assert(!r.log.includes('auth session'),'never issue token before readiness');
 fs.unlinkSync(running);fs.writeFileSync(unit,'Environment=T3CODE_HOME='+base+'\n');r=run('fixture-id',{T3_TEST_FAIL:'start'});assert.equal(r.status,9);assert(!r.log.includes('auth session'));
 fs.mkdirSync(path.join(home,'.local/bin'),{recursive:true});fs.renameSync(path.join(base,'bin/t3'),path.join(home,'.local/bin/t3'));fs.writeFileSync(running,'');r=run();assert.equal(r.status,0,r.stderr);
 // Sessions carry a per-device subject; malformed ids and older CLIs fall back safely.
 fs.writeFileSync(running,'');r=run();assert.equal(r.status,0,r.stderr);assert(r.log.includes('auth session issue --base-dir '+base+' --subject omarchy-t3-code:0123456789abcdef '));assert(r.stdout.trim().endsWith('fixture-token'));
 r=run('fixture-id',{},"x'; rm -rf /");assert.equal(r.status,0,r.stderr);assert(r.log.includes('--subject omarchy-t3-code --scope'));
 r=run('fixture-id',{T3_TEST_FAIL:'--subject'});assert.equal(r.status,0,r.stderr);assert.equal((r.log.match(/auth session issue/g)||[]).length,2);assert(r.stdout.trim().endsWith('fixture-token'));
 console.log('REMOTE_BOOTSTRAP_PASS: running server reused; stopped native service started; absent service installed; readiness checked; profile/environment mismatch and readiness/start failures rejected; local-bin CLI fallback; per-device session subject with safe fallbacks');
}finally{fs.rmSync(root,{recursive:true,force:true})}
