// Shared unchanged between original Node modules and the Qt-compiled controller.
// All recording URIs and deletion callbacks are in-memory observations only.
function runVoiceOracle(factory) {
    function units(value) { var output=[];for(var i=0;i<value.length;i++)output.push(value.charCodeAt(i));return output }
    function normalized(value) {
        if(typeof value==='string')return {utf16:units(value)};
        if(Array.isArray(value))return value.map(normalized);
        if(value&&typeof value==='object'){var output={};Object.keys(value).sort().forEach(function(key){output[key]=normalized(value[key])});return output}
        return value;
    }
    function drain() { var chain=Promise.resolve();for(var i=0;i<96;i++)chain=chain.then(function(){});return chain }
    var errors=[],constructors=[],errorApi=factory();
    var errorValues=[null,undefined,1,true,'error',{},[],function(){},new Error('plain')];
    ['unavailable','unsupported-locale','preparation-failed','transcription-failed','cancelled','unknown',''].forEach(function(code){
        try{var constructed=new errorApi.VoiceTranscriptionError({code:code,message:'source error'});constructors.push({accepted:true});errorValues.push(constructed)}catch(error){constructors.push({threw:error.message})}
        [undefined,null,0,{},new Error('cause')].forEach(function(cause){errorValues.push({_tag:'VoiceTranscriptionError',code:code,message:'struct error',cause:cause})});
    });
    var inherited=Object.create({_tag:'VoiceTranscriptionError',code:'cancelled',message:'inherited'});
    var array=[],callable=function(){};
    [array,callable].forEach(function(value){value._tag='VoiceTranscriptionError';value.code='cancelled';value.message='properties'});
    errorValues.push(inherited,array,callable,Object.assign(Object.create(null),{_tag:'VoiceTranscriptionError',code:'cancelled',message:'null prototype'}));
    [undefined,null,false,0,'brand'].forEach(function(marker){var value={};value['~effect/Schema/Class/VoiceTranscriptionError']=marker;errorValues.push(value)});
    var mutated=new errorApi.VoiceTranscriptionError({code:'cancelled',message:'mutated'});mutated.code=7;mutated.message=null;errorValues.push(mutated);
    var getter={_tag:'VoiceTranscriptionError',code:'cancelled'};Object.defineProperty(getter,'message',{get:function(){throw Error('getter refused')}});errorValues.push(getter);
    errorValues.forEach(function(value){try{errors.push({accepted:errorApi.errorPredicate(value)})}catch(error){errors.push({threw:error.message})}});
    var pure=[], texts=['','hello','a b','界文','é x','e\u0301 ','😀 x','\ud800x','x\udfff','𐐀 x','(hello)',"a'",'１２ x','ไทย','x\n','x\u2028'];
    var transcripts=['word',' é ','界','日本語','e\u0301','😀','\ud800','\udfff','１２',''],locales=['auto','en','es-CL','ja','zh_Hant','yue','th','lo','km','my','AUTO','en_US'];
    texts.forEach(function(text,textIndex){transcripts.forEach(function(transcript,transcriptIndex){locales.forEach(function(locale,localeIndex){
        var api=factory(),positions=[0,text.length,Math.floor(text.length/2),-1,text.length+2,0.5];
        var start=positions[(textIndex+transcriptIndex+localeIndex)%positions.length];
        var captured={ownerKey:'draft:a',text:text,revision:7,selection:{start:start,end:(localeIndex%4===0?text.length:start)}};
        var current={ownerKey:'draft:a',text:text,revision:7,selection:{start:0,end:0}};
        if(localeIndex===8)current.revision++;
        if(localeIndex===9)current.ownerKey='draft:b';
        if(localeIndex===10)current.text+='changed';
        if(localeIndex===11)current=null;
        pure.push(api.resolveTranscriptCommit(captured,current,transcript,locale));
    })})});
    var cases=[
        {name:'buffered'}, {name:'empty',text:''}, {name:'missing-draft',draft:null}, {name:'unavailable',unavailable:true},
        {name:'permission-retry',permission:{granted:false,canAskAgain:true}}, {name:'permission-settings',permission:{granted:false,canAskAgain:false}},
        {name:'permission-throws',fail:{permission:'notAllowed'}}, {name:'permission-generic',fail:{permission:'generic'}},
        {name:'prepare-unsupported',fail:{prepare:'unsupported-locale'}}, {name:'prepare-native',fail:{prepare:'preparation-failed'}},
        {name:'prepare-generic',fail:{prepare:'generic'}}, {name:'prepare-busy',fail:{prepare:'busy'}},
        {name:'configure-failed',fail:{configure:'generic'}}, {name:'recorder-failed',fail:{recorder:'notAllowed'}},
        {name:'stop-failed',fail:{stop:'generic'}}, {name:'transcribe-native',fail:{transcribe:'transcription-failed'}},
        {name:'transcribe-generic',fail:{transcribe:'generic'}}, {name:'transcribe-busy',fail:{transcribe:'busy'}},
        {name:'cleanup-success',post:'clean'}, {name:'cleanup-failure',post:'clean',fail:{post:'generic'}},
        {name:'cleanup-null',post:null}, {name:'streaming',streaming:true}, {name:'live',live:'live result'},
        {name:'live-fallback',live:null}, {name:'live-error',live:'result',fail:{live:'transcription-failed'}},
        {name:'no-uri',noUri:true}, {name:'streaming-no-uri',streaming:true,noUri:true},
        {name:'stale-revision',steps:['start','editRevision','stop']}, {name:'stale-text',steps:['start','editText','stop']},
        {name:'stale-owner',steps:['start','editOwner','stop']}, {name:'selection-only',steps:['start','selection','stop']},
        {name:'cancel-recording',steps:['start','cancel']}, {name:'dispose-recording',steps:['start','dispose']},
        {name:'owner-recording',steps:['start','ownerChanged']}, {name:'background-recording',steps:['start','background']},
        {name:'status-error',steps:['start','statusError']}, {name:'status-finished',steps:['start','statusFinished']},
        {name:'status-foreign',steps:['start','statusForeign','stop']}, {name:'status-no-uri',steps:['start','statusNoUri']},
        {name:'cancel-permission',gates:['permission'],steps:['start','cancel','resolve:permission']},
        {name:'dispose-prepare',gates:['prepare'],steps:['start','dispose','resolve:prepare']},
        {name:'owner-configure',gates:['configure'],steps:['start','ownerChanged','resolve:configure']},
        {name:'background-prepare-recorder',gates:['recorder'],steps:['start','background','resolve:recorder']},
        {name:'cancel-transcribe',gates:['transcribe'],steps:['start','stop','cancel','resolve:transcribe']},
        {name:'dispose-transcribe',gates:['transcribe'],steps:['start','stop','dispose','resolve:transcribe']},
        {name:'cleanup-skip',post:'clean',gates:['post'],steps:['start','stop','skip','resolve:post']},
        {name:'cleanup-cancel',post:'clean',gates:['post'],steps:['start','stop','cancel','resolve:post']},
        {name:'retry-buffered',retry:true,failOnce:{transcribe:'transcription-failed'},steps:['start','stop','start']},
        {name:'retry-change-owner',retry:true,failOnce:{transcribe:'transcription-failed'},steps:['start','stop','editOwner','start','stop']},
        {name:'retry-cancel',retry:true,failOnce:{transcribe:'transcription-failed'},steps:['start','stop','cancel']},
        {name:'simultaneous-controller',steps:['start','secondStart','stop','secondStart','secondStop']},
        {name:'abandoned-session-awaits',gates:['prepare'],steps:['start','cancel','secondStart','resolve:prepare','secondStop']},
        {name:'duplicate-stop',gates:['transcribe'],steps:['start','stop','stop','resolve:transcribe']}
    ];
    function scenario(input) {
        var api=factory(),observations=[],snapshots=[],pending=[],jobs=[],usedOnce={},draft=input.draft===null?null:{ownerKey:'draft:a',text:'hello ',revision:4,selection:{start:6,end:6}};
        function record(name,value){observations.push([name,value===undefined?null:JSON.parse(JSON.stringify(value))])}
        function error(kind){if(kind==='generic')return new Error('synthetic failure');if(kind==='busy')return new Error('voice-operation-busy');if(kind==='notAllowed'){var value=new Error('permission refused');value.name='NotAllowedError';return value}return new api.VoiceTranscriptionError({code:kind,message:'native '+kind})}
        function operation(name,value,signal){
            record(name,{aborted:signal?signal.aborted:false});
            if(signal)signal.addEventListener('abort',function(){record('abort:'+name)} ,{once:true});
            var failure=input.fail&&input.fail[name];
            if(!failure&&input.failOnce&&input.failOnce[name]&&!usedOnce[name]){failure=input.failOnce[name];usedOnce[name]=true}
            var settle=function(resolve,reject){if(failure)reject(error(failure));else resolve(value)};
            if(input.gates&&input.gates.indexOf(name)>=0)return new Promise(function(resolve,reject){pending.push({name:name,finish:function(){settle(resolve,reject)}})});
            return new Promise(settle);
        }
        function dependencies(label){
            var recorder={uri:input.noUri?null:'memory:recording/'+label,
                prepareToRecordAsync:function(signal){return operation('recorder',undefined,signal)},
                record:function(options){record('record:'+label,options)},stop:function(){return operation('stop',undefined)}};
            var prepared={locale:'en',transcribe:function(uri,options){record('transcribe-uri',uri);return operation('transcribe',input.text===undefined?'voice':input.text,options.signal)}};
            if(Object.prototype.hasOwnProperty.call(input,'live'))prepared.start=function(uri,options){record('live-uri',uri);return operation('live',input.live,options.signal)};
            if(input.streaming)prepared.streaming={finish:function(options){return operation('streaming','stream result',options.signal)}};
            return {recorder:recorder,retryFailedRecording:!!input.retry,
                getTranscriber:function(){record('get-transcriber');return input.unavailable?null:{prepare:function(options){return operation('prepare',prepared,options.signal)}}},
                requestPermission:function(){return operation('permission',input.permission||{granted:true,canAskAgain:true})},
                configureRecording:function(){return operation('configure',undefined)},releaseRecording:function(){return operation('release',undefined)},
                deleteRecording:function(uri){record('delete-plan',uri)},
                postProcess:function(text,options){record('post-input',{text:text,draft:options.draft});return input.post?operation('post',input.post,options.signal):null},
                onPostProcessingError:function(value){record('post-error',{code:value.code||null,message:value.message})},
                readDraft:function(){record('read-draft',draft);return draft?{ownerKey:draft.ownerKey,text:draft.text,revision:draft.revision,selection:{start:draft.selection.start,end:draft.selection.end}}:null},
                commitDraft:function(text,selection){record('commit',{text:text,selection:selection});draft={ownerKey:draft.ownerKey,text:text,revision:draft.revision+1,selection:selection}},
                onStateChange:function(state){record('state:'+label,{phase:state.phase,error:state.error,errorAction:state.errorAction,blocks:api.voiceInputBlocksSubmission(state)})}};
        }
        var controller=new api.VoiceInputController(dependencies('a')),second=new api.VoiceInputController(dependencies('b'));
        function launch(name,job){var state={name:name,state:'pending'};jobs.push(state);Promise.resolve(job).then(function(){state.state='resolved'},function(value){state.state='rejected';state.error=value.message})}
        function step(action){
            if(action==='start')launch(action,controller.start());else if(action==='stop')launch(action,controller.stop());
            else if(action==='secondStart')launch(action,second.start());else if(action==='secondStop')launch(action,second.stop());
            else if(action==='cancel')controller.cancel();else if(action==='dispose')controller.dispose();else if(action==='ownerChanged')controller.ownerChanged();
            else if(action==='background')launch(action,controller.appMovedToBackground());else if(action==='skip')controller.skipPostProcessing();
            else if(action==='editRevision')draft.revision++;else if(action==='editText')draft.text+='changed';else if(action==='editOwner')draft.ownerKey='draft:b';else if(action==='selection')draft.selection={start:0,end:0};
            else if(action.indexOf('resolve:')===0){var index=pending.findIndex(function(value){return value.name===action.slice(8)});if(index<0)throw Error(input.name+': missing deferred '+action);pending.splice(index,1)[0].finish()}
            else if(action.indexOf('status')===0)launch(action,controller.handleRecorderStatus({isFinished:action!=='statusError',hasError:action==='statusError',error:action==='statusError'?'device interrupted':null,url:action==='statusNoUri'?null:action==='statusForeign'?'memory:foreign':'memory:recording/a'}));
            else throw Error('Unknown scenario action '+action);
            return drain().then(function(){snapshots.push({after:action,state:controller.currentState,second:second.currentState,jobs:jobs.map(function(value){return Object.assign({},value)}),observations:observations.splice(0)})});
        }
        var chain=Promise.resolve();(input.steps||['start','stop']).forEach(function(action){chain=chain.then(function(){return step(action)})});
        return chain.then(function(){controller.dispose();second.dispose();pending.splice(0).forEach(function(value){value.finish()});return drain()}).then(function(){
            if(jobs.some(function(value){return value.state==='pending'}))throw Error(input.name+': unsettled operation');
            return {name:input.name,snapshots:snapshots,final:{draft:draft,state:controller.currentState,second:second.currentState,jobs:jobs,observations:observations}};
        });
    }
    var traces=[],chain=Promise.resolve();cases.forEach(function(input){chain=chain.then(function(){return scenario(input)}).then(function(value){traces.push(value)})});
    return chain.then(function(){return {constructors:normalized(constructors),errors:normalized(errors),errorCases:errors.length,pure:normalized(pure),traces:normalized(traces),pureCases:pure.length,asyncCases:cases.length}});
}
