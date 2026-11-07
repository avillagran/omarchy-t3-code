#include "pcmdsp.h"
#include "voicemath.h"
#include <QQmlEngine>
#include <QThread>
#include <QPointer>
#include <cmath>
#include <limits>

namespace {
// Bound a single callback independently of the original five-minute PCM limit.
// The device owner must split larger buffers instead of silently dropping audio.
constexpr qsizetype MaxInputBytes=1024*1024;
constexpr qsizetype MaxFrames=4096;
}
O3PcmDsp::O3PcmDsp(QObject *parent):QObject(parent) {}
O3PcmDsp::~O3PcmDsp()=default;
bool O3PcmDsp::isPrepared() const { return prepared; }
QString O3PcmDsp::lastError() const { return error; }
bool O3PcmDsp::fail(const QString &message) {
    if (error.isEmpty()) {
        error=message;
        prepared=false;
        recording=false;
        partial.clear();
        QMetaObject::invokeMethod(this,[this,message](){emit failed(message);},Qt::QueuedConnection);
    }
    return false;
}
bool O3PcmDsp::checked(const QJSValue &result) {
    if (result.isError()) return fail(QStringLiteral("Microphone processing failed."));
    return error.isEmpty();
}
bool O3PcmDsp::prepare(const QAudioFormat &deviceFormat,const QString &trustedResource,bool meterOnly) {
    Q_ASSERT(QThread::currentThread()==thread());
    if (engine || !error.isEmpty()) return fail(QStringLiteral("Microphone processing is already prepared."));
    if (!deviceFormat.isValid() || deviceFormat.bytesPerSample()<=0 || deviceFormat.bytesPerFrame()<=0)
        return fail(QStringLiteral("The microphone format is unsupported."));
    if (deviceFormat.sampleRate()<16000)
        return fail(QStringLiteral("Microphone sample rate must be at least 16 kHz."));
    format=deviceFormat;
    engine=std::make_unique<QJSEngine>();
    // This QObject is owned by its C++ caller, never by the script wrapper.
    QJSEngine::setObjectOwnership(this,QJSEngine::CppOwnership);
    engine->globalObject().setProperty("sink",engine->newQObject(this));
    auto *math=new O3VoiceMath(engine.get());
    engine->globalObject().setProperty("nativePcmMath",engine->newQObject(math));
    auto resource=trustedResource;
    if (resource.startsWith(".pragma library\n")) resource.remove(0,16);
    if (!checked(engine->evaluate("var nativePcm=(function(){"+resource+";return createDsp})();",QStringLiteral("pinned-native-pcm")))) return false;
    engine->globalObject().setProperty("deviceRate",format.sampleRate());
    engine->globalObject().setProperty("meterOnly",meterOnly);
    if (!checked(engine->evaluate(R"JS(
        var Processor;
        var port={postMessage:function(value){
            if(typeof value==='number')sink.publishLevel(value);
            else if(typeof value==='string')sink.publishEvent(value);
            else sink.publishPcm(value.buffer.slice(value.byteOffset,value.byteOffset+value.byteLength));
        }};
        function Base(){this.port=port}
        nativePcm(deviceRate,{
            AudioWorkletProcessor:Base,
            registerProcessor:function(name,value){if(name!=='t3-pcm-capture')throw Error('Unexpected processor');Processor=value},
            math:{PI:Math.PI,min:Math.min,max:Math.max,sqrt:Math.sqrt,
                sin:function(x){return nativePcmMath.sin(x)},
                cos:function(x){return nativePcmMath.cos(x)},
                exp:function(x){return nativePcmMath.exp(x)}}
        });
        var processor=new Processor({processorOptions:{meterOnly:meterOnly}});
    )JS",QStringLiteral("owned-pcm-platform")))) return false;
    processor=engine->globalObject().property("processor");
    port=engine->globalObject().property("port");
    prepared=true;
    return true;
}
bool O3PcmDsp::command(const QString &value) {
    if (!prepared) return false;
    auto message=engine->newObject();message.setProperty("data",value);
    return checked(port.property("onmessage").callWithInstance(port,{message}));
}
bool O3PcmDsp::start() {
    Q_ASSERT(QThread::currentThread()==thread());
    if (!prepared || finished) return false;
    if (!command(QStringLiteral("start"))) return false;
    recording=true;
    return true;
}
bool O3PcmDsp::processFrames(qsizetype frames) {
    const auto channels=format.channelCount();
    auto input=engine->newArray(1),channelList=engine->newArray(channels);
    auto floatArray=engine->globalObject().property("Float32Array");
    const auto sampleBytes=format.bytesPerSample();
    const auto frameBytes=format.bytesPerFrame();
    for (int channel=0;channel<channels;channel++) {
        auto values=floatArray.callAsConstructor({QJSValue(int(frames))});
        if (!checked(values)) return false;
        for (qsizetype frame=0;frame<frames;frame++) {
            const auto offset=frame*frameBytes+channel*sampleBytes;
            // Qt owns device-format normalization. The original worklet still
            // owns channel averaging, resampling, metering, chunks and limits.
            values.setProperty(quint32(frame),format.normalizedSampleValue(partial.constData()+offset));
        }
        channelList.setProperty(quint32(channel),values);
    }
    input.setProperty(0,channelList);
    return checked(processor.property("process").callWithInstance(processor,{input}));
}
bool O3PcmDsp::feed(const QByteArray &deviceBytes) {
    Q_ASSERT(QThread::currentThread()==thread());
    if (!prepared) return false;
    if (!recording || finished) return true;
    if (deviceBytes.size()>MaxInputBytes) return fail(QStringLiteral("Microphone input exceeded its buffer limit."));
    partial.append(deviceBytes);
    const auto frameBytes=format.bytesPerFrame();
    while (partial.size()>=frameBytes) {
        const auto frames=qMin(MaxFrames,partial.size()/frameBytes);
        if (!processFrames(frames)) return false;
        partial.remove(0,frames*frameBytes);
        if (finished) {partial.clear();break;}
    }
    return true;
}
bool O3PcmDsp::stop() {
    Q_ASSERT(QThread::currentThread()==thread());
    if (!prepared) return false;
    // No partial sample/frame is invented. Every complete delivered frame was
    // already fed; the unchanged processor performs its own final FIR flush.
    partial.clear();
    const bool result=command(QStringLiteral("stop"));
    recording=false;
    finished=true;
    return result;
}
void O3PcmDsp::publishPcm(const QByteArray &bytes) {
    if (bytes.isEmpty() || bytes.size()>8000*4 || bytes.size()%4!=0) {
        fail(QStringLiteral("Microphone processing returned invalid PCM."));return;
    }
    enqueue({Message::Pcm,bytes,0});
}
void O3PcmDsp::publishLevel(double value) {
    if (!std::isfinite(value) || value<0 || value>1) {
        fail(QStringLiteral("Microphone processing returned an invalid level."));return;
    }
    enqueue({Message::Level,{},value});
}
void O3PcmDsp::publishEvent(const QString &value) {
    if (value==QStringLiteral("stopped")) enqueue({Message::Stopped,{},0});
    else if (value==QStringLiteral("limit")) {finished=true;recording=false;enqueue({Message::Limit,{},0});}
    else fail(QStringLiteral("Microphone processing returned an invalid event."));
}
void O3PcmDsp::enqueue(Message message) {
    messages.push_back(std::move(message));
    if (deliveryQueued) return;
    deliveryQueued=true;
    // MessagePort delivers outside the processor's stack. A receiver cancelling
    // during a PCM callback must not re-enter a still-running JS process().
    QMetaObject::invokeMethod(this,[this]() {
        deliveryQueued=false;
        auto ready=std::move(messages);
        messages.clear();
        QPointer<O3PcmDsp> alive(this);
        for (const auto &message:ready) {
            switch (message.kind) {
            case Message::Pcm:emit pcm(message.bytes);break;
            case Message::Level:emit level(message.value);break;
            case Message::Stopped:emit stopped();break;
            case Message::Limit:emit durationLimit();break;
            }
            if (!alive) return;
        }
    },Qt::QueuedConnection);
}
