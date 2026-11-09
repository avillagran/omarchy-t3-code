#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QJSEngine>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QTimer>
#include <cstdio>
#include <cstdlib>
static QString read(const char *path){QFile file(QString::fromLocal8Bit(path));if(!file.open(QIODevice::ReadOnly))qFatal("Cannot read watch fixture");auto text=QString::fromUtf8(file.readAll());if(text.startsWith(".pragma library\n"))text.remove(0,16);return text;}
int main(int argc,char **argv){
 QCoreApplication app(argc,argv);if(argc!=8||QCoreApplication::applicationPid()==1||!QFile::exists("/proc/1/stat"))return 2;QJSEngine engine;QProcess process;QByteArray input,errors;bool eof=false,exited=false;int exitCode=-1;QProcess::ExitStatus exitStatus=QProcess::CrashExit;
 auto evaluate=[&](const QString &code,const QString &name){auto value=engine.evaluate(code,name);if(value.isError()){fprintf(stderr,"%s:%d %s\n",qPrintable(name),value.property("lineNumber").toInt(),qPrintable(value.toString()));exit(3);}return value;};
 const QString names[]={"primitives","voice","input","preferences","host"};
 for(int i=0;i<5;i++)evaluate("var "+names[i]+"=(function(){"+read(argv[i+2])+";return create;})();",names[i]);
 evaluate(read(argv[7]),"driver");evaluate("var done=false,failure=null,result=null,fixture=runVoiceWatchFixture({primitives,voice,input,preferences,host});fixture.result.then(function(value){result=value;done=true},function(error){failure=String(error.stack||error);done=true});","run");
 QObject::connect(&process,&QProcess::readyReadStandardOutput,&app,[&]{input+=process.readAllStandardOutput();if(input.size()>32*1024*1024){app.exit(8);return;}while(input.contains('\n')){auto end=input.indexOf('\n');auto line=input.left(end);input.remove(0,end+1);auto received=engine.globalObject().property("fixture").property("receive").callWithInstance(engine.globalObject().property("fixture"),{QString::fromUtf8(line)});if(received.isError()){fprintf(stderr,"Wire decode: %s\n",qPrintable(received.toString()));app.exit(9);return;}}});
 QObject::connect(&process,&QProcess::readyReadStandardError,&app,[&]{auto chunk=process.readAllStandardError();if(errors.size()+chunk.size()>65536){app.exit(10);return;}errors+=chunk;});
 QObject::connect(&process,qOverload<int,QProcess::ExitStatus>(&QProcess::finished),&app,[&](int code,QProcess::ExitStatus status){exited=true;exitCode=code;exitStatus=status;});
 process.setProgram(QString::fromLocal8Bit(argv[1]));process.start();
 QTimer poll,timeout;timeout.setSingleShot(true);QObject::connect(&timeout,&QTimer::timeout,&app,[&]{fprintf(stderr,"Actual Qt/watch deadline\n");app.exit(4);});
 QObject::connect(&poll,&QTimer::timeout,&app,[&]{
  auto failure=engine.globalObject().property("failure");if(!failure.isNull()){fprintf(stderr,"%s\n",qPrintable(failure.toString()));app.exit(5);return;}
  if(process.state()==QProcess::Running&&!eof){auto outbound=evaluate("fixture.takeWire()","outbound");for(quint32 i=0;i<outbound.property("length").toUInt();i++){auto bytes=outbound.property(i).toString().toUtf8()+'\n';if(process.bytesToWrite()+bytes.size()>32*1024*1024||process.write(bytes)!=bytes.size()){app.exit(11);return;}}if(engine.globalObject().property("done").toBool()&&process.bytesToWrite()==0){eof=true;process.closeWriteChannel();}}
  if(!exited){return;}
  if(!eof||exitStatus!=QProcess::NormalExit||exitCode!=0||!input.isEmpty()){fwrite(errors.data(),1,errors.size(),stderr);fprintf(stderr,"Actual watch exit %d eof=%d\n",exitCode,int(eof));app.exit(6);return;}
  // Require absence across this entire private PID namespace, including any
  // descendants adopted by init; excluding PID1 does not hide live helpers.
  for(const auto &name:QDir("/proc").entryList(QDir::Dirs|QDir::NoDotAndDotDot)){bool ok=false;auto pid=name.toLongLong(&ok);if(ok&&pid!=1&&pid!=QCoreApplication::applicationPid())return;}
  auto peer=QJsonDocument::fromJson(read("/state/peer-receipt.json").toUtf8()).object();if(peer.value("eofShutdown").toBool()!=true||peer.value("actualSyntheticPcmBytes").toInteger()<=0){app.exit(12);return;}
  auto summary=QJsonDocument::fromJson(engine.evaluate("JSON.stringify(result)").toString().toUtf8()).object();summary.insert("peer",peer);summary.insert("watchExit",exitCode);summary.insert("allOtherNamespaceProcessesGone",true);
  auto result=QJsonDocument(summary).toJson(QJsonDocument::Compact);fwrite(result.data(),1,result.size(),stdout);fputc('\n',stdout);app.quit();
 });timeout.start(45000);poll.start(2);return app.exec();
}
