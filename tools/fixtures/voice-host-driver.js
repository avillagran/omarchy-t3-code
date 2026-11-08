// Exercise the real compiled controller and platform across the metadata bridge.
// This peer is deliberately synthetic; physical PCM/HTTP acceptance is separate.
function runNativeVoiceHostOracle(factories) {
    function check(value, message) { if(!value)throw new Error(message) }
    function settle() {var result=Promise.resolve();for(var i=0;i<100;i++)result=result.then(function(){});return result}
    var results=[]
    function scenario(mode, cleanup, deny, retry, lateEvents, unavailable, deferred) {
        var primitives=factories.primitives(),runtime=factories.voice(primitives)
        var sequence=0,session=0,requests=[],sessions={},states=[],commits=[],failures=0,binding,prepareError="",durationEvents=0,textEvents=0,microphoneEvents=0,heldPrepare=null,heldDispose=null
        var draft={ownerKey:"draft/one",text:"hello ",selection:{start:6,end:6},revision:1}
        var settings={voiceMicrophone:"chosen-mic",voiceShortcutMode:"auto",speechLanguage:"auto",
            speechTranslateToEnglish:false,speechCustomWords:[],speechSilenceFiltering:true,
            speechRemoveFillerWords:true,speechPostProcessingPrompt:{mode:"default",customInstructions:""},speechCorrectionWord:""}
        var choice={id:"grant-one",destination:{kind:"environment",environmentId:"audio"}}
        var snapshot={choiceJson:JSON.stringify(choice),cleanupJson:JSON.stringify(cleanup?"cleanup-one":null)}
        function reply(packet,value,failure) {host.receive({kind:"result",requestId:packet.requestId,session:packet.session,value:value||{},error:failure||null})}
        function send(packet) {
            requests.push({action:packet.action,session:packet.session||null})
            if(packet.action==="owner"||packet.action==="abort"||packet.action==="abort-request")return
            Promise.resolve().then(function(){
                var state=sessions[packet.session]
                switch(packet.action) {
                case "snapshot":reply(packet,snapshot);break
                case "decode-preferences":reply(packet,{valueJson:JSON.stringify(settings)});break
                case "project-context":reply(packet,{valueJson:JSON.stringify({speechProjectCustomWords:[{term:"ProjectTerm",aliases:[]}]})});break
                case "begin":var id="session-"+(++session);sessions[id]={released:false,stopped:false};reply(packet,{session:id,consent:snapshot});break
                case "configure":
                    state.options=JSON.parse(packet.optionsJson)
                    check(state.options.speechCustomWords.some(function(word){return word.term==="ProjectTerm"}),"original project vocabulary included")
                    check(packet.cleanupGrantJson===snapshot.cleanupJson,"captured cleanup identity")
                    reply(packet);break
                case "status":reply(packet,{valueJson:JSON.stringify({supported:true,supportsStreaming:mode==="streaming",supportsBufferedStreaming:false,supportsTranslation:false})});break
                case "stream-ticket":reply(packet);break
                case "stream-connect":state.connected=true;reply(packet);break
                case "capture-prepare":
                    check(packet.microphoneId==="chosen-mic","actual preference microphone")
                    check(packet.mode===mode,"original platform selected mode")
                    if(deferred&&packet.session==="session-1"){heldPrepare=packet;break}
                    if(deny){reply(packet,null,"capture_permission_denied");break}
                    host.receive({kind:"microphones",session:packet.session,devices:[{deviceId:"chosen-mic",label:"Selected microphone"}]})
                    reply(packet)
                    if(mode==="streaming") {check(state.connected,"capture follows handle insertion");host.receive({kind:"stream",session:packet.session,value:{event:"ready"}})}
                    break
                case "record":reply(packet);break
                case "stop":state.stopped=true;reply(packet,{uri:mode==="buffered"?"o3-voice-recording:"+packet.session:null});break
                case "release-capture":state.released=true;reply(packet);break
                case "stream-finish":check(state.released&&state.stopped,"finish follows actual release receipt");reply(packet,{text:"voice"});break
                case "transcribe":
                    check(state.released&&state.stopped,"buffered request after stop/release")
                    if(retry&&failures++===0)reply(packet,null,"voice_request_failed")
                    else reply(packet,{text:"voice"})
                    break
                case "post-process":
                    var input=JSON.parse(packet.inputJson)
                    check(input.transcript==="voice"&&input.draft.text==="hello ","original cleanup callback payload")
                    reply(packet,{text:"clean voice"});break
                case "delete-recording":reply(packet);break
                case "dispose":if(heldPrepare&&packet.session==="session-1")heldDispose=packet;else reply(packet);break
                default:throw new Error("Unexpected action "+packet.action)
                }
            })
        }
        var host=factories.host({primitives:primitives,runtime:runtime,inputFactory:factories.input,
            preferencesFactory:factories.preferences,uuid:function(){return "request-"+(++sequence)},send:send,
            readLatest:function(){return {clientSettings:settings,input:{environmentId:"draft",ownerKey:draft.ownerKey,projectId:"project",projectName:"Project",cleanup:true},cleanupPrepared:true}},
            onLevel:function(){},onDurationLimit:function(){durationEvents++},onText:function(){textEvents++},onError:function(message){throw new Error(message)},
            onMicrophones:function(devices){check(devices.length===1&&devices[0].deviceId==="chosen-mic"&&devices[0].label==="Selected microphone","native device metadata reaches current owner only");microphoneEvents++},
            onAvailable:function(value){if(!value&&controller){controller.dispose();binding.dispose()}},
            onPermission:function(){throw new Error("Synthetic peer has no microphone permission prompt")}})
        host.ownerChanged(draft.ownerKey,"draft",1)
        host.receive({kind:"ready",available:true})
        binding=host.controller()
        function makeController(bound){return new runtime.VoiceInputController({retryFailedRecording:true,recorder:bound.recorder,
            getTranscriber:function(){return {prepare:function(options){return bound.transcriber.prepare(options).catch(function(failure){prepareError=String(failure.message || failure);throw failure})}}},requestPermission:function(){return Promise.resolve({granted:true,canAskAgain:true})},
            configureRecording:function(){return Promise.resolve()},releaseRecording:bound.releaseRecording,deleteRecording:bound.deleteRecording,
            postProcess:bound.postProcess,onPostProcessingError:function(){throw new Error("Unexpected cleanup fallback")},
            readDraft:function(){return draft},commitDraft:function(text,selection){commits.push(text);draft=Object.assign({},draft,{text:text,selection:selection,revision:draft.revision+1})},
            onStateChange:function(state){states.push(state.phase)}})}
        var controller=makeController(binding)
        return settle().then(function(){
            var starting=controller.start()
            if(!deferred)return starting
            return settle().then(function(){
                check(controller.currentState.phase==="preparing"&&heldPrepare!==null,"old native microphone preparation deferred")
                var previousController=controller,previousBinding=binding
                previousController.dispose();previousBinding.dispose()
                binding=host.controller();controller=makeController(binding)
                var next=controller.start()
                return settle().then(function(){
                    check(session===1&&controller.currentState.phase==="preparing","successor waits for actual old preparation terminal")
                    check(heldDispose!==null,"old owner cleanup dispatched")
                    var preparePacket=heldPrepare,disposePacket=heldDispose;heldPrepare=null;heldDispose=null
                    reply(preparePacket,null,"voice_cancelled");reply(disposePacket)
                    return Promise.all([starting,next])
                })
            })
        }).then(function(){
            if(deny) {
                check(controller.currentState.phase==="error","permission refusal reaches original controller")
                check(/Microphone access/.test(controller.currentState.error),"NotAllowedError keeps original permission message")
                check(!requests.some(function(request){return request.action==="record"}),"permission denial cannot start recording")
                return
            }
            check(controller.currentState.phase==="recording","actual controller recording: "+mode+" "+controller.currentState.error+" "+prepareError+" "+JSON.stringify(requests))
            check(microphoneEvents===1,"prepared capture publishes one current microphone list")
            if(unavailable){
                host.receive({kind:"ready",available:false})
                return settle().then(function(){
                    check(controller.currentState.phase==="idle","unavailable owner clears submission block")
                    check(requests.some(function(request){return request.action==="release-capture"||request.action==="dispose"}),"cleanup remains routable on live unavailable channel")
                    check(commits.length===0,"availability loss cannot commit audio")
                })
            }
            return controller.stop().then(function(){
                if(retry) {
                    check(controller.currentState.phase==="error","first transport error retained for explicit retry")
                    if(lateEvents){
                        var previousController=controller,previousBinding=binding,oldSession="session-"+session
                        binding=host.controller();controller=makeController(binding)
                        return controller.start().then(function(){
                            check(controller.currentState.phase==="recording","successor recording")
                            host.receive({kind:"capture-error",session:oldSession,error:"late-old-error"})
                            host.receive({kind:"duration-limit",session:oldSession})
                            host.receive({kind:"stream",session:oldSession,value:{event:"text",text:{committed:"old",tentative:""}}})
                            host.receive({kind:"microphones",session:oldSession,devices:[{deviceId:"old-mic",label:"Stale microphone"}]})
                            check(durationEvents===0&&textEvents===0,"old retained recording cannot update successor callbacks")
                            check(microphoneEvents===2,"old retained recording cannot replace successor device metadata")
                            previousController.dispose();previousBinding.dispose()
                            return settle().then(function(){
                                check(!sessions["session-"+session].released,"old cleanup cannot release successor capture")
                                var before=requests.length
                                previousBinding.deleteRecording("o3-voice-recording:"+oldSession)
                                check(requests.length===before,"confirmed disposal forgets old URI routing")
                                return controller.stop()
                            })
                        })
                    }
                    return controller.start().then(function(){check(requests.filter(function(r){return r.action==="capture-prepare"}).length===1,"retry keeps original recording")})
                }
            }).then(function(){
                check(commits.length===1&&commits[0]==="hello "+(cleanup?"clean voice":"voice"),"original controller insertion")
                check(controller.currentState.phase==="idle","finished controller idle")
            })
        }).then(function(){
            controller.dispose();binding.dispose();return settle()
        }).then(function(){
            results.push({mode:mode,cleanup:cleanup,denied:deny,retry:retry,lateEvents:!!lateEvents,unavailable:!!unavailable,deferred:!!deferred,states:states,commits:commits,actions:requests.map(function(request){return request.action})})
        })
    }
    function snapshotOrdering(){
        var primitives=factories.primitives(),packet,seen=[]
        var host=factories.host({primitives:primitives,runtime:factories.voice(primitives),inputFactory:factories.input,preferencesFactory:factories.preferences,
            uuid:function(){return "snapshot-request"},send:function(command){if(command.action==="snapshot")packet=command},
            onConsent:function(choice){seen.push(choice.id)}})
        host.receive({kind:"ready",available:true})
        host.receive({kind:"result",requestId:packet.requestId,value:{choiceJson:'{"id":"old"}',cleanupJson:"null"}})
        host.receive({kind:"consent",generation:1,value:{choiceJson:'{"id":"new"}',cleanupJson:"null"}})
        return settle().then(function(){check(seen.join(",")==="new","older snapshot Promise cannot roll back newer consent event");results.push({snapshotOrdering:true})})
    }
    return scenario("buffered",false,false,false)
        .then(function(){return scenario("streaming",false,false,false)})
        .then(function(){return scenario("buffered",true,false,false)})
        .then(function(){return scenario("buffered",false,true,false)})
        .then(function(){return scenario("buffered",false,false,true)})
        .then(function(){return scenario("buffered",false,false,true,true)})
        .then(function(){return scenario("buffered",false,false,false,false,true)})
        .then(function(){return scenario("buffered",false,false,false,false,false,true)})
        .then(snapshotOrdering)
        .then(function(){return {count:results.length,cases:results}})
}
