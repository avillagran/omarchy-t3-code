#include <QImage>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <cstdio>
int main(int argc,char **argv){
 if(argc!=2)return 2;
 const QFileInfo file(QString::fromLocal8Bit(argv[1]));
 if(!file.isFile()||file.isSymLink()||file.size()>32*1024*1024)return 3;
 const QImage image(file.absoluteFilePath());if(image.isNull()||image.width()<20||image.height()<20)return 4;
 const auto json=QJsonDocument(QJsonObject{{"width",image.width()},{"height",image.height()},{"color",image.pixelColor(image.width()-10,image.height()-10).name()}}).toJson(QJsonDocument::Compact);
 std::fwrite(json.constData(),1,json.size(),stdout);std::fputc('\n',stdout);return 0;
}
