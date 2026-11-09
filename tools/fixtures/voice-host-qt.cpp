#include <QCoreApplication>
#include <QFile>
#include <QJSEngine>
#include <QTimer>
#include <cstdio>

static QString read(const char *name) {
    QFile file(QString::fromLocal8Bit(name));
    if (!file.open(QIODevice::ReadOnly)) qFatal("Cannot read voice bridge fixture");
    auto text = QString::fromUtf8(file.readAll());
    if (text.startsWith(".pragma library\n")) text.remove(0, 16);
    return text;
}
int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);
    if (argc != 7) return 2;
    QJSEngine engine;
    fprintf(stderr, "%s\n", qPrintable(engine.evaluate("JSON.stringify({flatMap:typeof [].flatMap,normalize:typeof ''.normalize})").toString()));
    const auto evaluate = [&](const QString &code, const QString &name) {
        const auto value = engine.evaluate(code, name);
        if (value.isError()) {
            fprintf(stderr, "%s:%d %s\n", qPrintable(name), value.property("lineNumber").toInt(), qPrintable(value.toString()));
            exit(3);
        }
    };
    const QString names[] = {"primitives", "voice", "input", "preferences", "host"};
    for (int index = 0; index < 5; ++index)
        evaluate("var " + names[index] + "=(function(){" + read(argv[index+1]) + ";return create;})();", names[index]);
    evaluate(read(argv[6]), "driver");
    evaluate(R"JS(
var done=false,failure=null,result=null;
runNativeVoiceHostOracle({primitives,voice,input,preferences,host}).then(function(value){result=value;done=true},function(error){failure=String(error&&error.message||error)+"\n"+String(error&&error.stack||"");done=true});
)JS", "run");
    QTimer timeout, poll;
    timeout.setSingleShot(true);
    QObject::connect(&timeout, &QTimer::timeout, &app, [&] { app.exit(4); });
    QObject::connect(&poll, &QTimer::timeout, &app, [&] {
        if (!engine.globalObject().property("done").toBool()) return;
        const auto failure = engine.globalObject().property("failure");
        if (!failure.isNull()) { fprintf(stderr, "%s\n", qPrintable(failure.toString())); app.exit(5); return; }
        const auto bytes = engine.evaluate("JSON.stringify(result)").toString().toUtf8();
        fwrite(bytes.data(), 1, bytes.size(), stdout); fputc('\n', stdout); app.quit();
    });
    timeout.start(30000); poll.start(0);
    return app.exec();
}
