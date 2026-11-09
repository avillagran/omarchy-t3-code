#include <QCoreApplication>
#include <QCryptographicHash>
#include <QFile>
#include <QHash>
#include <QJSEngine>
#include <QSharedPointer>
#include <QTimer>
#include <cstdio>
#include "../../native/voice/voicemath.h"

class Digests : public QObject {
    Q_OBJECT
    QHash<QString,QSharedPointer<QCryptographicHash>> hashes;
    QHash<QString,qint64> lengths;
public:
    explicit Digests(QObject *parent) : QObject(parent) {}
    Q_INVOKABLE void append(const QString &key, const QByteArray &bytes) {
        if (!hashes.contains(key)) hashes.insert(key,QSharedPointer<QCryptographicHash>::create(QCryptographicHash::Sha256));
        hashes[key]->addData(bytes); lengths[key]+=bytes.size();
    }
    Q_INVOKABLE QString finish(const QString &key) {
        if (!hashes.contains(key)) append(key,QByteArray());
        return QString::number(lengths[key])+":"+QString::fromLatin1(hashes[key]->result().toHex());
    }
};
static QString read(const QString &path) {
    QFile file(path);if(!file.open(QIODevice::ReadOnly))qFatal("Missing PCM fixture");
    QString source=QString::fromUtf8(file.readAll());if(source.startsWith(".pragma library\n"))source.remove(0,16);return source;
}
int main(int argc,char **argv) {
    QCoreApplication app(argc,argv);if(argc!=3)return 2;
    QJSEngine engine;
    auto checked=[&](const QString &source,const QString &name){auto value=engine.evaluate(source,name);if(value.isError()){fprintf(stderr,"%s:%d: %s\n",qPrintable(name),value.property("lineNumber").toInt(),qPrintable(value.toString()));exit(3);}return value;};
    engine.globalObject().setProperty("pcmDigest",engine.newQObject(new Digests(&engine)));
    engine.globalObject().setProperty("nativePcmMath",engine.newQObject(new O3VoiceMath(&engine)));
    checked("var pcm=(function(){"+read(argv[1])+";return {createDsp:createDsp,createCapture:createCapture}})();",argv[1]);
    checked(read(argv[2]),argv[2]);
    checked(R"JS(var completed=false,failure=null,result=null;
      runPcmOracle(function(rate,port){var Processor;
        function Base(){this.port=port}
        var math={PI:Math.PI,min:Math.min,max:Math.max,sqrt:Math.sqrt,sin:function(x){return nativePcmMath.sin(x)},cos:function(x){return nativePcmMath.cos(x)},exp:function(x){return nativePcmMath.exp(x)}};
        var api=pcm.createDsp(rate,{math:math,AudioWorkletProcessor:Base,registerProcessor:function(name,value){if(name!=='t3-pcm-capture')throw Error('Unexpected processor');Processor=value}});
        return {PcmResampler:api.PcmResampler,Processor:Processor,math:math};
      },pcmDigest).then(function(value){result=value;completed=true},function(error){failure=String(error)+'\n'+String(error&&error.stack||'');completed=true});)JS","run-pcm");
    QTimer poll;QObject::connect(&poll,&QTimer::timeout,&app,[&](){if(!engine.globalObject().property("completed").toBool())return;if(!engine.globalObject().property("failure").isNull()){fprintf(stderr,"%s\n",qPrintable(engine.globalObject().property("failure").toString()));app.exit(4);return;}auto output=checked("JSON.stringify(result)","serialize").toString().toUtf8();fwrite(output.data(),1,output.size(),stdout);fputc('\n',stdout);app.quit();});poll.start(0);
    QTimer deadline;deadline.setSingleShot(true);QObject::connect(&deadline,&QTimer::timeout,&app,[&](){fprintf(stderr,"PCM deadline\n");app.exit(5);});deadline.start(90000);
    return app.exec();
}
#include "pcm-capture-qt.moc"
