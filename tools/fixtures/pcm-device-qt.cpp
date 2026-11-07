// Exercise the actual C++ device-byte consumer against the unchanged shared DSP
// oracle. The private adapter only supplies synthetic Float32 device buffers.
#include "../../native/voice/pcmdsp.h"
#include "../../native/voice/voicemath.h"
#include <QCoreApplication>
#include <QCryptographicHash>
#include <QEvent>
#include <QFile>
#include <QHash>
#include <QSharedPointer>
#include <QTimer>
#include <cstdio>
#include <cstring>

static QString read(const QString &path) {
    QFile file(path);if(!file.open(QIODevice::ReadOnly))qFatal("Missing PCM device fixture");
    auto source=QString::fromUtf8(file.readAll());
    if(source.startsWith(".pragma library\n"))source.remove(0,16);
    return source;
}
class Digests final:public QObject {
    Q_OBJECT
    QHash<QString,QSharedPointer<QCryptographicHash>> hashes;
    QHash<QString,qint64> lengths;
public:
    explicit Digests(QObject *parent):QObject(parent) {}
    Q_INVOKABLE void append(const QString &key,const QByteArray &bytes) {
        if(!hashes.contains(key))hashes.insert(key,QSharedPointer<QCryptographicHash>::create(QCryptographicHash::Sha256));
        hashes[key]->addData(bytes);lengths[key]+=bytes.size();
    }
    Q_INVOKABLE QString finish(const QString &key) {
        if(!hashes.contains(key))append(key,QByteArray());
        return QString::number(lengths[key])+":"+QString::fromLatin1(hashes[key]->result().toHex());
    }
};
class Device final:public QObject {
    Q_OBJECT
    QJSEngine &engine;
    QString resource;
    std::unique_ptr<O3PcmDsp> dsp;
    QJSValue port;
    int rate=0,channels=0;
    bool meter=false,started=false;
    void ensure(bool success) {if(!success)qFatal("Actual device DSP failed");}
    void drain() {if(dsp)QCoreApplication::sendPostedEvents(dsp.get(),QEvent::MetaCall);}
    void post(const QJSValue &value) {
        const auto result=port.property("postMessage").callWithInstance(port,{value});
        if(result.isError())qFatal("Fixture port rejected a device message");
    }
    void prepare(int count) {
        channels=count;
        QAudioFormat format;format.setSampleRate(rate);format.setChannelCount(channels);format.setSampleFormat(QAudioFormat::Float);
        dsp=std::make_unique<O3PcmDsp>();
        connect(dsp.get(),&O3PcmDsp::failed,this,[](const QString &){qFatal("Actual device DSP emitted failure");});
        connect(dsp.get(),&O3PcmDsp::pcm,this,[this](const QByteArray &bytes){
            const auto value=engine.globalObject().property("Float32Array").callAsConstructor({engine.toScriptValue(bytes)});
            if(value.isError())qFatal("Device output ArrayBuffer conversion failed");
            post(value);
        });
        connect(dsp.get(),&O3PcmDsp::level,this,[this](double value){post(QJSValue(value));});
        connect(dsp.get(),&O3PcmDsp::stopped,this,[this](){post(QJSValue("stopped"));});
        connect(dsp.get(),&O3PcmDsp::durationLimit,this,[this](){post(QJSValue("limit"));});
        ensure(dsp->prepare(format,resource,meter));ensure(dsp->start());
    }
public:
    Device(QJSEngine &value,QString source):engine(value),resource(std::move(source)) {}
    Q_INVOKABLE void begin(int sampleRate,const QJSValue &messagePort,bool meterOnly) {
        dsp.reset();port=messagePort;rate=sampleRate;channels=0;meter=meterOnly;started=false;
    }
    Q_INVOKABLE void command(const QString &value) {
        if(value=="start")started=true;
        else if(value=="stop") {
            if(!dsp)prepare(1);
            ensure(dsp->stop());drain();started=false;
        } else qFatal("Unexpected fixture worklet command");
    }
    Q_INVOKABLE void feed(const QJSValue &input) {
        if(!started)return;
        const auto values=input.property(0);
        const auto count=values.property("length").toInt();
        if(count<1)return;
        const auto frames=values.property(0).property("length").toInt();
        if(!dsp)prepare(count);
        if(count!=channels)qFatal("Synthetic device format changed within a stream");
        QByteArray bytes(frames*channels*int(sizeof(float)),Qt::Uninitialized);
        for(int frame=0;frame<frames;frame++)for(int channel=0;channel<channels;channel++) {
            const float value=float(values.property(channel).property(frame).toNumber());
            std::memcpy(bytes.data()+(frame*channels+channel)*sizeof(float),&value,sizeof(value));
        }
        // A QIODevice write may split a frame at any byte. These two calls
        // exercise the production carry without adding or losing a sample.
        const auto first=qMin(qsizetype(3),bytes.size());
        ensure(dsp->feed(bytes.first(first)));
        if(bytes.size()>first)ensure(dsp->feed(bytes.sliced(first)));
        drain();
    }
};
static void inputAndDeliveryChecks(const QString &resource) {
    for(int problem=0;problem<5;problem++) {
        QAudioFormat format;format.setSampleRate(16000);format.setChannelCount(1);format.setSampleFormat(QAudioFormat::Float);
        if(problem==0)format.setSampleRate(8000);
        if(problem==1)format.setSampleRate(0);
        if(problem==2)format.setChannelCount(0);
        if(problem==3)format.setSampleFormat(QAudioFormat::Unknown);
        if(problem==4)format.setSampleFormat(QAudioFormat::NSampleFormats);
        O3PcmDsp invalid;
        if(invalid.prepare(format,resource)||invalid.isPrepared()||invalid.lastError().isEmpty())qFatal("Invalid device format was admitted");
    }
    QAudioFormat format;format.setSampleRate(16000);format.setChannelCount(1);format.setSampleFormat(QAudioFormat::Float);
    O3PcmDsp actual;
    bool feeding=false,ended=false,gotLevel=false;
    QObject::connect(&actual,&O3PcmDsp::level,&actual,[&](double){
        if(feeding)qFatal("Device callback re-entered a running process stack");
        gotLevel=true;if(!actual.stop())qFatal("Callback cancellation failed");
    });
    QObject::connect(&actual,&O3PcmDsp::stopped,&actual,[&](){ended=true;});
    if(!actual.prepare(format,resource)||!actual.start())qFatal("Delivery fixture preparation failed");
    feeding=true;
    if(!actual.feed(QByteArray(800*4,char(0))))qFatal("Delivery fixture feed failed");
    feeding=false;
    if(gotLevel||ended)qFatal("Device messages were delivered synchronously");
    QCoreApplication::sendPostedEvents(&actual,QEvent::MetaCall);
    QCoreApplication::sendPostedEvents(&actual,QEvent::MetaCall);
    if(!gotLevel||!ended)qFatal("Callback stop was not acknowledged");
}
int main(int argc,char **argv) {
    QCoreApplication app(argc,argv);if(argc!=3)return 2;
    QJSEngine engine;
    auto checked=[&](const QString &source,const QString &name) {
        auto result=engine.evaluate(source,name);
        if(result.isError()) {fprintf(stderr,"%s:%d: %s\n",qPrintable(name),result.property("lineNumber").toInt(),qPrintable(result.toString()));exit(3);}
        return result;
    };
    const auto resource=read(argv[1]);
    inputAndDeliveryChecks(resource);
    Device device(engine,resource);
    QJSEngine::setObjectOwnership(&device,QJSEngine::CppOwnership);
    engine.globalObject().setProperty("device",engine.newQObject(&device));
    engine.globalObject().setProperty("pcmDigest",engine.newQObject(new Digests(&engine)));
    engine.globalObject().setProperty("nativePcmMath",engine.newQObject(new O3VoiceMath(&engine)));
    checked("var pcm=(function(){"+resource+";return {createDsp:createDsp}})();",argv[1]);
    checked(read(argv[2]),argv[2]);
    checked(R"JS(var completed=false,failure=null,result=null;
      runPcmOracle(function(rate,port){
        var math={PI:Math.PI,min:Math.min,max:Math.max,sqrt:Math.sqrt,sin:function(x){return nativePcmMath.sin(x)},cos:function(x){return nativePcmMath.cos(x)},exp:function(x){return nativePcmMath.exp(x)}};
        function Base(){this.port=port}
        var api=pcm.createDsp(rate,{math:math,AudioWorkletProcessor:Base,registerProcessor:function(){}});
        function Processor(options){device.begin(rate,port,options.processorOptions.meterOnly);port.onmessage=function(event){device.command(event.data)}}
        Processor.prototype.process=function(inputs){device.feed(inputs);return true};
        return {PcmResampler:api.PcmResampler,Processor:Processor,math:math};
      },pcmDigest).then(function(value){result=value;completed=true},function(error){failure=String(error)+'\n'+String(error&&error.stack||'');completed=true});)JS","run-device-dsp");
    QTimer poll;QObject::connect(&poll,&QTimer::timeout,&app,[&](){
        if(!engine.globalObject().property("completed").toBool())return;
        if(!engine.globalObject().property("failure").isNull()){fprintf(stderr,"%s\n",qPrintable(engine.globalObject().property("failure").toString()));app.exit(4);return;}
        const auto bytes=checked("JSON.stringify(result)","serialize").toString().toUtf8();fwrite(bytes.data(),1,bytes.size(),stdout);fputc('\n',stdout);app.quit();
    });poll.start(0);
    QTimer deadline;deadline.setSingleShot(true);QObject::connect(&deadline,&QTimer::timeout,&app,[&](){fprintf(stderr,"Device DSP deadline\n");app.exit(5);});deadline.start(90000);
    return app.exec();
}
#include "pcm-device-qt.moc"
