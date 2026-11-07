// The same public-operation traces run on the unbundled source and bundle.
import assert from 'node:assert/strict';
export function exercise(create){
 const AUDIO='t3code:voice-audio-grant:v1',CLEANUP='t3code:voice-cleanup-grant:v1';
 const store=new Map(),events=[],rows=[],ids=new Map();let failKey;
 const window={localStorage:{getItem(key){if(key===failKey)throw Error('Synthetic read denial');return store.get(key)??null;},setItem(key,value){store.set(key,value);},removeItem(key){store.delete(key);}},dispatchEvent(event){events.push(event.detail?.key??event.key);},addEventListener(){},removeEventListener(){}};
 const consent=create({window,CustomEvent:class{constructor(type,{detail}){this.type=type;this.detail=detail;}},console:{error(){}}});
 const environment={kind:'environment',environmentId:'native-env'},cloud={kind:'cloud',environmentId:'native-env',provider:'openai'};
 const current=()=>consent.useAudioChoice();
 function normalize(value){
  if(typeof value==='string'){
   if(/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(value)){if(!ids.has(value))ids.set(value,'grant-'+(ids.size+1));return ids.get(value);}
   for(const [id,name]of ids)if(value===AUDIO+':saved:'+id)return AUDIO+':saved:'+name;
   return value;
  }
  if(Array.isArray(value))return value.map(normalize);
  if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).map(([key,item])=>[key,normalize(item)]));
  return value;
 }
 function record(caseName,observations={}){rows.push(normalize({case:caseName,choice:current(),cleanup:consent.useCleanupGrant(),...observations}));}
 function reset(){store.clear();failKey=undefined;events.length=0;}
 reset();assert.equal(current(),null);assert.equal(consent.readCleanupGrant(),null);record('fresh');
 consent.chooseAudioDestination(environment);const first=current();assert.equal(first.destination.environmentId,'native-env');const bound=consent.bindAudioGrant(first.id);assert(bound());record('environment',{allowed:bound()});
 consent.chooseTranscriptCleanup(true);const cleanup=consent.readCleanupGrant();assert.equal(typeof cleanup,'string');record('independent-cleanup-on');
 consent.chooseAudioDestination(environment);assert.notEqual(current().id,first.id);assert(!bound());assert.equal(consent.readCleanupGrant(),cleanup);record('same-destination-reselection',{oldAllowed:bound()});
 const ordinary={voiceTranscriptionDestination:environment,voiceTranscriptCleanupEnabled:true};consent.chooseAudioDestination(null);assert.equal(current(),'off');assert.equal(ordinary.voiceTranscriptionDestination,environment);assert(!bound());record('stale-ordinary-preferences',{oldAllowed:bound()});
 consent.chooseTranscriptCleanup(false);assert.equal(consent.readCleanupGrant(),null);assert(!store.has(CLEANUP));record('independent-cleanup-off');
 consent.chooseAudioDestination(environment);const previous=current(),prior=consent.bindAudioGrant(previous.id);const acknowledge=consent.pendingCloudAudioChoice(cloud);assert.deepEqual(current(),previous);assert(prior());record('pending-keeps-previous',{previousAllowed:prior()});
 const raw=JSON.parse(store.get(AUDIO)),marker=AUDIO+':saved:'+raw.pending.id;store.set(marker,'false');assert.equal(current(),'off');assert(!prior());record('negative-save-marker',{previousAllowed:prior()});
 store.delete(marker);failKey=marker;assert.equal(current(),'off');assert(!prior());record('unreadable-save-marker',{previousAllowed:prior()});failKey=undefined;
 acknowledge();assert.equal(current().destination.provider,'openai');assert(!prior());const cloudBound=consent.bindAudioGrant(current().id);assert(cloudBound());record('owned-cloud-ack',{previousAllowed:prior(),cloudAllowed:cloudBound()});
 const late=consent.pendingCloudAudioChoice({...cloud,provider:'xai'});consent.chooseAudioDestination(environment);const selected=current();late();assert.deepEqual(current(),selected);assert(!cloudBound());record('stale-cloud-ack',{oldAllowed:cloudBound()});
 reset();consent.chooseAudioDestination({...cloud,provider:'openrouter',model:'vendor/model.v1_2-a'});assert.equal(current().destination.model,'vendor/model.v1_2-a');record('openrouter');
 for(const model of ['openrouter/auto','author/model:nitro','no-slash','a/','a/b'.repeat(60)]){const before=current();consent.chooseAudioDestination({...cloud,provider:'openrouter',model});assert.deepEqual(current(),before);}record('invalid-models-retain-choice');
 reset();store.set(AUDIO,JSON.stringify({id:'legacy\ud800',destination:{kind:'environment',environmentId:'  '+('x'.repeat(4096))+'\udfff  ',extra:true},unknown:true}));const legacy=current();assert.equal(legacy.id,'legacy\ud800');assert.equal(legacy.destination.environmentId,'x'.repeat(4096)+'\udfff');assert(!('unknown'in legacy));const legacyBound=consent.bindAudioGrant(legacy.id);assert(legacyBound());record('utf16-long-id-and-unknown-fields',{allowed:legacyBound()});
 store.set(AUDIO,'{broken');assert.equal(current(),null);assert(!legacyBound());record('corrupt-choice',{oldAllowed:legacyBound()});consent.chooseAudioDestination(environment);assert.equal(current().destination.environmentId,'native-env');record('corrupt-choice-recovery');
 const binding=consent.bindAudioGrant(current().id);assert(binding());failKey=AUDIO;assert(!binding());record('unreadable-choice',{oldAllowed:binding()});failKey=undefined;assert(binding());record('storage-read-restored',{allowed:binding()});
 store.set(CLEANUP,'123');assert.equal(consent.readCleanupGrant(),null);record('corrupt-cleanup-is-independent');
 reset();store.set(AUDIO,JSON.stringify({pending:{id:'',destination:cloud},previous:{id:'previous',destination:environment}}));assert.equal(current().id,'previous');assert(consent.bindAudioGrant('previous')());record('empty-pending-id-missing-marker');
 return {cases:rows,generatedIdentities:ids.size,scope:'Whole original consent/storage producer and codecs; framework snapshot adapter explicit; no audio or UI acceptance'};
}
