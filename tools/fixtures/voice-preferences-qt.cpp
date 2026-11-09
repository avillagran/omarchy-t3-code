#include <QCoreApplication>
#include <QFile>
#include <QJSEngine>
#include <QTimer>
#include <cstdio>
static QString read(const char *path){QFile file(QString::fromLocal8Bit(path));if(!file.open(QIODevice::ReadOnly))qFatal("Cannot read preference fixture");auto text=QString::fromUtf8(file.readAll());if(text.startsWith(".pragma library\n"))text.remove(0,16);return text;}
int main(int argc,char **argv){QCoreApplication app(argc,argv);if(argc!=4)return 2;QJSEngine engine;
auto evaluate=[&](const QString &text){auto value=engine.evaluate(text);if(value.isError()){fprintf(stderr,"%d %s\n",value.property("lineNumber").toInt(),qPrintable(value.toString()));exit(3);}};
evaluate("var primitives=(function(){"+read(argv[1])+";return create;})()();var preferences=(function(){"+read(argv[2])+";return create;})();");evaluate(read(argv[3]));
evaluate("var done=false,failure=null,result=null;runVoicePreferencesOracle(function(platform){platform.flatMap=primitives.flatMap;return preferences(platform)},primitives.flatMap).then(function(value){result=value;done=true},function(error){failure=String(error.stack||error);done=true});");
QTimer timeout,poll;timeout.setSingleShot(true);QObject::connect(&timeout,&QTimer::timeout,&app,[&]{app.exit(4);});QObject::connect(&poll,&QTimer::timeout,&app,[&]{if(!engine.globalObject().property("done").toBool())return;auto failure=engine.globalObject().property("failure");if(!failure.isNull()){fprintf(stderr,"%s\n",qPrintable(failure.toString()));app.exit(5);return;}auto bytes=engine.evaluate("JSON.stringify(result)").toString().toUtf8();fwrite(bytes.data(),1,bytes.size(),stdout);fputc('\n',stdout);app.quit();});timeout.start(10000);poll.start(0);return app.exec();}
