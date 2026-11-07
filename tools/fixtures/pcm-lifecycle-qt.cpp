#include <QCoreApplication>
#include <QFile>
#include <QJSEngine>
#include <QTimer>
#include <cstdio>

static QString read(const QString &path) {
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) qFatal("Missing capture lifecycle fixture");
    QString source=QString::fromUtf8(file.readAll());
    if (source.startsWith(".pragma library\n")) source.remove(0,16);
    return source;
}
int main(int argc,char **argv) {
    QCoreApplication app(argc,argv);
    if (argc!=4) return 2;
    QJSEngine engine;
    auto checked=[&](const QString &source,const QString &name) {
        auto value=engine.evaluate(source,name);
        if (value.isError()) {
            fprintf(stderr,"%s:%d: %s\n",qPrintable(name),value.property("lineNumber").toInt(),qPrintable(value.toString()));
            exit(3);
        }
        return value;
    };
    checked("var captureFactory=(function(){"+read(argv[1])+";return createCapture})();",argv[1]);
    checked("var abortPlatform=(function(){"+read(argv[3])+";return create()})();",argv[3]);
    checked(read(argv[2]),argv[2]);
    checked(R"JS(var completed=false,failure=null,result=null;
        runCaptureLifecycle(captureFactory,function(){return new abortPlatform.AbortController()}).then(function(value){result=value;completed=true},function(error){failure=String(error)+'\n'+String(error&&error.stack||'');completed=true});)JS","run-lifecycle");
    QTimer poll;
    QObject::connect(&poll,&QTimer::timeout,&app,[&]() {
        if (!engine.globalObject().property("completed").toBool()) return;
        if (!engine.globalObject().property("failure").isNull()) {
            fprintf(stderr,"%s\n",qPrintable(engine.globalObject().property("failure").toString()));
            app.exit(4);
            return;
        }
        const auto output=checked("JSON.stringify(result)","serialize").toString().toUtf8();
        fwrite(output.data(),1,output.size(),stdout);fputc('\n',stdout);app.quit();
    });
    poll.start(0);
    QTimer deadline;
    deadline.setSingleShot(true);
    QObject::connect(&deadline,&QTimer::timeout,&app,[&](){fprintf(stderr,"Lifecycle deadline\n");app.exit(5);});
    deadline.start(20000);
    return app.exec();
}
