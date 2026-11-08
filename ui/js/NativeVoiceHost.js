.pragma library

// Metadata-only linkage between the original controller/platform and the owned
// native capture/SDK process. Audio and transport tickets stay in that process.
function create(adapters) {
    var pending = new Map(), scopes = new Map(), recordings = new Map()
    var capabilities = new WeakMap(), streamTickets = new WeakMap()
    var current = null, available = false, stopped = false
    var choice = null, cleanupGrant = null, notificationEpoch=0, owner = {key:"", environment:"", epoch:0}
    var runtime = adapters.runtime, primitives = adapters.primitives

    function error(code) {
        if (code === "capture_permission_denied") {
            var denied = new Error("Microphone permission was denied.")
            denied.name = "NotAllowedError"
            return denied
        }
        if (["voice_cancelled", "voice_consent_revoked", "voice_owner_changed"].indexOf(code) >= 0)
            return new runtime.VoiceTranscriptionError({code:"cancelled", message:code})
        var failure = new Error(code || "voice_host_unavailable")
        failure.code = code
        return failure
    }
    function send(value) { if (!stopped) adapters.send(value) }
    function rpc(action, fields, scope, signal) {
        var cleanup=["release-capture","delete-recording","dispose"].indexOf(action)>=0
        if ((!available && !cleanup) || stopped) return Promise.reject(error("voice_host_unavailable"))
        var id = adapters.uuid(), abort
        var result = new Promise(function(resolve, reject) {
            abort = function() { send({action:"abort-request", target:id}) }
            pending.set(id, {resolve:resolve, reject:reject, scope:scope, signal:signal, abort:abort})
            send(Object.assign({action:action, requestId:id}, scope ? {session:scope.id} : {}, fields || {}))
            if (signal) {
                signal.addEventListener("abort", abort, {once:true})
                if (signal.aborted) abort()
            }
        })
        // Cancellation waits for the native terminal receipt. In particular, a
        // cancelled microphone prompt must drain before the global voice owner.
        return result
    }
    function updateSnapshot(value, expectedEpoch) {
        if(expectedEpoch!==undefined && expectedEpoch!==notificationEpoch)return
        if (!value || typeof value.choiceJson !== "string" || typeof value.cleanupJson !== "string") throw error("voice_consent_unavailable")
        choice = JSON.parse(value.choiceJson)
        cleanupGrant = JSON.parse(value.cleanupJson)
        if (adapters.onConsent) adapters.onConsent(choice, cleanupGrant)
    }
    function snapshotRpc(action, fields) {
        var epoch=notificationEpoch
        return rpc(action,fields).then(function(value){updateSnapshot(value,epoch);return value})
    }
    function active(scope) {
        return available && !scope.disposed && scope.owner.key === owner.key &&
            scope.owner.environment === owner.environment && scope.owner.epoch === owner.epoch &&
            choice && choice !== "off" && choice.id === scope.grantId
    }
    function abortScope(scope) { if (scope) send({action:"abort", session:scope.id}) }
    function live(scope) {return scope===current && active(scope) && !(scope.signal && scope.signal.aborted)}
    function dispose(scope) {
        if (!scope || scope.disposing) return Promise.resolve()
        scope.disposed = true
        abortScope(scope)
        scope.disposing = rpc("dispose", {}, scope).then(function() {
            if(scope.signal)scope.signal.removeEventListener("abort",scope.abort)
            scope.uris.forEach(function(uri){if(recordings.get(uri)===scope)recordings.delete(uri)})
            scope.uris.clear()
            scopes.delete(scope.id)
        })
        // Keep a failed cleanup owner reachable. Unknown is not a release receipt.
        scope.disposing.catch(function(failure) { if (adapters.onCleanupError) adapters.onCleanupError(failure) })
        return scope.disposing
    }
    function collect(scope) {
        if (!scope || scope === current || scope.uris.size) return
        var busy = false
        pending.forEach(function(request) { if (request.scope === scope) busy = true })
        if (!busy) dispose(scope)
    }
    function task(scope, action, fields, transform) {
        return {scope:scope, action:action, fields:fields || {}, transform:transform}
    }
    function runTask(value, options) {
        if (!value || !value.scope || scopes.get(value.scope.id) !== value.scope) return Promise.reject(error("voice_recording_unavailable"))
        var scope = value.scope
        return scope.configured.then(function() {
            if (options && options.signal) options.signal.throwIfAborted()
            return rpc(value.action, value.fields, scope, options && options.signal)
        }).then(function(result) { return value.transform ? value.transform(result) : result })
    }
    function scopeFor(prepared) {
        var scope = prepared && scopes.get(prepared.id)
        if (!scope || scope.prepared !== prepared) throw error("voice_owner_changed")
        return scope
    }
    function transcribe(prepared, pcm) {
        var scope = scopeFor(prepared), capability = capabilities.get(pcm)
        if (!capability || capability.scope !== scope || recordings.get(capability.uri) !== scope) throw error("voice_recording_unavailable")
        return task(scope, "transcribe", {uri:capability.uri})
    }
    function opaquePcm(value) {
        if (!capabilities.has(value)) throw error("voice_recording_unavailable")
        return value
    }
    var nativeInput = adapters.inputFactory({
        Schema:primitives.Schema, VoiceTranscriptionError:runtime.VoiceTranscriptionError,
        throwIfVoiceTranscriptionAborted:runtime.throwIfVoiceTranscriptionAborted,
        runEnvironmentSpeech:runTask,
        getEnvironmentSpeechStatus:function(prepared) {
            return task(scopeFor(prepared), "status", {}, function(value) { return JSON.parse(value.valueJson) })
        },
        requireCloudSpeechKey:function(prepared, provider) {
            var scope = scopeFor(prepared)
            return task(scope, "cloud-keys", {}, function(keys) {
                if (keys[provider]) return undefined
                return rpc("cloud-message", {provider:provider, reason:"missing-key"}).then(function(value) {
                    throw new runtime.VoiceTranscriptionError({code:"unavailable", message:value.message})
                })
            })
        },
        getEnvironmentSpeechStreamUrl:function(prepared) {
            var scope = scopeFor(prepared)
            return task(scope, "stream-ticket", {}, function() {
                var ticket = {}; streamTickets.set(ticket, scope); return ticket
            })
        },
        openSpeechStream:function(input) {
            var scope = streamTickets.get(input.url)
            if (!scope || scope.stream) throw error("voice_already_prepared")
            streamTickets.delete(input.url)
            var resolveReady, rejectReady
            var stream = {ready:new Promise(function(resolve, reject) { resolveReady=resolve;rejectReady=reject }),
                resolveReady:resolveReady, rejectReady:rejectReady, onText:input.onText, onError:input.onError,
                finish:function(options) { return rpc("stream-finish", {}, scope, options.signal).then(function(value) { return value.text }) },
                cancel:function() { abortScope(scope) }}
            scope.stream = stream
            stream.ready.catch(function() {})
            stream.started = rpc("stream-connect", {}, scope, input.signal)
            stream.started.catch(function(failure) { rejectReady(failure) })
            return stream
        },
        transcribeEnvironmentPcm:transcribe,
        transcribeCloudPcm:function(prepared, provider, pcm) { return transcribe(prepared, pcm) },
        fetch:function(uri, options) {
            var scope = recordings.get(uri)
            if (!scope) return Promise.reject(error("voice_recording_unavailable"))
            if (options.signal) options.signal.throwIfAborted()
            var capability = {}; capabilities.set(capability, {scope:scope, uri:uri})
            return Promise.resolve({arrayBuffer:function() { return Promise.resolve(capability) }})
        },
        Uint8Array:opaquePcm,
        createBrowserPcmCapture:function(input, prepare) {
            var scope = current, uri = null, transcription = null, captureSignal
            var recorder = {
                get uri() { return uri },
                prepareToRecordAsync:function() {
                    if (!captureSignal) return Promise.reject(error("voice_not_prepared"))
                    captureSignal.throwIfAborted()
                    return rpc("capture-prepare", {mode:transcription.streaming ? "streaming" : "buffered", microphoneId:input.getMicrophoneId()}, scope, captureSignal)
                },
                record:function(options) {
                    rpc("record", {seconds:options.forDuration}, scope, captureSignal).catch(function(failure) {
                        if (live(scope)) input.onError(failure.message)
                    })
                },
                stop:function() {
                    return rpc("stop", {}, scope).then(function(value) {
                        if(scope.disposed){uri=null;return}
                        uri = value.uri || null
                        if (uri) { recordings.set(uri, scope); scope.uris.add(uri) }
                    })
                }
            }
            scope.captureInput = input
            return {recorder:recorder,
                cancelRecording:function() { return rpc("release-capture", {}, scope) },
                deleteRecording:function(value) {
                    if (recordings.get(value) !== scope) return
                    rpc("delete-recording", {uri:value}, scope).then(function() {
                        recordings.delete(value);scope.uris.delete(value);if (uri===value)uri=null;collect(scope)
                    }).catch(function(failure) { if (adapters.onCleanupError) adapters.onCleanupError(failure) })
                },
                transcriber:{prepare:function(options) {
                    captureSignal = options.signal
                    return prepare(options).then(function(value) {
                        transcription = value
                        // This ACK confirms native handle insertion, not WS ready.
                        return value.streaming ? value.streaming.started.then(function() { return value }) : value
                    }).then(function(value) { options.signal.throwIfAborted(); return value })
                }}}
        }
    })
    function latest(scope, settings) {
        var value = adapters.readLatest()
        return Object.assign({}, value, {clientSettings:settings, projectWords:scope.projectWords, cleanupPrepared:value.cleanupPrepared ? scope.prepared : null})
    }
    function prepare(options, binding) {
        if(binding.disposed)return Promise.reject(error("voice_owner_changed"))
        var capturedOwner = Object.assign({}, owner), capturedChoice = choice
        if (!capturedChoice || capturedChoice === "off" || !capturedChoice.id) return Promise.reject(error("voice_consent_unavailable"))
        var initial = adapters.readLatest(), raw = initial.clientSettings
        return Promise.all([
            rpc("decode-preferences", {valueJson:JSON.stringify(raw)}, null, options.signal),
            rpc("project-context", {projectId:initial.input.projectId || null}, null, options.signal)
        ]).then(function(metadata) {
            var decoded=metadata[0],projectWords=JSON.parse(metadata[1].valueJson).speechProjectCustomWords
            var epoch=notificationEpoch
            return rpc("begin", {ownerKey:capturedOwner.key, grantIdJson:JSON.stringify(capturedChoice.id)}, null, options.signal).then(function(value) {
                updateSnapshot(value.consent,epoch)
                var previous = current
                var scope = {id:value.session, owner:capturedOwner, grantId:capturedChoice.id,
                    prepared:{id:value.session}, projectWords:projectWords, uris:new Set(), disposed:false, configured:Promise.resolve()}
                scopes.set(scope.id, scope);current=scope;binding.scope=scope
                scope.signal=options.signal;scope.abort=function(){abortScope(scope);if(scope.stream)scope.stream.rejectReady(error("voice_cancelled"))}
                options.signal.addEventListener("abort",scope.abort,{once:true})
                collect(previous)
                if (binding.disposed || options.signal.aborted || !active(scope)) {
                    abortScope(scope);dispose(scope);options.signal.throwIfAborted();throw error("voice_owner_changed")
                }
                var settings = JSON.parse(decoded.valueJson)
                scope.latestRef = {current:latest(scope, settings)}
                scope.preferences = adapters.preferencesFactory({latestRef:scope.latestRef,flatMap:primitives.flatMap,
                    hasAudioConsent:function() { return active(scope) },
                    readCleanupGrant:function() { return cleanupGrant }, runEnvironmentSpeech:runTask,
                    postProcessEnvironmentTranscript:function(target, transcript, draft, preferences) {
                        return task(scope, "post-process", {inputJson:JSON.stringify({transcript:transcript, draft:draft, options:preferences})})
                    }})
                scope.platform = nativeInput.createBrowserVoiceInputPlatform({prepared:scope.prepared,
                    cloudProvider:capturedChoice.destination.kind === "cloud" ? capturedChoice.destination.provider : undefined,
                    cloudModel:capturedChoice.destination.model,
                    hasAudioConsent:function() { return active(scope) },
                    getTranscriptionOptions:function() {
                        var result = scope.preferences.getTranscriptionOptions()
                        scope.configured = rpc("configure", {optionsJson:JSON.stringify(result), cleanupGrantJson:JSON.stringify(scope.preferences.getRecordingPreferences().cleanupGrant)}, scope, options.signal)
                        return result
                    },
                    getMicrophoneId:function() { return settings.voiceMicrophone },
                    onLevel:adapters.onLevel, onDurationLimit:adapters.onDurationLimit,
                    onText:adapters.onText, onError:adapters.onError})
                return scope.platform.transcriber.prepare(options)
            })
        })
    }
    var api = {
        controller:function() {
            // Each controller keeps its own capture facade. A disposed editor's
            // delayed finally block cannot stop or release a successor's mic.
            var binding={scope:null,disposed:false}
            return {
                recorder:{get uri(){var scope=binding.scope;return scope&&scope.platform?scope.platform.recorder.uri:null},
                    prepareToRecordAsync:function(){return binding.scope.platform.recorder.prepareToRecordAsync()},
                    record:function(options){return binding.scope.platform.recorder.record(options)},
                    stop:function(){var scope=binding.scope;return scope&&scope.platform?scope.platform.recorder.stop():Promise.resolve()}},
                transcriber:{prepare:function(options){return prepare(options,binding)}},
                releaseRecording:function(){var scope=binding.scope;return scope&&scope.platform?scope.platform.cancelRecording():Promise.resolve()},
                deleteRecording:function(uri){var scope=recordings.get(uri);if(scope&&scope===binding.scope)scope.platform.deleteRecording(uri)},
                postProcess:function(transcript,options){
                    var scope=binding.scope
                    if(!scope||!scope.preferences)return null
                    scope.latestRef.current=latest(scope,scope.latestRef.current.clientSettings)
                    return scope.preferences.postProcess(transcript,options)
                },
                dispose:function(){binding.disposed=true;if(binding.scope)dispose(binding.scope)}
            }
        },
        ownerChanged:function(key, environment, epoch) {
            owner={key:key,environment:environment,epoch:epoch}
            send({action:"owner", ownerKey:key, draftEnvironment:environment, epoch:epoch})
        },
        receive:function(event) {
            if (!event || stopped) return
            if (event.kind === "ready") {
                available=event.available===true
                if(adapters.onAvailable)adapters.onAvailable(available)
                api.ownerChanged(owner.key,owner.environment,owner.epoch)
                if(available)snapshotRpc("snapshot").catch(function(failure){if(adapters.onCleanupError)adapters.onCleanupError(failure)})
                return
            }
            if(event.kind==="consent") {notificationEpoch++;updateSnapshot(event.value);return}
            if (event.kind === "result") {
                var request=pending.get(event.requestId)
                if(!request)return
                pending.delete(event.requestId)
                if(request.signal)request.signal.removeEventListener("abort",request.abort)
                if(event.error)request.reject(error(event.error));else request.resolve(event.value || {})
                collect(request.scope)
                return
            }
            var scope=scopes.get(event.session)
            if(!scope || !live(scope))return
            if(event.kind==="permission")adapters.onPermission(scope.id,event.permissionId)
            else if(event.kind==="microphones"&&adapters.onMicrophones)adapters.onMicrophones(event.devices)
            else if(event.kind==="level")scope.captureInput.onLevel(event.value)
            else if(event.kind==="duration-limit")scope.captureInput.onDurationLimit()
            else if(event.kind==="capture-error")scope.captureInput.onError(error(event.error).message)
            else if(event.kind==="stream" && scope.stream) {
                var value=event.value,stream=scope.stream
                if(value.event==="ready")stream.resolveReady()
                else if(value.event==="text" && value.text)stream.onText(value.text)
                else if(value.event==="error") { var failure=error(value.error);stream.rejectReady(failure);stream.onError(failure) }
            }
        },
        permission:function(session, permissionId, allow) {
            var scope=scopes.get(session)
            if(!scope)return Promise.reject(error("voice_owner_changed"))
            return rpc("permission",{permissionId:permissionId,allow:allow===true && active(scope)},scope)
        },
        isVoicePatch:function(patch){return Object.keys(patch || {}).some(function(key){return ["voiceMicrophone","voiceShortcutMode","speechLanguage","speechTranslateToEnglish","speechCustomWords","speechSilenceFiltering","speechRemoveFillerWords","speechPostProcessingPrompt","speechCorrectionWord"].indexOf(key)>=0})},
        preferences:function(value){return rpc("decode-preferences",{valueJson:JSON.stringify(value)}).then(function(result){return JSON.parse(result.valueJson)})},
        updatePreferences:function(patch,revision){return rpc("update-preferences",{patch:patch,revision:revision})},
        snapshot:function() { return snapshotRpc("snapshot") },
        chooseAudio:function(destination) { return snapshotRpc("choose-audio",{destinationJson:JSON.stringify(destination)}) },
        chooseCleanup:function(enabled) { return snapshotRpc("choose-cleanup",{enabled:enabled}) },
        pendingCloud:function(destination) { return rpc("pending-cloud",{destinationJson:JSON.stringify(destination)}) },
        acknowledgeCloud:function(acknowledgement) { return snapshotRpc("acknowledge-cloud",{acknowledgement:acknowledgement}) },
        lost:function() {
            stopped=true;available=false
            if(adapters.onAvailable)adapters.onAvailable(false)
            pending.forEach(function(request) {
                if(request.signal)request.signal.removeEventListener("abort",request.abort)
                request.reject(error("voice_host_lost"))
            });pending.clear()
            scopes.forEach(function(scope){scope.disposed=true;if(scope.stream)scope.stream.rejectReady(error("voice_host_lost"))})
        }
    }
    return api
}
