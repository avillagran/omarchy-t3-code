// Toolkit behavior proof, not a product clipboard endpoint. Private GUI only.
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QClipboard>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSocketNotifier>
#include <QTimer>
#include <QFileInfo>
#include <unistd.h>
#include <fcntl.h>
#include <cstdio>
#include "waylandoffers.h"
int main(int argc,char **argv){
 QGuiApplication app(argc,argv);if(argc!=4)return 2;const bool enabled=QString::fromUtf8(argv[2])=="1";QQmlApplicationEngine engine;engine.load(QUrl::fromLocalFile(QFileInfo(QString::fromUtf8(argv[3])).absoluteFilePath()));if(engine.rootObjects().size()!=1)return 2;auto *root=engine.rootObjects().first();std::unique_ptr<NativeWaylandOffers> member;if(enabled)member=std::make_unique<NativeWaylandOffers>();
 auto reply=[](const QJsonObject &row){const auto line=QJsonDocument(row).toJson(QJsonDocument::Compact);fprintf(stdout,"%s\n",line.constData());fflush(stdout);};
 QByteArray input;fcntl(STDIN_FILENO,F_SETFL,fcntl(STDIN_FILENO,F_GETFL)|O_NONBLOCK);QSocketNotifier readable(STDIN_FILENO,QSocketNotifier::Read);QObject::connect(&readable,&QSocketNotifier::activated,&app,[&]{char bytes[4096];ssize_t count;while((count=read(STDIN_FILENO,bytes,sizeof bytes))>0){input.append(bytes,count);if(input.size()>4096){app.exit(2);return;}}if(count==0){app.quit();return;}int end;while((end=input.indexOf('\n'))>=0){const auto row=QJsonDocument::fromJson(input.left(end)).object();input.remove(0,end+1);const auto op=row["op"].toString();if(op=="quit"){app.quit();return;}if(op=="copy")QGuiApplication::clipboard()->setText("owned toolkit clipboard");QJsonObject result{{"id",row["id"]},{"typed",root->property("typed").toString()},{"drops",root->property("drops").toInt()},{"payload",root->property("payload").toString()},{"entered",root->property("entered").toInt()},{"active",root->property("active").toBool()},{"member",enabled},{"streamBytes",member?qint64(member->streamedBytes):0}};if(op=="read")result["clipboard"]=QGuiApplication::clipboard()->text();reply(result);}});
 QElapsedTimer startup;startup.start();QTimer ready;ready.setInterval(10);QObject::connect(&ready,&QTimer::timeout,&app,[&]{if(!member||member->ready()){reply({{"ready",true}});ready.stop();}else if(startup.elapsed()>2000)app.exit(2);});ready.start();QTimer::singleShot(20000,&app,&QCoreApplication::quit);return app.exec();
}
