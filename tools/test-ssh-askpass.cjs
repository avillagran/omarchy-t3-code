#!/usr/bin/env node
// Synthetic credentials only: verify masking, prompt escaping and cancellation.
const fs=require('node:fs'),os=require('node:os'),path=require('node:path'),cp=require('node:child_process'),assert=require('node:assert/strict');
const dir=fs.mkdtempSync(path.join(os.tmpdir(),'ot3-ssh-askpass-'));
try {
 const fake=path.join(dir,'zenity'),args=path.join(dir,'args');
 fs.writeFileSync(fake,'#!/bin/sh\nprintf "%s\\n" "$@" > "$OT3_TEST_ARGS"\nprintf "%s" fixture-answer\n',{mode:0o755});
 const env={...process.env,PATH:dir+':/usr/bin:/bin',OT3_TEST_ARGS:args};
 const helper=path.resolve(__dirname,'../bin/ot3-ssh-askpass');
 assert.equal(cp.execFileSync('bash',[helper,'<host> & verification code:'],{env,encoding:'utf8'}),'fixture-answer');
 const flags=fs.readFileSync(args,'utf8');assert(flags.includes('--hide-text'));assert(flags.includes('--text=&lt;host&gt; &amp; verification code:'));assert(!flags.includes('fixture-answer'));
 fs.writeFileSync(fake,'#!/bin/sh\nexit 1\n',{mode:0o755});
 const cancel=cp.spawnSync('bash',[helper,'Password:'],{env,encoding:'utf8'});assert.equal(cancel.status,1);assert.equal(cancel.stdout,'');
 console.log('SSH askpass checks passed: masked entry, escaped prompts and cancellation; no credential persistence.');
} finally {fs.rmSync(dir,{recursive:true,force:true});}
