// Actual QProcess/stdin metadata, original controller/platform/preferences and
// synthetic microphone permission. This driver never supplies PCM or inference.
function runVoiceWatchFixture(factories) {
    function deferred(){var resolve,reject;var promise=new Promise(function(a,b){resolve=a;reject=b});return {promise:promise,resolve:resolve,reject:reject}}
    function check(value,message){if(!value)throw new Error(message)}
    var available=deferred(),finished=deferred(),queue=[],pending=new Map(),drainers=[],actions=[],states=[],commits=[],sequence=0
    var controller,binding,recordConfirmed=false,stopStarted=false,permissionCount=0,closed=false
    var primitives=factories.primitives(),runtime=factories.voice(primitives)
    var draft={ownerKey:"qt-draft",text:"hello ",selection:{start:6,end:6},revision:1}
    var settings={voiceMicrophone:"",voiceShortcutMode:"auto",speechLanguage:"auto",speechTranslateToEnglish:false,speechCustomWords:[],speechSilenceFiltering:true,speechRemoveFillerWords:true,speechPostProcessingPrompt:{mode:"default",customInstructions:""},speechCorrectionWord:""}
    function fail(error){finished.reject(error instanceof Error?error:new Error(String(error)))}
    function checkDrain(){Promise.resolve().then(function(){if(!pending.size&&!queue.length){var all=drainers;drainers=[];all.forEach(function(resolve){resolve()})}})}
    function drain(){return new Promise(function(resolve){drainers.push(resolve);checkDrain()})}
    function stop(){if(stopStarted||!recordConfirmed||!controller||controller.currentState.phase!=="recording")return;stopStarted=true;controller.stop().then(function(){
        check(commits.length===1&&commits[0]==="hello qt hosted transcript","Actual original draft insertion")
        check(controller.currentState.phase==="idle","Original controller finished idle")
        return drain()
    }).then(function(){controller.dispose();binding.dispose();return drain()}).then(function(){finished.resolve({states:states,actions:actions,commits:commits,permissionCount:permissionCount,realWire:true})},fail)}
    var host=factories.host({primitives:primitives,runtime:runtime,inputFactory:factories.input,preferencesFactory:factories.preferences,
        uuid:function(){return "qt-request-"+(++sequence)},
        send:function(packet){if(closed)throw new Error("Fixture wire closed");actions.push(packet.action);if(packet.requestId)pending.set(packet.requestId,packet.action);queue.push(JSON.stringify(packet))},
        readLatest:function(){return {clientSettings:settings,input:{environmentId:"draft-unavailable",ownerKey:draft.ownerKey,projectId:null,projectName:null,cleanup:false},cleanupPrepared:false}},
        onAvailable:function(value){if(value)available.resolve();else fail(new Error("Actual native Host unavailable"))},
        onPermission:function(session,id){permissionCount++;host.permission(session,id,true).catch(fail)},
        onLevel:function(level){if(recordConfirmed&&level>0)stop()},
        onDurationLimit:function(){fail(new Error("Unexpected recording duration limit"))},
        onText:function(){fail(new Error("Buffered path received stream text"))},
        onError:function(message){fail(new Error(message))},onCleanupError:fail})
    host.ownerChanged(draft.ownerKey,"draft-unavailable",1)
    available.promise.then(function(){return host.chooseAudio({kind:"environment",environmentId:"chosen"})}).then(function(){return host.chooseCleanup(false)}).then(function(){
        binding=host.controller();controller=new runtime.VoiceInputController({retryFailedRecording:true,recorder:binding.recorder,getTranscriber:function(){return binding.transcriber},requestPermission:function(){return Promise.resolve({granted:true,canAskAgain:true})},configureRecording:function(){return Promise.resolve()},releaseRecording:binding.releaseRecording,deleteRecording:binding.deleteRecording,postProcess:binding.postProcess,
            readDraft:function(){return draft},commitDraft:function(text,selection){commits.push(text);draft={ownerKey:draft.ownerKey,text:text,selection:selection,revision:draft.revision+1}},onStateChange:function(state){states.push(state.phase)},onPostProcessingError:function(){fail(new Error("Cleanup was disabled"))}})
        return controller.start()
    }).then(function(){check(controller.currentState.phase==="recording","Actual controller reached recording")},fail).catch(fail)
    return {
        result:finished.promise,
        takeWire:function(){var result=queue;queue=[];checkDrain();return result},
        receive:function(line){var envelope=JSON.parse(line);check(envelope.type==="native-voice"&&envelope.data,"Exact production metadata envelope");var event=envelope.data;if(event.kind==="result"){var action=pending.get(event.requestId);check(action,"Unexpected wire result");pending.delete(event.requestId);if(action==="record"&&!event.error)recordConfirmed=true;}host.receive(event);checkDrain()},
        close:function(){closed=true;host.lost()}
    }
}
