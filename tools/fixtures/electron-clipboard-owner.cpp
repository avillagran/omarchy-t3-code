// Independent private X11 selection owner. No user clipboard/history is read.
#include <QGuiApplication>
#include <QClipboard>
#include <QMimeData>
#include <QImage>
#include <QColor>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSocketNotifier>
#include <QTimer>
#include <QFile>
#include <cstdio>
#include <unistd.h>
#include <fcntl.h>
int main(int argc,char **argv){
 QGuiApplication app(argc,argv);QByteArray input;
 auto reply=[](const QJsonObject &row){const auto bytes=QJsonDocument(row).toJson(QJsonDocument::Compact);fprintf(stdout,"%s\n",bytes.constData());fflush(stdout);};
 auto *clipboard=QGuiApplication::clipboard();
 QSocketNotifier read(STDIN_FILENO,QSocketNotifier::Read);
 fcntl(STDIN_FILENO,F_SETFL,fcntl(STDIN_FILENO,F_GETFL)|O_NONBLOCK);
 QObject::connect(&read,&QSocketNotifier::activated,&app,[&]{
  char data[4096];ssize_t count;while((count=::read(STDIN_FILENO,data,sizeof data))>0){input.append(data,count);if(input.size()>65536){app.exit(2);return;}}
  if(count==0){app.quit();return;}int end;
  while((end=input.indexOf('\n'))>=0){auto row=QJsonDocument::fromJson(input.left(end)).object();input.remove(0,end+1);auto operation=row["op"].toString();
   if(operation=="text")clipboard->setText(row["text"].toString());
   else if(operation=="image"){QImage image(2,2,QImage::Format_ARGB32);image.fill(QColor("#123456"));clipboard->setImage(image);}
   else if(operation=="read")reply({{"id",row["id"]},{"text",clipboard->text()}});
   else if(operation=="quit"){app.quit();return;}
   if(operation=="text"||operation=="image")QTimer::singleShot(40,&app,[reply,row]{reply({{"id",row["id"]},{"ready",true}});});
  }
 });
 reply({{"ready",true}});QTimer::singleShot(40000,&app,&QCoreApplication::quit);return app.exec();
}
