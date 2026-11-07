// This member stays on the real GUI display/seat. No clipboard body crosses QML.
#include "clipboardplatform.h"
#include "waylandoffers.h"
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSocketNotifier>
#include <QUuid>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <poll.h>
#include <cerrno>
#include <cstring>
struct ClipboardPlatform::State {
 ClipboardPlatform *owner;int listener=-1,peer=-1,pidfd=-1;pid_t worker=0;QString address,binding,session;bool admitted=false;
 QSocketNotifier *acceptor=nullptr,*reader=nullptr,*death=nullptr;
 std::unique_ptr<NativeWaylandOffers> offers;
 explicit State(ClipboardPlatform *owner):owner(owner){}
 void closePeer(){session.clear();delete reader;reader=nullptr;if(peer>=0)::close(peer);peer=-1;if(admitted){admitted=false;emit owner->workerAdmittedChanged();}}
 void retire(){closePeer();delete death;death=nullptr;if(pidfd>=0)::close(pidfd);pidfd=-1;worker=0;binding.clear();if(acceptor)acceptor->setEnabled(false);}
 ~State(){retire();delete acceptor;if(listener>=0)::close(listener);}
 bool workerAlive()const{pollfd value{pidfd,POLLIN,0};return pidfd>=0&&poll(&value,1,0)==0;}
 void receive(){
  if(!workerAlive()){retire();return;}
  char bytes[4096];alignas(cmsghdr) char ancillary[CMSG_SPACE(sizeof(ucred))+CMSG_SPACE(sizeof(int)*4)];iovec data{bytes,sizeof bytes};msghdr message{};message.msg_iov=&data;message.msg_iovlen=1;message.msg_control=ancillary;message.msg_controllen=sizeof ancillary;
  const auto size=recvmsg(peer,&message,MSG_DONTWAIT|MSG_CMSG_CLOEXEC);if(size<0&&(errno==EAGAIN||errno==EINTR))return;
  bool credential=false,unexpected=false;for(auto *header=CMSG_FIRSTHDR(&message);header;header=CMSG_NXTHDR(&message,header)){
   if(header->cmsg_level==SOL_SOCKET&&header->cmsg_type==SCM_CREDENTIALS&&header->cmsg_len==CMSG_LEN(sizeof(ucred))){ucred value;memcpy(&value,CMSG_DATA(header),sizeof value);credential=value.pid==worker&&value.uid==getuid();}
   else{unexpected=true;if(header->cmsg_level==SOL_SOCKET&&header->cmsg_type==SCM_RIGHTS){const auto count=(header->cmsg_len-CMSG_LEN(0))/sizeof(int);const auto *fds=reinterpret_cast<const int *>(CMSG_DATA(header));for(size_t i=0;i<count;i++)::close(fds[i]);}}
  }
  if(size<=0||message.msg_flags&(MSG_TRUNC|MSG_CTRUNC)||!credential||unexpected){closePeer();return;}
  if(admitted&&workerAlive()&&QByteArray(bytes,int(size))==QByteArrayLiteral("{\"op\":\"probe\",\"version\":1}")){if(send(peer,"alive",5,MSG_NOSIGNAL|MSG_DONTWAIT)!=5)closePeer();return;}
  if(QByteArray(bytes,int(size))!=QByteArrayLiteral("{\"op\":\"hello\",\"version\":1}")||admitted||!workerAlive()){closePeer();return;}
  session=QUuid::createUuid().toString(QUuid::WithoutBraces);
  const auto reply=QJsonDocument(QJsonObject{{"type","admitted"},{"version",1},{"binding",session}}).toJson(QJsonDocument::Compact);
  if(send(peer,reply.constData(),size_t(reply.size()),MSG_NOSIGNAL|MSG_DONTWAIT)!=reply.size()){closePeer();return;}
  admitted=true;emit owner->workerAdmittedChanged();
 }
 void accept(){
  const auto candidate=accept4(listener,nullptr,nullptr,SOCK_NONBLOCK|SOCK_CLOEXEC);if(candidate<0)return;
  ucred credentials{};socklen_t size=sizeof credentials;int pass=1;
  if(worker<=0||!workerAlive()||peer>=0||getsockopt(candidate,SOL_SOCKET,SO_PEERCRED,&credentials,&size)||size!=sizeof credentials||credentials.pid!=worker||credentials.uid!=getuid()||setsockopt(candidate,SOL_SOCKET,SO_PASSCRED,&pass,sizeof pass)){::close(candidate);return;}
  peer=candidate;reader=new QSocketNotifier(peer,QSocketNotifier::Read,owner);QObject::connect(reader,&QSocketNotifier::activated,owner,[this]{receive();});receive();
 }
};
ClipboardPlatform::ClipboardPlatform(QObject *parent):QObject(parent),state(std::make_unique<State>(this)){
 // Merely constructing the member registers metadata asynchronously. The normal
 // Qt event loop reads the socket; the member dispatches only its own proxies.
 if(!qGuiApp||!QGuiApplication::platformName().startsWith("wayland"))return;
 state->offers=std::make_unique<NativeWaylandOffers>();
 state->listener=socket(AF_UNIX,SOCK_SEQPACKET|SOCK_NONBLOCK|SOCK_CLOEXEC,0);if(state->listener<0)return;int pass=1;if(setsockopt(state->listener,SOL_SOCKET,SO_PASSCRED,&pass,sizeof pass)){::close(state->listener);state->listener=-1;return;}
 const auto name=("o3-clipboard-"+QUuid::createUuid().toString(QUuid::WithoutBraces)).toLatin1();sockaddr_un address{};address.sun_family=AF_UNIX;memcpy(address.sun_path+1,name.constData(),size_t(name.size()));
 if(::bind(state->listener,reinterpret_cast<sockaddr *>(&address),socklen_t(offsetof(sockaddr_un,sun_path)+1+name.size()))||listen(state->listener,4)){::close(state->listener);state->listener=-1;return;}
 state->address=QString::fromLatin1(name);state->acceptor=new QSocketNotifier(state->listener,QSocketNotifier::Read,this);state->acceptor->setEnabled(false);connect(state->acceptor,&QSocketNotifier::activated,this,[this]{state->accept();});
}
ClipboardPlatform::~ClipboardPlatform()=default;
QString ClipboardPlatform::endpoint()const{return state->address;}
bool ClipboardPlatform::available()const{return state->listener>=0;}
bool ClipboardPlatform::workerAdmitted()const{return state->admitted;}
QString ClipboardPlatform::bindWorker(qint64 pid){
 if(!available()||pid<=0||pid>2147483647)return {};
 QFile status(QString("/proc/%1/status").arg(pid));if(!status.open(QIODevice::ReadOnly))return {};const auto rows=status.readAll().split('\n');bool child=false;for(const auto &row:rows)if(row.startsWith("PPid:"))child=row.mid(5).trimmed().toLongLong()==getpid();if(!child)return {};
 const auto descriptor=int(syscall(SYS_pidfd_open,pid,0));if(descriptor<0)return {};
 state->retire();state->worker=pid_t(pid);state->pidfd=descriptor;state->binding=QUuid::createUuid().toString(QUuid::WithoutBraces);state->death=new QSocketNotifier(descriptor,QSocketNotifier::Read,this);const auto binding=state->binding;connect(state->death,&QSocketNotifier::activated,this,[this,binding]{retireWorker(binding);});state->acceptor->setEnabled(true);return binding;
}
void ClipboardPlatform::retireWorker(const QString &binding){if(!binding.isEmpty()&&binding==state->binding)state->retire();}
