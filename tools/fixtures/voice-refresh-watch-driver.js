// Real Qt/stdin/watch/Host and SDK; the example owns its synthetic microphone.
function runVoiceWatchFixture(factories) {
 function deferred(){var resolve,reject;return {promise:new Promise(function(yes,no){resolve=yes;reject=no}),resolve:resolve,reject:reject}}
 function check(value,message){if(!value)throw Error(message)}
 var available=deferred(),finished=deferred(),queue=[],pending=new Map(),drainers=[],actions=[],states=[],commits=[],sequence=0;
 var controller,binding,recordConfirmed=false,stopStarted=false,permissionCount=0,closed=false,microphoneEvents=[],cached=[],metadataActive=false;
 var refreshStarted=false,refreshCompleted=false,positiveLevels=0;
 var primitives=factories.primitives(),runtime=factories.voice(primitives);
 var draft={ownerKey:'qt-refresh-draft',text:'hello ',selection:{start:6,end:6},revision:1};
 var settings={voiceMicrophone:'',voiceShortcutMode:'auto',speechLanguage:'auto',speechTranslateToEnglish:false,speechCustomWords:[],speechSilenceFiltering:true,speechRemoveFillerWords:true,speechPostProcessingPrompt:{mode:'default',customInstructions:''},speechCorrectionWord:''};
 function fail(error){finished.reject(Error(String(error&&error.message||error)+' '+JSON.stringify({actions:actions,states:states,permissionCount:permissionCount,microphoneEvents:microphoneEvents,recordConfirmed:recordConfirmed,refreshStarted:refreshStarted,refreshCompleted:refreshCompleted,positiveLevels:positiveLevels})))}
 function checkDrain(){Promise.resolve().then(function(){if(!pending.size&&!queue.length){var all=drainers;drainers=[];all.forEach(function(resolve){resolve()})}})}
 function drain(){return new Promise(function(resolve){drainers.push(resolve);checkDrain()})}
 function stop(){
  if(stopStarted||!recordConfirmed||!refreshCompleted||positiveLevels===0||!controller||controller.currentState.phase!=='recording')return;
  stopStarted=true;
  controller.stop().then(function(){
   check(controller.currentState.phase==='idle'&&commits.length===1&&commits[0]==='hello qt hosted transcript','original controller transcript insertion');
   check(permissionCount===1&&microphoneEvents.length>=2&&refreshCompleted,'one explicit permission covers initial and refreshed metadata');
   check(!metadataActive&&cached.length===3,'accepted choices remain cached after actual capture stop');
   var before=actions.length;
   return host.refreshMicrophones().then(function(){throw Error('Idle refresh unexpectedly succeeded')},function(){check(actions.length===before,'idle refresh sends no capture or permission RPC')});
  }).then(drain).then(function(){controller.dispose();binding.dispose();return drain()}).then(function(){
   finished.resolve({states:states,actions:actions,commits:commits,permissionCount:permissionCount,
    microphoneEvents:microphoneEvents,positiveLevels:positiveLevels,realWire:true,
    refreshRpcCount:actions.filter(function(action){return action==='refresh-microphones'}).length,
    nativeRustRefreshVerified:true,cachedChoicesAfterStop:true,idleRefreshInert:true});
  },fail);
 }
 function refresh(){
  if(refreshStarted||!recordConfirmed||!controller||controller.currentState.phase!=='recording')return;
  refreshStarted=true;check(metadataActive,'manual refresh requires already prepared actual capture');
  var first=host.refreshMicrophones(),second=host.refreshMicrophones();
  check(first===second,'actual Host facade coalesces concurrent manual refresh');
  first.then(function(){
   check(microphoneEvents.length>=2&&permissionCount===1,'actual Rust refresh completes after repeated original metadata');
   check(actions.filter(function(action){return action==='refresh-microphones'}).length===1,'one typed refresh wire command');
   refreshCompleted=true;stop();
  },fail);
 }
 var host=factories.host({primitives:primitives,runtime:runtime,inputFactory:factories.input,preferencesFactory:factories.preferences,
  uuid:function(){return 'qt-refresh-request-'+(++sequence)},
  send:function(packet){if(closed)throw Error('Fixture wire closed');actions.push(packet.action);if(packet.requestId)pending.set(packet.requestId,packet.action);queue.push(JSON.stringify(packet))},
  readLatest:function(){return {clientSettings:settings,input:{environmentId:'draft-unavailable',ownerKey:draft.ownerKey,projectId:null,projectName:null,cleanup:false},cleanupPrepared:false}},
  onAvailable:function(value){if(value)available.resolve();else fail(Error('Actual native Host unavailable'))},
  onPermission:function(session,id){permissionCount++;check(permissionCount===1,'no extra permission request for metadata');host.permission(session,id,true).catch(fail)},
  onMicrophones:function(devices){
   check(permissionCount===1&&Array.isArray(devices)&&devices.length===3,'three original fake microphone metadata entries');
   check(devices.every(function(device){return typeof device.deviceId==='string'&&device.deviceId.length>0&&typeof device.label==='string'}),'original exact microphone metadata shape');
   if(cached.length)check(JSON.stringify(cached)===JSON.stringify(devices),'manual refresh preserves real device identities');
   cached=devices;microphoneEvents.push({count:devices.length,hasNonDefault:devices.some(function(device){return device.deviceId!=='default'})});
  },
  onMicrophonesActive:function(value){metadataActive=value},onMicrophonesError:function(value){fail(Error('Unexpected actual metadata failure '+value))},
  onLevel:function(value){if(value>0){positiveLevels++;stop()}},
  onDurationLimit:function(){fail(Error('Unexpected recording duration limit'))},onText:function(){fail(Error('Buffered fixture received streaming text'))},
  onError:fail,onCleanupError:fail});
 host.ownerChanged(draft.ownerKey,'draft-unavailable',1);
 available.promise.then(function(){return host.chooseAudio({kind:'environment',environmentId:'chosen'})}).then(function(){return host.chooseCleanup(false)}).then(function(){
  binding=host.controller();controller=new runtime.VoiceInputController({retryFailedRecording:true,recorder:binding.recorder,
   getTranscriber:function(){return binding.transcriber},requestPermission:function(){return Promise.resolve({granted:true,canAskAgain:true})},
   configureRecording:function(){return Promise.resolve()},releaseRecording:binding.releaseRecording,deleteRecording:binding.deleteRecording,postProcess:binding.postProcess,
   readDraft:function(){return draft},commitDraft:function(text,selection){commits.push(text);draft={ownerKey:draft.ownerKey,text:text,selection:selection,revision:draft.revision+1}},
   onStateChange:function(state){states.push(state.phase)},onPostProcessingError:function(){fail(Error('Cleanup was disabled'))}});
  return controller.start();
 }).then(function(){check(controller.currentState.phase==='recording','actual original controller reached recording');refresh()},fail).catch(fail);
 return {
  result:finished.promise,
  takeWire:function(){var value=queue;queue=[];checkDrain();return value},
  receive:function(line){
   var envelope=JSON.parse(line);check(envelope.type==='native-voice'&&envelope.data,'production watch envelope');var event=envelope.data;
   if(event.kind==='result'){var action=pending.get(event.requestId);check(action,'unexpected native wire result');pending.delete(event.requestId);if(action==='record'&&!event.error)recordConfirmed=true}
   host.receive(event);checkDrain();refresh();
  },
  close:function(){closed=true;host.lost()}
 };
}
