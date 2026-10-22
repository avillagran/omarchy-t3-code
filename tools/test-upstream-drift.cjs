#!/usr/bin/env node
// tools/upstream-drift.cjs against a two-commit fixture repository. No network, no real T3 checkout.
const fs=require('fs'),path=require('path'),os=require('os'),assert=require('assert'),{execFileSync,spawnSync}=require('child_process');
const dir=fs.mkdtempSync(path.join(os.tmpdir(),'ot3-drift-')),repo=path.join(dir,'t3'),tool=path.join(__dirname,'upstream-drift.cjs');
const git=(...a)=>execFileSync('git',['-C',repo,...a],{encoding:'utf8',env:{...process.env,GIT_AUTHOR_NAME:'t',GIT_AUTHOR_EMAIL:'t@e',GIT_COMMITTER_NAME:'t',GIT_COMMITTER_EMAIL:'t@e'}}).trim();
const write=(f,t)=>{fs.mkdirSync(path.dirname(path.join(repo,f)),{recursive:true});fs.writeFileSync(path.join(repo,f),t)};
const contracts=(methods,settings,keys)=>{
 write('packages/contracts/src/rpc.ts',`export const WS_METHODS = {\n${methods.map((m,i)=>`  m${i}: "${m}",`).join('\n')}\n} as const;\nHttpApiEndpoint.get("shell", "/api/orchestration/shell", {});\n`);
 write('packages/contracts/src/orchestrationV2.ts','const A = Schema.Struct({ type: Schema.Literal("message.dispatch") });\n');
 write('packages/contracts/src/settings.ts',`export const ClientSettingsSchema = Schema.Struct({\n${settings.map(k=>`  ${k}: Schema.Boolean,`).join('\n')}\n});\nexport const ServerSettings = Schema.Struct({\n  enableAssistantStreaming: Schema.Boolean,\n});\n`);
 write('packages/contracts/src/keybindings.ts',`const COMMANDS = [\n${keys.map(k=>`  "${k}",`).join('\n')}\n];\n`);
};
try{
 fs.mkdirSync(repo);git('init','-q');
 contracts(['server.getConfig','projects.list','old.method'],['followUpBehavior'],['chat.new']);write('apps/web/src/components/chat/Composer.tsx','x');
 git('add','-A');git('commit','-qm','base');const base=git('rev-parse','HEAD');
 contracts(['server.getConfig','projects.list','projects.listIgnoredFiles'],['followUpBehavior','newClientToggle'],['chat.new','composer.sendBackground']);write('apps/web/src/components/chat/Composer.tsx','y');
 git('add','-A');git('commit','-qm','feat(web): composer change');
 const pin=path.join(dir,'t3code.json'),inventory=path.join(dir,'parity.json'),corpus=path.join(dir,'corpus.txt');
 fs.writeFileSync(pin,JSON.stringify({revision:base,tag:'base',date:'2026-01-01'}));
 // ot3 uses getConfig, the old method and the follow-up setting.
 fs.writeFileSync(corpus,'rpc(e,"server.getConfig");rpc(e,"old.method");settings["followUpBehavior"];"message.dispatch";"/api/orchestration/shell";"chat.new"');
 const run=(...a)=>spawnSync(process.execPath,[tool,'--source',repo,'--pin-file',pin,'--inventory',inventory,'--corpus',corpus,...a],{encoding:'utf8'});
 let r=run('--rev',base,'--check');assert.equal(r.status,1,'an empty inventory does not cover the pin');assert.match(r.stderr,/not in the inventory: rpc:server\.getConfig/);
 r=run('--rev',base,'--sync');assert.equal(r.status,0,r.stderr);
 const items=JSON.parse(fs.readFileSync(inventory,'utf8')).items;
 assert.equal(items['rpc:server.getConfig'].status,'supported');assert.equal(items['rpc:projects.list'].status,'missing');assert.equal(items['rpc:server.getConfig'].reviewed,false);
 r=run('--rev',base,'--check');assert.equal(r.status,0,r.stderr);
 r=run();assert.equal(r.status,0,r.stderr);
 assert.match(r.stdout,/\*\*new\*\* `projects\.listIgnoredFiles` — not in ot3/);
 assert.match(r.stdout,/\*\*removed\*\* `old\.method` — \*\*ot3 still uses it \(breaking\)\*\*/);
 assert.match(r.stdout,/\*\*new\*\* `newClientToggle`/);assert.match(r.stdout,/\*\*new\*\* `composer\.sendBackground`/);
 assert.match(r.stdout,/### Chat and composer \(1\)[\s\S]*composer change/);
 r=run('--pin');assert.equal(r.status,1,'the pin cannot move past unreviewed items');
 r=run('--sync');assert.equal(r.status,0);r=run('--pin');assert.equal(r.status,0,r.stderr);
 assert.equal(JSON.parse(fs.readFileSync(pin,'utf8')).revision,git('rev-parse','HEAD'));
 // A supported item ot3 stops referencing is a regression.
 fs.writeFileSync(corpus,'"old.method"');r=run('--check');assert.equal(r.status,1);assert.match(r.stderr,/marked supported but ot3 no longer references it: rpc:server\.getConfig/);
 console.log('UPSTREAM_DRIFT_PASS: new/removed/breaking surface, settings and keybindings, commits by area, inventory sync/check, pin guarded by the inventory');
}finally{fs.rmSync(dir,{recursive:true,force:true})}
