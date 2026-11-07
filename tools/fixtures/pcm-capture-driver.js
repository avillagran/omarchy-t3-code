// Whole native DSP in both engines. Inputs are exact binary fractions/integers;
// input generation does not introduce a second trigonometric implementation.
function runPcmOracle(factory,digest) {
    var cases=[],rows=[],sequence=Promise.resolve(),mathDigest=null;
    [16000,22050,24000,32000,44100,48000,88200,96000,192000].forEach(function(rate){
        ['silence','impulse','alternating','ramp','random','stereo-cancel','stereo-mixed'].forEach(function(pattern){cases.push({rate:rate,pattern:pattern,frames:8197,blocks:[1,127,128,241,511,37],meterOnly:false})});
        cases.push({rate:rate,pattern:'random',frames:13117,blocks:[128],meterOnly:true});
    });
    cases.push({rate:16000,pattern:'ramp',frames:4800000+257,blocks:[8192],meterOnly:false});
    function append(key,array){digest.append(key,array.buffer.slice(array.byteOffset,array.byteOffset+array.byteLength))}
    cases.forEach(function(input,index){sequence=sequence.then(function(){
        var name='case:'+index,events=[],packets=0,samples=0,levels=0,state=0x13579bdf;
        var port={postMessage:function(value){
            if(typeof value==='string'){events.push(value);return}
            if(typeof value==='number'){append(name+':levels',new Float64Array([value]));levels++;return}
            append(name+':pcm',value);packets++;samples+=value.length;
        }};
        return Promise.resolve(factory(input.rate,port)).then(function(api){
            if(index===0){
                for(var term=-4096;term<=4096;term++){
                    var argument=term/32;
                    append('math-primitives',new Float64Array([api.math.sin(argument),api.math.cos(argument),api.math.exp(argument)]));
                }
                append('math-primitives',new Float64Array([api.math.sin(-0),api.math.cos(-0),api.math.exp(-0)]));
                mathDigest=digest.finish('math-primitives');
            }
            var resampler=new api.PcmResampler(input.rate);
            append(name+':coefficients',resampler.coefficients);
            var processor=new api.Processor({processorOptions:{meterOnly:input.meterOnly}});
            processor.process([[new Float32Array([1])]]);
            if(packets||levels||events.length)throw Error('Unstarted processor emitted data');
            port.onmessage({data:'start'});
            var offset=0,blockIndex=0;
            while(offset<input.frames){
                var length=Math.min(input.blocks[blockIndex++%input.blocks.length],input.frames-offset),channels=[new Float32Array(length)];
                if(input.pattern.indexOf('stereo')===0)channels.push(new Float32Array(length));
                for(var frame=0;frame<length;frame++){
                    var at=offset+frame,value=0;
                    if(input.pattern==='impulse')value=at===0?1:0;
                    else if(input.pattern==='alternating')value=at%2?1:-1;
                    else if(input.pattern==='ramp')value=((at%513)-256)/256;
                    else if(input.pattern!=='silence'){state=(Math.imul(state,1664525)+1013904223)|0;value=((state>>>16)-32768)/32768}
                    channels[0][frame]=value;
                    if(channels.length===2)channels[1][frame]=input.pattern==='stereo-cancel'?-value:value/2;
                }
                processor.process([channels]);offset+=length;
            }
            port.onmessage({data:'stop'});var before={packets:packets,samples:samples,levels:levels,events:events.length};
            port.onmessage({data:'stop'});processor.process([[new Float32Array([1,1])]]);
            // The original processor acknowledges each stop, but flushes PCM once.
            if(packets!==before.packets||samples!==before.samples||levels!==before.levels)throw Error('Stopped processor emitted more audio');
            rows.push({input:input,coefficients:digest.finish(name+':coefficients'),pcm:digest.finish(name+':pcm'),levels:digest.finish(name+':levels'),packets:packets,samples:samples,levelEvents:levels,events:events});
        });
    })});
    return sequence.then(function(){return {mathDigest:mathDigest,mathPrimitiveObservations:8194*3,cases:rows,wholeModuleCases:cases.length,fullLimitSamples:rows[rows.length-1].samples}});
}
