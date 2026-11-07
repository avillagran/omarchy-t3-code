// Raw device samples and expected float bits come from the complete pinned
// Chromium traits compiled with Clang, then run through the whole native DSP.
function runPcmIntegerOracle(factory,digest,buffer) {
    var count=131073,view=new DataView(buffer),rows=[],sequence=Promise.resolve();
    if(view.byteLength!==8+3*count*8)throw Error('Invalid native codec corpus length');
    var magic='';for(var at=0;at<8;at++)magic+=String.fromCharCode(view.getUint8(at));
    if(magic!=='O3CODEC1')throw Error('Invalid native codec corpus');
    function append(key,array){digest.append(key,array.buffer.slice(array.byteOffset,array.byteOffset+array.byteLength))}
    [1,2,3].forEach(function(kind){[16000,44100,48000].forEach(function(rate){[1,2].forEach(function(channels){
        sequence=sequence.then(function(){
            var key='integer:'+kind+':'+rate+':'+channels,events=[],samples=0,packets=0,levels=0;
            var port={postMessage:function(value){
                if(typeof value==='string'){events.push(value);return}
                if(typeof value==='number'){append(key+':levels',new Float64Array([value]));levels++;return}
                append(key+':pcm',value);samples+=value.length;packets++;
            }};
            return Promise.resolve(factory(kind,rate,channels,port)).then(function(device){
                device.start();
                var offset=0,block=0,sizes=[1,127,513,4096,3,241],width=kind===1?1:kind===2?2:4;
                while(offset<count){
                    var frames=Math.min(count-offset,sizes[block++%sizes.length]),raw=new ArrayBuffer(frames*channels*width),data=new DataView(raw),normalized=[];
                    for(var channel=0;channel<channels;channel++)normalized.push(new Float32Array(frames));
                    for(var frame=0;frame<frames;frame++)for(var channel=0;channel<channels;channel++){
                        var index=(offset+frame+channel*54321)%count,entry=8+((kind-1)*count+index)*8,bits=view.getUint32(entry,true),destination=(frame*channels+channel)*width;
                        if(width===1)data.setUint8(destination,bits);
                        else if(width===2)data.setUint16(destination,bits,true);
                        else data.setUint32(destination,bits,true);
                        normalized[channel][frame]=view.getFloat32(entry+4,true);
                    }
                    device.feed(raw,normalized);offset+=frames;
                }
                device.stop();var before=samples;device.stop();if(samples!==before)throw Error('Repeated integer stop emitted PCM');
                rows.push({format:kind,rate:rate,channels:channels,inputFrames:count,pcm:digest.finish(key+':pcm'),levels:digest.finish(key+':levels'),samples:samples,packets:packets,levelEvents:levels,events:events});
            });
        });
    })})});
    return sequence.then(function(){return {integerDeviceCases:rows.length,codecCorpusSamples:3*count,cases:rows}});
}
