// Independent private display selection owner. No user clipboard/history is read.
#include <QGuiApplication>
#include <QWindow>
#include <QRasterWindow>
#include <QPainter>
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
class PeerWindow:public QRasterWindow {
 void paintEvent(QPaintEvent *)override{QPainter paint(this);paint.fillRect(QRect(QPoint(),size()),QColor("#eeeeee"));}
};
int main(int argc,char **argv){
 QGuiApplication app(argc,argv);QByteArray input;PeerWindow window;if(qEnvironmentVariableIsSet("O3_TEST_WAYLAND")){window.setTitle("Owned synthetic clipboard peer");window.resize(320,240);window.show();window.requestActivate();}
 auto reply=[&window](QJsonObject row){row["active"]=window.isActive();const auto bytes=QJsonDocument(row).toJson(QJsonDocument::Compact);fprintf(stdout,"%s\n",bytes.constData());fflush(stdout);};
 auto *clipboard=QGuiApplication::clipboard();
 QSocketNotifier read(STDIN_FILENO,QSocketNotifier::Read);
 fcntl(STDIN_FILENO,F_SETFL,fcntl(STDIN_FILENO,F_GETFL)|O_NONBLOCK);
 QObject::connect(&read,&QSocketNotifier::activated,&app,[&]{
  char data[4096];ssize_t count;while((count=::read(STDIN_FILENO,data,sizeof data))>0){input.append(data,count);if(input.size()>65536){app.exit(2);return;}}
  if(count==0){app.quit();return;}int end;
  while((end=input.indexOf('\n'))>=0){auto row=QJsonDocument::fromJson(input.left(end)).object();input.remove(0,end+1);auto operation=row["op"].toString();
   if(operation=="text")clipboard->setText(row["text"].toString());
   else if(operation=="large")clipboard->setText(QString(200000,QChar('L')));
   else if(operation=="at-limit")clipboard->setText(QString(16*1024*1024,QChar('V')));
   else if(operation=="oversized")clipboard->setText(QString(17*1024*1024,QChar('B')));
   else if(operation=="many"){auto *mime=new QMimeData;for(int i=0;i<130;i++)mime->setData(QString("application/x-owned-%1").arg(i),QByteArray("x"));mime->setText("owned metadata cap");clipboard->setMimeData(mime);}
   else if(operation=="image"){QImage image(2,2,QImage::Format_ARGB32);image.fill(QColor("#123456"));clipboard->setImage(image);}
   else if(operation=="read-image"){const auto image=clipboard->image();const auto pixel=image.isNull()?QColor():image.pixelColor(0,0);reply({{"id",row["id"]},{"width",image.width()},{"height",image.height()},{"red",pixel.red()},{"green",pixel.green()},{"blue",pixel.blue()},{"alpha",pixel.alpha()}});}
   else if(operation=="read")reply({{"id",row["id"]},{"text",clipboard->text()}});
   else if(operation=="quit"){app.quit();return;}
   if(operation=="text"||operation=="image"||operation=="large"||operation=="at-limit"||operation=="oversized"||operation=="many")QTimer::singleShot(40,&app,[reply,row]{reply({{"id",row["id"]},{"ready",true}});});
  }
 });
 reply({{"ready",true}});QTimer::singleShot(40000,&app,&QCoreApplication::quit);return app.exec();
}
