// Actual original controller/platform plus native Host; transport metadata peer only.
function runNativeVoiceHostOracle(factories) {
 function check(value,message){if(!value)throw Error(message)}
 function settle(){var value=Promise.resolve();for(var index=0;index<100;index++)value=value.then(function(){});return value}
 var primitives=factories.primitives(),runtime=factories.voice(primitives),sequence=0,sessionSequence=0;
 var packets=[],sessions={},heldRefresh=null,refreshError=false,initialMetadataFailure=false,metadataError='',permission=null,devices=[],active=false,invalidations=0,metadataEvents=0,commits=[];
 var draft={ownerKey:'draft/one',text:'hello ',selection:{start:6,end:6},revision:1};
 var settings={voiceMicrophone:'chosen-mic',voiceShortcutMode:'auto',speechLanguage:'auto',speechTranslateToEnglish:false,
  speechCustomWords:[],speechSilenceFiltering:true,speechRemoveFillerWords:true,speechPostProcessingPrompt:{mode:'default',customInstructions:''},speechCorrectionWord:''};
 var snapshot={choiceJson:JSON.stringify({id:'explicit-fixture-audio-destination',destination:{kind:'environment',environmentId:'audio'}}),cleanupJson:'null'};
 function reply(packet,value,failure){host.receive({kind:'result',requestId:packet.requestId,session:packet.session,value:value||{},error:failure||null})}
 function emit(session,label){host.receive({kind:'microphones',session:session,devices:[{deviceId:'chosen-mic',label:label},{deviceId:'line-input',label:'Line input'},{deviceId:'default',label:'Default input'}]})}
 function send(packet){
  packets.push(packet);
  if(['owner','abort','abort-request'].indexOf(packet.action)>=0)return;
  Promise.resolve().then(function(){
   var state=sessions[packet.session];
   switch(packet.action){
   case 'snapshot':reply(packet,snapshot);break;
   case 'decode-preferences':reply(packet,{valueJson:JSON.stringify(settings)});break;
   case 'project-context':reply(packet,{valueJson:JSON.stringify({speechProjectCustomWords:[]})});break;
   case 'begin':var id='refresh-scope-'+(++sessionSequence);sessions[id]={};reply(packet,{session:id,consent:snapshot});break;
   case 'status':reply(packet,{valueJson:JSON.stringify({supported:true,supportsStreaming:true,supportsBufferedStreaming:false,supportsTranslation:false})});break;
   case 'configure':case 'stream-ticket':case 'stream-connect':case 'record':reply(packet);break;
   case 'capture-prepare':
    check(packet.microphoneId==='chosen-mic','original saved microphone reaches capture');
    state.prepare=packet;host.receive({kind:'permission',session:packet.session,permissionId:'explicit-'+packet.session});break;
   case 'permission':
    check(packet.allow===true&&state.prepare,'fixture requires explicit permission through actual Host API');
    if(initialMetadataFailure){host.receive({kind:'microphones-error',session:packet.session,error:'microphones_unavailable'});initialMetadataFailure=false}else emit(packet.session,'Initial '+packet.session);
    reply(state.prepare);state.prepare=null;reply(packet);
    host.receive({kind:'stream',session:packet.session,value:{event:'ready'}});break;
   case 'refresh-microphones':
    check(!state.stopped,'refresh never dispatched after stop');
    if(refreshError)reply(packet,null,'microphones_unavailable');else{check(!heldRefresh,'one refresh RPC in flight');heldRefresh=packet}break;
   case 'stop':state.stopped=true;reply(packet,{uri:null});break;
   case 'release-capture':state.released=true;reply(packet);break;
   case 'stream-finish':check(state.stopped&&state.released,'original stop/release ordering');reply(packet,{text:'voice'});break;
   case 'dispose':case 'delete-recording':reply(packet);break;
   default:throw Error('Unexpected refresh peer action '+packet.action);
   }
  });
 }
 var host=factories.host({primitives:primitives,runtime:runtime,inputFactory:factories.input,preferencesFactory:factories.preferences,
  uuid:function(){return 'refresh-request-'+(++sequence)},send:send,
  readLatest:function(){return {clientSettings:settings,input:{environmentId:'draft',ownerKey:draft.ownerKey,projectId:'project',cleanup:true},cleanupPrepared:false}},
  onPermission:function(session,id){permission={session:session,id:id}},
  onMicrophones:function(value){devices=value;metadataError='';metadataEvents++},onMicrophonesActive:function(value){active=value},
  onMicrophonesError:function(value){metadataError=value},
  onMicrophonesInvalidated:function(){devices=[];invalidations++},
  onLevel:function(){},onDurationLimit:function(){},onText:function(){},onError:function(value){throw Error(value)}});
 host.ownerChanged(draft.ownerKey,'draft',1);host.receive({kind:'ready',available:true});
 var binding=host.controller();
 function controllerFor(bound){return new runtime.VoiceInputController({retryFailedRecording:true,recorder:bound.recorder,
  getTranscriber:function(){return bound.transcriber},requestPermission:function(){return Promise.resolve({granted:true,canAskAgain:true})},
  configureRecording:function(){return Promise.resolve()},releaseRecording:bound.releaseRecording,deleteRecording:bound.deleteRecording,
  postProcess:bound.postProcess,onPostProcessingError:function(){throw Error('Unexpected cleanup')},readDraft:function(){return draft},
  commitDraft:function(text,selection){commits.push(text);draft={ownerKey:draft.ownerKey,text:text,selection:selection,revision:draft.revision+1}},onStateChange:function(){}})}
 var controller=controllerFor(binding),starting,firstSession,refreshRequestsBefore;
 var cases=[];
 function rejected(action,message){return action.then(function(){throw Error(message+' unexpectedly succeeded')},function(error){check(!!error,message)})}
 return settle().then(function(){
  var before=packets.length;return rejected(host.refreshMicrophones(),'idle refresh refused').then(function(){check(packets.length===before,'idle refresh starts no RPC or capture');cases.push('idle request cannot start recording or permission')})
 }).then(function(){
  starting=controller.start();return settle();
 }).then(function(){
  check(permission&&!active&&controller.currentState.phase==='preparing','native permission boundary before metadata active');
  firstSession=permission.session;var before=packets.length;
  return rejected(host.refreshMicrophones(),'preparing refresh refused').then(function(){check(packets.length===before,'refresh cannot create a permission grant');return host.permission(permission.session,permission.id,true)});
 }).then(function(){return starting}).then(function(){
  check(active&&controller.currentState.phase==='recording'&&metadataEvents===1,'explicitly prepared current scope active');
  var beforeSync=invalidations;host.ownerChanged(draft.ownerKey,'draft',1);
  check(active&&invalidations===beforeSync&&devices.length===3,'identical owner synchronization preserves active metadata and accepted cache');
  cases.push('prepare and explicit native permission precede refresh');
  refreshRequestsBefore=packets.filter(function(value){return value.action==='refresh-microphones'}).length;
  var first=host.refreshMicrophones(),second=host.refreshMicrophones();check(first===second,'same in-flight refresh promise');
  return settle().then(function(){check(heldRefresh&&heldRefresh.session===firstSession,'current scope owns typed refresh');emit(firstSession,'<b>Literal refreshed label</b>');var packet=heldRefresh;heldRefresh=null;reply(packet);return first});
 }).then(function(){
  check(devices[0].label==='<b>Literal refreshed label</b>','current repeated metadata reaches UI adapter unchanged');
  check(packets.filter(function(value){return value.action==='refresh-microphones'}).length===refreshRequestsBefore+1,'rapid refreshes coalesce at the Host boundary');
  check(settings.voiceMicrophone==='chosen-mic','refresh never writes preferences');cases.push('coalesced manual refresh routes current scope without changing selection');
  emit(firstSession,'Synthetic devicechange');check(devices[0].label==='Synthetic devicechange','repeated current event accepted');
  cases.push('synthetic repeated event plumbing, not physical hotplug');
  refreshError=true;return rejected(host.refreshMicrophones(),'enumeration error surfaced');
 }).then(function(){
  check(active&&controller.currentState.phase==='recording','metadata error preserves actual original controller phase');refreshError=false;
  cases.push('metadata error does not stop recording');
  var refresh=host.refreshMicrophones().then(function(){return 'accepted'},function(){return 'rejected'});
  return settle().then(function(){var stop=controller.stop();return settle().then(function(){
   check(!active,'stop deactivates immediately');var accepted=JSON.stringify(devices);emit(firstSession,'STALE AFTER STOP');check(JSON.stringify(devices)===accepted,'late event cannot replace cached list');
   check(devices[0].label==='Synthetic devicechange','last accepted list survives idle for next-recording selection');
   var packet=heldRefresh;heldRefresh=null;reply(packet);return Promise.all([stop,refresh]);
  })});
 }).then(function(values){
  check(values[1]==='rejected'&&controller.currentState.phase==='idle','pending refresh cannot complete successfully after stop');
  check(commits.length===1&&commits[0]==='hello voice','original draft insertion is unchanged');cases.push('stop cancels refresh authority and retains accepted cache');
  return rejected(host.refreshMicrophones(),'idle after stop refresh refused');
 }).then(function(){
  initialMetadataFailure=true;permission=null;starting=controller.start();return settle();
 }).then(function(){
  check(permission&&permission.session!==firstSession&&!active,'next recording has a fresh explicit permission scope');
  return host.permission(permission.session,permission.id,true).then(function(){return starting});
 }).then(function(){
  check(active&&controller.currentState.phase==='recording'&&devices.length===3&&devices[0].label==='Synthetic devicechange'&&metadataError==='microphones_unavailable','failed initial enumeration preserves three cached choices and active recording');
  host.receive({kind:'microphones',session:permission.session,devices:[]});
  check(devices.length===0&&metadataError===''&&active,'successful empty list replaces cache without stopping capture');
  cases.push('failed initial enumeration retains three cached devices; successful empty replaces them');
  var previousSession=permission.session;
  var pending=host.refreshMicrophones().then(function(){return 'accepted'},function(){return 'rejected'});
  return settle().then(function(){
   var packet=heldRefresh;heldRefresh=null;
   host.ownerChanged('draft/two','draft',2);controller.dispose();binding.dispose();
   check(!active&&devices.length===0,'owner replacement invalidates cached realm metadata');
   draft={ownerKey:'draft/two',text:'second ',selection:{start:7,end:7},revision:1};
   binding=host.controller();controller=controllerFor(binding);permission=null;starting=controller.start();
   return settle().then(function(){
    check(permission&&permission.session!==previousSession,'successor scope created by actual original controller');
    return host.permission(permission.session,permission.id,true);
   }).then(function(){return starting}).then(function(){
    check(active&&controller.currentState.phase==='recording','successor active before late old refresh');
    var accepted=JSON.stringify(devices);emit(previousSession,'OLD OWNER');reply(packet);
    return pending.then(function(outcome){
     check(outcome==='rejected'&&JSON.stringify(devices)===accepted&&active,'old refresh completion and metadata cannot alter successor');
     cases.push('owner replacement fences old refresh completion and metadata');
    });
   });
  });
 }).then(function(){
  var pending=host.refreshMicrophones().then(function(){return 'accepted'},function(){return 'rejected'});
  return settle().then(function(){host.lost();return pending}).then(function(outcome){
   check(outcome==='rejected'&&!active&&devices.length===0,'helper loss invalidates metadata and pending refresh');
   controller.dispose();binding.dispose();cases.push('helper loss cancels pending metadata authority');
   return settle();
  });
 }).then(function(){
  return {complete:true,count:cases.length,cases:cases,scope:'Actual original controller/platform/Host with synthetic metadata peer; no physical hotplug, microphone, PCM or native Rust parser execution',
   explicitPermissionActions:packets.filter(function(value){return value.action==='permission'}).length,
   refreshRequests:packets.filter(function(value){return value.action==='refresh-microphones'}).length,
   originalCommits:commits,invalidations:invalidations};
 });
}
