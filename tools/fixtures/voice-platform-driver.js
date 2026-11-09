// Shared unchanged by the original Node module and compiled Qt platform.
// Capture, fetch and SDK boundaries are explicit in-memory test doubles.
function runVoicePlatformOracle(factory) {
    function units(value) { var out=[];for(var i=0;i<value.length;i++)out.push(value.charCodeAt(i));return out }
    function clean(value) {
        if(typeof value==='string')return {utf16:units(value)};
        if(Array.isArray(value))return value.map(clean);
        if(value&&typeof value==='object'){var out={};Object.keys(value).sort().forEach(function(key){if(value[key]!==undefined)out[key]=clean(value[key])});return out}
        return value;
    }
    var cases=[],scenarios=[];
    [false,true].forEach(function(streaming){[false,true].forEach(function(buffered){[false,true].forEach(function(translation){
        [undefined,[],['ja'],['es','en'],['ja','es']].forEach(function(languages){[false,true].forEach(function(detection){
            ['auto','es','zz'].forEach(function(language){[false,true].forEach(function(translate){
                scenarios.push({status:{supported:true,supportsStreaming:streaming,supportsBufferedStreaming:buffered,supportsTranslation:translation,languages:languages,supportsLanguageDetection:detection},language:language,translate:translate});
            })});
        })});
    })})});
    ['openai','xai','openrouter'].forEach(function(provider){['auto','es','zz'].forEach(function(language){
        scenarios.push({provider:provider,model:'author/model',language:language,translate:true});
    })});
    scenarios.push({provider:'openrouter',language:'auto'});
    scenarios.push({status:{supported:false,reason:'unavailable \ud800'},language:'auto'});
    ['status','key','ticket','fetch','transcribe','after-transcribe'].forEach(function(abort){
        scenarios.push({status:{supported:true,supportsStreaming:true},provider:abort==='key'?'openai':undefined,language:'auto',abort:abort});
    });
    ['plain','typed','lookalike'].forEach(function(failure){scenarios.push({status:{supported:true},language:'auto',failure:failure})});
    function runScenario(scenario) {
        var trace=[],consent=true,options={speechLanguage:scenario.language,speechTranslateToEnglish:!!scenario.translate,speechCustomWords:['project \udfff']};
        var controller,api,prepared={identity:'private-prepared'},recording={identity:'recording'},textCallback=function(){},errorCallback;
        function operation(kind,extra) { trace.push(['operation',kind,extra||null]);return {kind:kind} }
        function abortAt(kind) { if(scenario.abort===kind)controller.abort() }
        var deps={
            createBrowserPcmCapture:function(input,prepare){trace.push(['capture-factory',input===inputValue]);return {prepare:prepare}},
            getEnvironmentSpeechStatus:function(p){trace.push(['same-prepared',p===prepared]);return operation('status')},
            requireCloudSpeechKey:function(p,provider){trace.push(['same-prepared',p===prepared]);return operation('key',provider)},
            getEnvironmentSpeechStreamUrl:function(p){trace.push(['same-prepared',p===prepared]);return operation('ticket')},
            openSpeechStream:function(input){trace.push(['stream',input.url,input.options===options,input.signal===controller.signal,input.canSend(),input.onText===textCallback]);errorCallback=input.onError;return {marker:'stream'}},
            transcribeEnvironmentPcm:function(p,pcm,opts,guard){trace.push(['upload-local',p===prepared,Array.prototype.slice.call(pcm),opts===options,guard()]);return operation('transcribe')},
            transcribeCloudPcm:function(p,provider,pcm,opts,guard,model){trace.push(['upload-cloud',p===prepared,provider,model,Array.prototype.slice.call(pcm),opts===options,guard()]);return operation('transcribe')},
            fetch:function(uri,opts){trace.push(['fetch',uri,opts.signal===controller.signal]);abortAt('fetch');return Promise.resolve({arrayBuffer:function(){return Promise.resolve(new Uint8Array([1,2,3,4]).buffer)}})},
            Uint8Array:Uint8Array,
            runEnvironmentSpeech:function(op,opts){trace.push(['run',op.kind,opts.signal===controller.signal]);abortAt(op.kind);if(op.kind==='status')return Promise.resolve(scenario.status);if(op.kind==='ticket')return Promise.resolve('opaque-test-ticket');
                if(op.kind==='transcribe'){
                    if(scenario.failure==='plain')return Promise.reject(new Error('private failure'));
                    if(scenario.failure==='typed')return Promise.reject(new api.VoiceTranscriptionError({code:'unavailable',message:'typed \ud800'}));
                    if(scenario.failure==='lookalike')return Promise.reject({_tag:'VoiceTranscriptionError',code:'unavailable',message:'plain object'});
                    abortAt('after-transcribe');return Promise.resolve({text:'transcript \ud800'});
                }return Promise.resolve(undefined);
            }
        };
        api=factory(deps);controller=new api.AbortController();
        var inputValue={prepared:prepared,cloudProvider:scenario.provider,cloudModel:scenario.model,hasAudioConsent:function(){return consent},getTranscriptionOptions:function(){trace.push(['options']);return options},getMicrophoneId:function(){return ''},onLevel:function(){},onDurationLimit:function(){},onText:textCallback,onError:function(message){trace.push(['error-callback',message])}};
        var platform=api.createBrowserVoiceInputPlatform(inputValue);
        return Promise.resolve().then(function(){return platform.prepare({signal:controller.signal})}).then(function(capture){
            trace.push(['prepared',capture.locale,!!capture.streaming]);
            if(errorCallback)errorCallback({message:'stream error \udfff'});
            // The transcriber must retain the preparation's original options.
            inputValue.getTranscriptionOptions=function(){throw new Error('Options were not captured')};
            consent=false;
            return capture.transcribe('owned-memory-recording',{signal:controller.signal});
        }).then(function(result){return {text:result}},function(error){return {code:error.code,message:error.message,typed:api.errorPredicate(error)}})
          .then(function(outcome){cases.push(clean({trace:trace,outcome:outcome}))});
    }
    return scenarios.reduce(function(chain,scenario){return chain.then(function(){return runScenario(scenario)})},Promise.resolve())
      .then(function(){return {cases:cases,count:cases.length}});
}
