// Whole original projection/presentation functions with actual Effect services.
// Private Node container only; output is a reference, not Rust acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createRequire, stripTypeScriptTypes} from 'node:module';
import {pathToFileURL} from 'node:url';
import vm from 'node:vm';
const [nativeRoot, output] = process.argv.slice(2);
if (!nativeRoot || !output) throw Error('native-root private-output required');
const paths = {
 projection:'packages/client-runtime/src/state/serverConfigProjection.ts',
 presentation:'packages/client-runtime/src/state/presentation.ts',
 settings:'packages/contracts/src/settings.ts'
};
const pins = {projection:'9b2e59b961a8d366e3679918e8d7a49b659cd7b8a1d459787c9ce88c4bbf2662',
 presentation:'beac3c8a1cec697205568b9c041cbb3f41a5148b8505f9dd1aa299c8a781b6f8'};
const sources = {}, hashes = {};
for (const [key,name] of Object.entries(paths)) {
 sources[key] = await fs.readFile(nativeRoot+'/'+name,'utf8');
 hashes[name] = createHash('sha256').update(sources[key]).digest('hex');
 if (pins[key]) assert.equal(hashes[name],pins[key]);
}
for (const key of ['automaticGitFetchInterval','providerHealthRefreshInterval','hostPowerMonitorActiveInterval','hostPowerMonitorIdleInterval'])
 assert(sources.settings.includes(key+': Schema.optionalKey(Schema.DurationFromMillis)'));
const require = createRequire('/deps/package.json');
assert.equal(JSON.parse(await fs.readFile('/deps/node_modules/effect/package.json','utf8')).version,'4.1.2');
const load = async name => import(pathToFileURL(require.resolve(name)).href);
const Option = await load('effect/Option'), Schema = await load('effect/Schema');
const context = vm.createContext({decodeDuration:Schema.decodeUnknownSync(Schema.DurationFromMillis)});
const native = new vm.SourceTextModule(stripTypeScriptTypes(sources.projection,{mode:'strip'}),{context});
await native.link(async name => {
 assert(['effect/Equal','effect/Option'].includes(name));
 const values = await load(name);
 return new vm.SyntheticModule(Object.keys(values),function(){for(const[key,value]of Object.entries(values))this.setExport(key,value)},{context});
});
await native.evaluate();
const first = sources.presentation.indexOf('function withoutProviders(');
const last = sources.presentation.indexOf('\nexport interface ',first);
assert(first>=0&&last>first);
vm.runInContext(stripTypeScriptTypes(sources.presentation.slice(first,last),{mode:'strip'}),context);
context.applyProjection = native.namespace.applyServerConfigProjection;
context.none = Option.none; context.some = Option.some; context.get = Option.getOrNull;
const base = {environment:{capabilities:{serverBrowser:true}},providers:[],settings:{automaticGitFetchInterval:500,providerHealthRefreshInterval:300000},keybindings:[],issues:[]};
const snap = () => ({version:1,type:'snapshot',config:structuredClone(base)});
const event = (type,payload) => ({version:1,type,payload});
const cases = [
 ['initial',snap()],['equal-snapshot',snap()],
 ['providers-only',event('providerStatuses',{providers:[{id:'source-only-fixture'}]})],
 ['same-settings-fresh-duration',event('settingsUpdated',{settings:{...base.settings}})],
 ['equal-integer-float-wire',JSON.stringify(event('settingsUpdated',{settings:{...base.settings}})).replace('"automaticGitFetchInterval":500','"automaticGitFetchInterval":500.0')],
 ['empty-themes-add-own-undefined',event('environmentThemesUpdated',{themes:[]})],
 ['repeated-empty-themes',event('environmentThemesUpdated',{themes:[]})],
 ['snapshot-removes-undefined-theme-key',snap()],
 ['empty-sources-add-own-undefined',event('usageLimitSourcesUpdated',{sources:[]})],
 ['repeated-empty-sources',event('usageLimitSourcesUpdated',{sources:[]})],
 ['snapshot-removes-undefined-source-key',snap()],
 ['both-undefined-themes',event('environmentThemesUpdated',{themes:[]})],
 ['both-undefined-sources',event('usageLimitSourcesUpdated',{sources:[]})],
 ['snapshot-removes-both-undefined',snap()],
 ['keybindings-new-empty-arrays',event('keybindingsUpdated',{keybindings:[],issues:[]})],
 ['keybindings-repeated-empty-arrays',event('keybindingsUpdated',{keybindings:[],issues:[]})],
 ['snapshot-reuses-empty-arrays',snap()],
 ['duration-changes',event('settingsUpdated',{settings:{...base.settings,automaticGitFetchInterval:501}})],
 ['duration-same-after-change',event('settingsUpdated',{settings:{...base.settings,automaticGitFetchInterval:501}})],
 ['snapshot-restores-duration',snap()]
];
vm.runInContext('var current=none(), previous=null;',context);
const rows = [];
for (const [name,wire] of cases) {
 const eventJson = typeof wire==='string'?wire:JSON.stringify(wire);
 context.encoded = eventJson;
 const result = vm.runInContext(`(function(){
  const event=JSON.parse(encoded);
  const settings=event.type==='snapshot'?event.config.settings:event.type==='settingsUpdated'?event.payload.settings:null;
  if(settings)for(const key of ['automaticGitFetchInterval','providerHealthRefreshInterval','hostPowerMonitorActiveInterval','hostPowerMonitorIdleInterval'])
    if(Object.prototype.hasOwnProperty.call(settings,key))settings[key]=decodeDuration(settings[key]);
  current=applyProjection(current,event);
  const config=withoutProviders(get(current)?.config??null,previous);
  const changed=config!==previous;previous=config;
  return {changed,ownUndefined:Object.keys(config??{}).filter(key=>config[key]===undefined).sort()};
 })()`,context);
 rows.push({name,eventJson,changed:result.changed,ownUndefined:Array.from(result.ownUndefined)});
}
assert.equal(rows.find(row=>row.name==='empty-themes-add-own-undefined').changed,true);
assert.equal(rows.find(row=>row.name==='snapshot-removes-undefined-theme-key').changed,true);
assert.equal(rows.find(row=>row.name==='empty-sources-add-own-undefined').changed,true);
assert.equal(rows.find(row=>row.name==='snapshot-removes-undefined-source-key').changed,true);
assert.equal(rows.find(row=>row.name==='same-settings-fresh-duration').changed,false);
assert.equal(rows.find(row=>row.name==='equal-integer-float-wire').changed,false);
await fs.writeFile(output,JSON.stringify({revision:'83a3674ffd84ffb33de768210c806cb1ac53fd80',sourceSha256:hashes,
 effect:'4.1.2',wholeProjection:true,wholeWithoutProviders:true,rows,
 qualification:'Actual whole original projection/presentation with decoded DurationFromMillis fields; pure config identity values, not full ServerConfig schema or Rust/runtime transport acceptance'},null,2)+'\n',{flag:'wx'});
