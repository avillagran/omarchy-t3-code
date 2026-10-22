#!/usr/bin/env node
// What changed in T3 Code since the revision ot3 is validated against
// (upstream/t3code.json), and what of it ot3 does not cover yet.
//
//   node tools/upstream-drift.cjs [--source DIR] [--rev REV]   report to stdout
//     --write   also write upstream/DRIFT.md
//     --sync    add the items found at REV to upstream/parity.json (unreviewed)
//     --check   exit 1 if the pinned revision has items missing from the inventory,
//               items marked supported that ot3 no longer references, or stale generated tables
//     --list KIND  print one surface (rpc, orchestration, http, client-setting, server-setting, keybinding) with ot3's coverage
//     --pin     move upstream/t3code.json to REV once its items are all in the inventory
//
// It compares what can be compared objectively: RPC methods, orchestration
// command/event types, HTTP endpoints, client/server settings and keybinding
// commands, plus the commits grouped by the part of ot3 they touch.
// Read-only on the T3 checkout (git show/grep/log only).
const fs=require('fs'),path=require('path'),{execFileSync}=require('child_process');
const root=path.resolve(__dirname,'..');
const args=process.argv.slice(2),flag=n=>args.includes(n),opt=(n,d)=>{const i=args.indexOf(n);return i>=0&&args[i+1]?args[i+1]:d};
// --pin-file, --inventory and --corpus let the test run against a fixture.
const pinFile=path.resolve(opt('--pin-file',path.join(root,'upstream/t3code.json'))),inventoryFile=path.resolve(opt('--inventory',path.join(root,'upstream/parity.json'))),reportFile=path.join(path.dirname(inventoryFile),'DRIFT.md');
const source=path.resolve(opt('--source',path.join(root,'../t3code-nightly')));
const pin=JSON.parse(fs.readFileSync(pinFile,'utf8'));
if(!fs.existsSync(path.join(source,'.git'))){console.error(`upstream-drift: no T3 Code checkout at ${source} (use --source)`);process.exit(2)}
const git=(...a)=>execFileSync('git',['-C',source,...a],{encoding:'utf8',maxBuffer:1<<28,stdio:['ignore','pipe','ignore']});
const tryGit=(...a)=>{try{return git(...a)}catch{return ''}};
const rev=git('rev-parse',opt('--rev','HEAD')).trim();
const tagOf=r=>tryGit('describe','--tags',r).trim()||r.slice(0,8);
const show=(r,f)=>tryGit('show',`${r}:${f}`);
const listFiles=(r,dir)=>tryGit('ls-tree','-r','--name-only',r,'--',dir).split('\n').filter(Boolean);
const uniq=a=>[...new Set(a)].sort();
const matches=(text,re)=>[...text.matchAll(re)].map(m=>m[1]);
// A Schema.Struct body from `export const NAME = Schema.Struct({` to its closing line.
const structKeys=(text,name)=>{const start=text.search(new RegExp(`export const ${name}\\s*=\\s*Schema\\.Struct\\(\\{`));if(start<0)return[];const end=text.indexOf('\n});',start);return matches(text.slice(start,end<0?undefined:end),/^ {2}([A-Za-z][A-Za-z0-9]*)\s*:/gm)};

// The comparable surface of T3 Code at one revision.
function surface(r){
  const contracts=listFiles(r,'packages/contracts/src').filter(f=>f.endsWith('.ts')&&!f.endsWith('.test.ts'));
  const rpc=[],types=[],http=[];
  for(const f of contracts){
    const t=show(r,f);
    for(const block of matches(t,/export const \w*METHODS\s*=\s*\{([\s\S]*?)\n\}/g))rpc.push(...matches(block,/:\s*"([A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)+)"/g));
    http.push(...matches(t,/HttpApiEndpoint\.\w+\(\s*"[^"]+",\s*"(\/api\/[^"]+)"/g));
    if(/orchestration/i.test(f))types.push(...matches(t,/type:\s*Schema\.Literal\("([a-z][A-Za-z]*(?:\.[A-Za-z0-9_-]+)+)"\)/g));
  }
  const settings=show(r,'packages/contracts/src/settings.ts');
  const keys=show(r,'packages/contracts/src/keybindings.ts');
  return {
    rpc:uniq(rpc),
    orchestration:uniq(types),
    http:uniq(http.map(p=>p.replace(/:[A-Za-z]+/g,'*'))),
    'client-setting':uniq(structKeys(settings,'ClientSettingsSchema')),
    'server-setting':uniq(structKeys(settings,'ServerSettings')),
    keybinding:uniq(matches(keys,/^\s+"([a-z][A-Za-z]*(?:\.[A-Za-z0-9]+)+)",?\s*$/gm)),
  };
}

// Everything ot3 wrote itself (generated copies of T3's tables would count every item as covered).
function ot3Corpus(){
  if(opt('--corpus'))return fs.readFileSync(opt('--corpus'),'utf8');
  const files=[];
  const walk=(dir,keep)=>{if(!fs.existsSync(dir))return;for(const e of fs.readdirSync(dir,{withFileTypes:true})){const p=path.join(dir,e.name);if(e.isDirectory())walk(p,keep);else if(keep(e.name))files.push(p)}};
  walk(path.join(root,'src'),n=>/\.(rs|sh)$/.test(n));
  walk(path.join(root,'vendor/t3-protocol/src'),n=>n.endsWith('.rs'));
  walk(path.join(root,'ui/qml'),n=>n.endsWith('.qml'));
  walk(path.join(root,'ui/js'),n=>n.endsWith('.js')&&!n.startsWith('Native'));
  walk(path.join(root,'app'),n=>n.endsWith('.qml'));
  return files.map(f=>fs.readFileSync(f,'utf8')).join('\n');
}
const corpus=ot3Corpus();
const escape=s=>s.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
function used(kind,name){
  if(kind==='http')return corpus.includes(name.split('*')[0]);
  if(kind.endsWith('-setting'))return new RegExp(`["'.]${escape(name)}\\b|\\b${escape(name)}\\s*:`).test(corpus);
  return corpus.includes(`"${name}"`)||corpus.includes(`'${name}'`)||(kind==='keybinding'&&corpus.includes(name));
}

const pinned=surface(pin.revision),target=rev===pin.revision?pinned:surface(rev);
const kinds=Object.keys(target);
const inventory=fs.existsSync(inventoryFile)?JSON.parse(fs.readFileSync(inventoryFile,'utf8')):{schemaVersion:1,items:{}};
const id=(k,n)=>`${k}:${n}`;

// Commits since the pin, grouped by the part of ot3 they touch.
const areas=[
  [/^packages\/contracts\//,'Protocol (contracts)'],[/^apps\/server\//,'Server'],[/^packages\/ssh\/|connection/i,'Connections'],
  [/components\/chat\/|composer/i,'Chat and composer'],[/sidebar/i,'Sidebar'],[/settings/i,'Preferences'],[/keybinding/i,'Keybindings'],
  [/provider/i,'Providers'],[/^apps\/desktop\//,'Desktop app (not ported)'],[/^apps\/mobile\//,'Mobile app (not applicable)'],[/^apps\/web\//,'Web UI (other)'],
];
function commitsByArea(){
  const log=tryGit('log','--no-merges','--format=%x1e%h%x09%s','--name-only',`${pin.revision}..${rev}`);
  const groups={};let total=0;
  for(const chunk of log.split('\x1e').filter(c=>c.trim())){
    const [head,...files]=chunk.trim().split('\n');total++;
    const hit=uniq(files.filter(Boolean).map(f=>(areas.find(([re])=>re.test(f)) || [null,'Other'])[1]));
    for(const a of hit.length?hit:['Other'])(groups[a]=groups[a] || []).push(head);
  }
  return {total,groups};
}

// Generated tables stamped with another revision than the pin.
function staleTables(){
  if(opt('--corpus'))return [];
  const dir=path.join(root,'ui/js');
  return fs.readdirSync(dir).filter(n=>n.endsWith('.sources.json')).filter(n=>{try{return JSON.parse(fs.readFileSync(path.join(dir,n),'utf8')).commit!==pin.revision}catch{return true}});
}

function report(){
  const {total,groups}=commitsByArea(),lines=[];
  lines.push(`# T3 Code drift`,'',`Validated against **${pin.tag}** (\`${pin.revision.slice(0,8)}\`, ${pin.date}). Compared with **${tagOf(rev)}** (\`${rev.slice(0,8)}\`): ${total} commits.`,'');
  const labels={rpc:'RPC methods',orchestration:'Orchestration commands and events',http:'HTTP endpoints','client-setting':'Client settings','server-setting':'Server settings',keybinding:'Keybinding commands'};
  const summary=[];
  for(const k of kinds){
    const added=target[k].filter(n=>!pinned[k].includes(n)),removed=pinned[k].filter(n=>!target[k].includes(n));
    const missingNow=target[k].filter(n=>!used(k,n)&&!['ignored','missing','partial'].includes((inventory.items[id(k,n)] || {}).status));
    summary.push(`| ${labels[k]} | ${target[k].length} | ${added.length} | ${removed.length} | ${target[k].filter(n=>used(k,n)).length} |`);
    if(!added.length&&!removed.length)continue;
    lines.push(`## ${labels[k]}`,'');
    for(const n of added)lines.push(`- **new** \`${n}\` — ${used(k,n)?'ot3 already references it':'not in ot3'}${inventory.items[id(k,n)]?` (inventory: ${inventory.items[id(k,n)].status})`:''}`);
    for(const n of removed)lines.push(`- **removed** \`${n}\`${used(k,n)?' — **ot3 still uses it (breaking)**':''}`);
    lines.push('');
    void missingNow;
  }
  lines.splice(4,0,'| Surface | At target | New | Removed | Referenced by ot3 |','| --- | --: | --: | --: | --: |',...summary,'');
  const stale=staleTables();
  if(stale.length)lines.push('## Generated tables','',...stale.map(n=>`- \`ui/js/${n}\` was generated from another revision than the pin.`),'');
  const unreviewed=Object.entries(inventory.items).filter(([,v])=>v.reviewed===false).length;
  lines.push('## Inventory','',`${Object.keys(inventory.items).length} items in \`upstream/parity.json\`, ${unreviewed} not reviewed yet.`,'');
  lines.push('## Commits by area','');
  for(const [,a] of areas.concat([[null,'Other']]))if(groups[a]){lines.push(`### ${a} (${groups[a].length})`,'',...groups[a].slice(0,60).map(c=>`- ${c.replace('\t',' ')}`),...(groups[a].length>60?[`- … ${groups[a].length-60} more`]:[]),'')}
  return lines.join('\n');
}

function sync(){
  let added=0;
  for(const k of kinds)for(const n of target[k]){
    if(inventory.items[id(k,n)])continue;
    inventory.items[id(k,n)]={status:used(k,n)?'supported':'missing',reviewed:false,since:rev===pin.revision?pin.tag:tagOf(rev)};added++;
  }
  inventory.items=Object.fromEntries(Object.entries(inventory.items).sort(([a],[b])=>a.localeCompare(b)));
  fs.writeFileSync(inventoryFile,JSON.stringify(inventory,null,1)+'\n');
  console.error(`upstream-drift: ${added} items added to upstream/parity.json`);
}

function check(){
  const problems=[];
  for(const k of kinds)for(const n of pinned[k]){
    const item=inventory.items[id(k,n)];
    if(!item)problems.push(`not in the inventory: ${id(k,n)}`);
    else if(item.status==='supported'&&!used(k,n))problems.push(`marked supported but ot3 no longer references it: ${id(k,n)}`);
  }
  for(const n of staleTables())problems.push(`generated from another revision than the pin: ui/js/${n}`);
  return problems;
}

if(opt('--list')){const k=opt('--list');for(const n of target[k] || [])console.log(`${used(k,n)?'ot3':'---'}  ${n}`);process.exit(0)}
if(flag('--sync'))sync();
if(flag('--pin')){
  const missing=kinds.flatMap(k=>target[k].filter(n=>!inventory.items[id(k,n)]).map(n=>id(k,n)));
  if(missing.length){console.error(`upstream-drift: ${missing.length} items at ${tagOf(rev)} are not in the inventory; run --sync and review them first`);process.exit(1)}
  const next={...pin,revision:rev,tag:tagOf(rev),date:git('log','-1','--format=%cs',rev).trim()};
  fs.writeFileSync(pinFile,JSON.stringify(next,null,2)+'\n');
  console.error(`upstream-drift: pinned ${next.tag}; regenerate the tables (tools/build-*.cjs) against it`);
}
const text=report();
if(flag('--write'))fs.writeFileSync(reportFile,text+'\n');
if(flag('--check')){const problems=check();if(problems.length){console.error(problems.map(p=>'upstream-drift: '+p).join('\n'));process.exit(1)}console.error('upstream-drift: inventory covers the pinned revision')}
else process.stdout.write(text+'\n');
