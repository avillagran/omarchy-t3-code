#include <QCoreApplication>
#include <QFile>
#include <QJSEngine>
#include <QTimer>
#include <cstdio>

static QString read(const char *name) {
    QFile input(QString::fromLocal8Bit(name));
    if (!input.open(QIODevice::ReadOnly)) qFatal("Cannot read platform fixture");
    QString text = QString::fromUtf8(input.readAll());
    if (text.startsWith(".pragma library\n")) text.remove(0, 16);
    return text;
}
int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);
    if (argc != 5) return 2;
    QJSEngine engine;
    const auto evaluate = [&](const QString &text, const QString &name) {
        const auto value = engine.evaluate(text, name);
        if (value.isError()) {
            fprintf(stderr, "%s:%d %s\n", qPrintable(name), value.property("lineNumber").toInt(), qPrintable(value.toString()));
            exit(3);
        }
    };
    const QString names[] = {"primitives", "voice", "platform"};
    for (int index = 0; index < 3; ++index)
        evaluate("var " + names[index] + "Factory=(function(){" + read(argv[index + 1]) + ";return create;})();", names[index]);
    evaluate(read(argv[4]), "driver");
    evaluate(R"JS(
var done=false,failure=null,result=null;
runVoicePlatformOracle(function(deps){
    var primitives=primitivesFactory(),voice=voiceFactory(primitives);
    deps.Schema=primitives.Schema;deps.VoiceTranscriptionError=voice.VoiceTranscriptionError;
    deps.throwIfVoiceTranscriptionAborted=voice.throwIfVoiceTranscriptionAborted;
    var platform=platformFactory(deps);
    platform.VoiceTranscriptionError=voice.VoiceTranscriptionError;
    platform.errorPredicate=voice.errorPredicate;platform.AbortController=primitives.AbortController;
    return platform;
}).then(function(value){result=value;done=true},function(error){failure=String(error&&error.stack||error);done=true});
)JS", "oracle");
    QTimer timeout, poll;
    timeout.setSingleShot(true);
    QObject::connect(&timeout, &QTimer::timeout, &app, [&] { app.exit(4); });
    QObject::connect(&poll, &QTimer::timeout, &app, [&] {
        if (!engine.globalObject().property("done").toBool()) return;
        const auto failure = engine.globalObject().property("failure");
        if (!failure.isNull()) { fprintf(stderr, "%s\n", qPrintable(failure.toString())); app.exit(5); return; }
        const auto output = engine.evaluate("JSON.stringify(result)").toString().toUtf8();
        fwrite(output.data(), 1, output.size(), stdout); fputc('\n', stdout); app.quit();
    });
    timeout.start(30000); poll.start(0);
    return app.exec();
}
