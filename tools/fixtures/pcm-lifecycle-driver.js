// Compare the complete capture producer across engines with identical controlled
// browser primitives. This exercises callback ordering, not a copied reducer.
function runCaptureLifecycle(factory) {
    function turns() {
        var chain=Promise.resolve();
        for(var index=0;index<12;index++)chain=chain.then(function(){});
        return chain;
    }
    function deferred(){var resolve,reject;var promise=new Promise(function(a,b){resolve=a;reject=b});return {promise:promise,resolve:resolve,reject:reject}}
    function outcome(promise){return Promise.resolve(promise).then(function(){return 'ok'},function(error){return String(error&&error.message||error)})}
    function signal(){
        var listeners=[];
        return {aborted:false,reason:null,addEventListener:function(name,callback){if(name!=='abort')throw Error('Unexpected signal event');listeners.push(callback)},removeEventListener:function(name,callback){listeners=listeners.filter(function(value){return value!==callback})},throwIfAborted:function(){if(this.aborted)throw this.reason},abort:function(){if(this.aborted)return;this.aborted=true;this.reason=Error('cancelled');listeners.slice().forEach(function(callback){callback()});listeners=[]}};
    }
    var rows=[],sequence=Promise.resolve();
    function test(name,body){sequence=sequence.then(function(){var env=environment();return Promise.resolve(body(env)).then(function(value){rows.push({name:name,result:value,events:env.events,liveTimers:env.timers.map(function(timer){return timer.ms}),blobs:env.blobs})})})}
    function environment(){
        var env={events:[],timers:[],blobs:[],permission:deferred(),module:deferred(),resume:deferred(),prepareError:null,streaming:false,device:'',nextId:0,ack:true,stopError:false,stopChunks:[],trackEnded:null};
        var events=env.events;
        env.signal=signal();
        env.track={stop:function(){events.push(['track.stop'])},addEventListener:function(name,callback,options){events.push(['track.listen',name,options.once]);env.trackEnded=callback}};
        env.stream={getTracks:function(){return [env.track]}};
        function Context(){
            events.push(['context.new']);env.context=this;this.destination={};
            this.audioWorklet={addModule:function(url){events.push(['module',url]);return env.module.promise}};
            this.createMediaStreamSource=function(stream){if(stream!==env.stream)throw Error('Wrong stream');events.push(['source.new']);return {connect:function(node){if(node!==env.node)throw Error('Wrong node');events.push(['source.connect'])}}};
            this.resume=function(){events.push(['context.resume']);return env.resume.promise};
            this.close=function(){events.push(['context.close']);return Promise.resolve()};
        }
        function Node(context,name,options){
            if(context!==env.context)throw Error('Wrong context');events.push(['node.new',name,options.channelCount]);env.node=this;
            this.connect=function(destination){if(destination!==context.destination)throw Error('Wrong destination');events.push(['node.connect'])};
            this.disconnect=function(){events.push(['node.disconnect'])};
            this.port={close:function(){events.push(['port.close'])},postMessage:function(message){events.push(['port.post',message]);if(message==='stop')Promise.resolve().then(function(){env.stopChunks.forEach(env.message);if(env.stopError)env.node.onprocessorerror();else if(env.ack)env.message('stopped')})}};
        }
        function Blob(parts,options){this.parts=parts.slice();this.type=options.type}
        var platform={navigator:{mediaDevices:{getUserMedia:function(options){events.push(['permission',options]);return env.permission.promise}}},AudioContext:Context,AudioWorkletNode:Node,Blob:Blob,URL:{createObjectURL:function(blob){var id='blob:owned-'+(++env.nextId),bytes=[];blob.parts.forEach(function(part){var view=new Uint8Array(part.buffer,part.byteOffset,part.byteLength);for(var i=0;i<view.length;i++)bytes.push(view[i])});env.blobs.push({id:id,type:blob.type,bytes:bytes});events.push(['url.create',id]);return id},revokeObjectURL:function(uri){events.push(['url.revoke',uri])}},setTimeout:function(callback,ms){var timer={id:++env.nextId,callback:callback,ms:ms};env.timers.push(timer);events.push(['timer.set',ms]);return timer.id},clearTimeout:function(id){var found=env.timers.filter(function(timer){return timer.id===id});events.push(['timer.clear',found.length?found[0].ms:null]);env.timers=env.timers.filter(function(timer){return timer.id!==id})},workletUrl:'owned:whole-native-worklet'};
        env.capture=factory(platform).createBrowserPcmCapture({getMicrophoneId:function(){events.push(['device']);return env.device},onLevel:function(value){events.push(['level',value])},onDurationLimit:function(){events.push(['limit'])},onError:function(value){events.push(['error',value])}},function(options){events.push(['transcription.prepare',options.signal===env.signal]);if(env.prepareError)return Promise.reject(Error(env.prepareError));return Promise.resolve(env.streaming?{streaming:{feed:function(pcm){events.push(['feed',Array.prototype.slice.call(pcm)])}}}:{})});
        env.message=function(value){env.node.port.onmessage({data:value})};
        env.prepare=function(){return env.capture.transcriber.prepare({signal:env.signal})};
        env.ready=function(){env.permission.resolve(env.stream);env.module.resolve();env.resume.resolve();return env.prepare().then(function(){return env.capture.recorder.prepareToRecordAsync()})};
        env.record=function(){env.capture.recorder.record({forDuration:300})};
        env.fire=function(ms){var found=env.timers.filter(function(timer){return timer.ms===ms});if(found.length!==1)throw Error('Expected one timer '+ms);env.timers=env.timers.filter(function(timer){return timer!==found[0]});found[0].callback()};
        return env;
    }
    test('not prepared',function(e){return outcome(e.capture.recorder.prepareToRecordAsync())});
    test('transcription preparation refuses',function(e){e.prepareError='destination refused';return outcome(e.prepare())});
    test('aborted transcription preparation',function(e){e.signal.abort();return outcome(e.prepare())});
    test('permission refuses',function(e){e.permission.reject(Error('permission refused'));return e.prepare().then(function(){return outcome(e.capture.recorder.prepareToRecordAsync())})});
    test('late permission after abort stops track',function(e){return e.prepare().then(function(){var pending=outcome(e.capture.recorder.prepareToRecordAsync());e.signal.abort();return pending}).then(function(value){e.permission.resolve(e.stream);return turns().then(function(){return value})})});
    test('abort before acquisition',function(e){return e.prepare().then(function(){e.signal.abort();return outcome(e.capture.recorder.prepareToRecordAsync())})});
    test('default microphone and clean stop',function(e){return e.ready().then(function(){e.record();return e.capture.recorder.stop()}).then(function(){return e.capture.recorder.uri})});
    test('exact selected microphone',function(e){e.device='test-device';return e.ready().then(function(){e.capture.cancelRecording();return e.capture.recorder.uri})});
    test('module load refuses',function(e){e.permission.resolve(e.stream);e.module.reject(Error('module failed'));return e.prepare().then(function(){return outcome(e.capture.recorder.prepareToRecordAsync())})});
    test('abort during module load',function(e){e.permission.resolve(e.stream);return e.prepare().then(function(){var pending=outcome(e.capture.recorder.prepareToRecordAsync());return turns().then(function(){if(!e.context||e.node)throw Error('Expected suspended module admission');e.signal.abort();e.module.resolve();return pending})})});
    test('resume refuses',function(e){e.permission.resolve(e.stream);e.module.resolve();e.resume.reject(Error('resume failed'));return e.prepare().then(function(){return outcome(e.capture.recorder.prepareToRecordAsync())})});
    test('abort during resume',function(e){e.permission.resolve(e.stream);e.module.resolve();return e.prepare().then(function(){var pending=outcome(e.capture.recorder.prepareToRecordAsync());return turns().then(function(){if(!e.events.some(function(event){return event[0]==='context.resume'}))throw Error('Expected suspended resume');e.signal.abort();e.resume.resolve();return pending})})});
    test('buffered packets and flush become exact blob',function(e){return e.ready().then(function(){e.record();e.message(0.25);e.message(new Float32Array([0.5,-0.25]));e.stopChunks=[new Float32Array([1/8])];return e.capture.recorder.stop()}).then(function(){return e.capture.recorder.uri})});
    test('streaming packets bypass blob',function(e){e.streaming=true;return e.ready().then(function(){e.record();e.message(new Float32Array([0.5]));e.stopChunks=[new Float32Array([-0.25])];return e.capture.recorder.stop()}).then(function(){return e.capture.recorder.uri})});
    test('stop timeout releases capture',function(e){e.ack=false;return e.ready().then(function(){e.record();var pending=outcome(e.capture.recorder.stop());e.fire(5000);return pending})});
    test('processor failure rejects stop',function(e){e.stopError=true;return e.ready().then(function(){e.record();return outcome(e.capture.recorder.stop())})});
    test('track disconnect reports error',function(e){return e.ready().then(function(){e.trackEnded();e.capture.cancelRecording();return 'ok'})});
    test('duration timer reports limit',function(e){return e.ready().then(function(){e.record();e.fire(300000);e.capture.cancelRecording();return 'ok'})});
    test('worklet reports limit',function(e){return e.ready().then(function(){e.record();e.message('limit');e.capture.cancelRecording();return 'ok'})});
    test('aborted worklet messages are ignored',function(e){return e.ready().then(function(){e.record();e.signal.abort();e.message(new Float32Array([1]));e.message(1);e.message('limit');return e.capture.recorder.stop()}).then(function(){return e.capture.recorder.uri})});
    test('cancel resolves pending stop',function(e){e.ack=false;return e.ready().then(function(){e.record();e.message(new Float32Array([1]));var pending=outcome(e.capture.recorder.stop());e.capture.cancelRecording();return pending})});
    test('repeated stop is inert after cleanup',function(e){return e.ready().then(function(){e.record();return e.capture.recorder.stop()}).then(function(){return e.capture.recorder.stop()}).then(function(){return e.capture.recorder.uri})});
    test('delete clears matching recording only',function(e){return e.ready().then(function(){e.record();e.message(new Float32Array([1]));return e.capture.recorder.stop()}).then(function(){var uri=e.capture.recorder.uri;e.capture.deleteRecording('blob:foreign');var kept=e.capture.recorder.uri===uri;e.capture.deleteRecording(uri);return {kept:kept,uri:e.capture.recorder.uri}})});
    test('prepare revokes previous recording',function(e){return e.ready().then(function(){e.record();e.message(new Float32Array([1]));return e.capture.recorder.stop()}).then(function(){return e.prepare()}).then(function(){return e.capture.recorder.prepareToRecordAsync()}).then(function(){var uri=e.capture.recorder.uri;e.capture.cancelRecording();return uri})});
    test('cancel is repeatable without duplicate track stop',function(e){return e.ready().then(function(){e.record();e.capture.cancelRecording();e.capture.cancelRecording();return e.capture.recorder.uri})});
    return sequence.then(function(){return {wholeCaptureCases:rows.length,cases:rows}});
}
