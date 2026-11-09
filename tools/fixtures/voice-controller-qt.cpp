#include <QCoreApplication>
#include <QCryptographicHash>
#include <QFile>
#include <QJSEngine>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTimer>
#include <cstdio>

// This harness runs without a GUI, microphone, network, or filesystem executor.
static QString read(const QString &name) {
    QFile input(name);
    if (!input.open(QIODevice::ReadOnly)) qFatal("Cannot read reviewed voice fixture");
    QString source = QString::fromUtf8(input.readAll());
    if (source.startsWith(".pragma library\n")) source.remove(0, 16);
    return source;
}

int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);
    if (argc != 4) return 2;
    QJSEngine engine;
    const auto checked = [&](const QString &source, const QString &name) {
        auto result = engine.evaluate(source, name);
        if (result.isError()) {
            fprintf(stderr, "%s:%d: %s\n", qPrintable(name), result.property("lineNumber").toInt(), qPrintable(result.toString()));
            exit(3);
        }
        return result;
    };
    checked("var voicePlatformFactory=(function(){" + read(argv[1]) + ";return create;})();", argv[1]);
    checked("var voiceNativeFactory=(function(){" + read(argv[2]) + ";return create;})();", argv[2]);
    auto unicode = checked(R"JS((function(){
        var api=voiceNativeFactory(voicePlatformFactory()),output=[],chunk=[];
        var predicates=['left','right','first','last'].map(function(kind){return api.propertyPredicate(kind)});
        for(var point=0;point<0x110000;point++){
            var value=String.fromCodePoint(point),mask=0;
            for(var index=0;index<4;index++)if(predicates[index].test(value))mask|=1<<index;
            chunk.push(String.fromCharCode(mask));
            if(chunk.length===4096){output.push(chunk.join(''));chunk=[]}
        }
        output.push(chunk.join(''));return output.join('');
    })())JS", "all-unicode-properties").toString().toLatin1();
    const auto digest = QCryptographicHash::hash(unicode, QCryptographicHash::Sha256).toHex();
    checked(read(argv[3]), argv[3]);
    checked("var __voiceDone=false,__voiceFailure=null,__voiceResult=null;runVoiceOracle(function(){return voiceNativeFactory(voicePlatformFactory())}).then(function(value){__voiceResult=value;__voiceDone=true},function(error){__voiceFailure=String(error&&error.stack||error);__voiceDone=true});", "run-oracle");
    QTimer deadline;
    deadline.setSingleShot(true);
    QObject::connect(&deadline, &QTimer::timeout, &app, [&]() { fprintf(stderr,"Voice oracle deadline\n"); app.exit(4); });
    QTimer poll;
    QObject::connect(&poll, &QTimer::timeout, &app, [&]() {
        if (!engine.globalObject().property("__voiceDone").toBool()) return;
        auto failure=engine.globalObject().property("__voiceFailure");
        if (!failure.isNull()) { fprintf(stderr,"%s\n",qPrintable(failure.toString())); app.exit(5); return; }
        auto value=checked("JSON.stringify(__voiceResult)", "serialize-oracle").toString().toUtf8();
        auto result=QJsonDocument::fromJson(value).object();
        result.insert("unicodeSha256",QString::fromLatin1(digest));
        result.insert("unicodeObservations",unicode.size());
        auto output=QJsonDocument(result).toJson(QJsonDocument::Compact);
        fwrite(output.data(),1,output.size(),stdout); fputc('\n',stdout); app.quit();
    });
    deadline.start(90000); poll.start(0);
    return app.exec();
}
