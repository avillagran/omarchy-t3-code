// Private Qt test observer; it never changes product state or process ownership.
#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtCore/QStringList>
#include <QtCore/QVariant>
#include <QtQml/QQmlExtensionPlugin>
#include <QtQml/qqml.h>
class ProcessInspector:public QObject {
 Q_OBJECT
 Q_PROPERTY(QObject* observed READ observed NOTIFY observedChanged)
public:
 using QObject::QObject;
 QObject* observed()const {return m_observed.data();}
 Q_INVOKABLE void find(QObject* owner,const QString& binary) {
  QObject* match=nullptr;
  if(owner&&!binary.isEmpty())for(auto* candidate:owner->findChildren<QObject*>()){
   if(candidate->property("command").toStringList()!=QStringList{binary,QStringLiteral("watch")})continue;
   if(!candidate->property("stdout").value<QObject*>())continue;
   if(candidate->metaObject()->indexOfSignal("exited(int,QProcess::ExitStatus)")<0)continue;
   if(match){match=nullptr;break;}
   match=candidate;
  }
  if(m_observed.data()==match)return;
  QObject::disconnect(m_destroyed);
  m_observed=match;
  if(match)m_destroyed=QObject::connect(match,&QObject::destroyed,this,[this]{m_observed.clear();emit observedChanged();});
  else m_destroyed={};
  emit observedChanged();
 }
signals:
 void observedChanged();
private:
 QPointer<QObject> m_observed;
 QMetaObject::Connection m_destroyed;
};
class ProcessInspectorPlugin final:public QQmlExtensionPlugin {
 Q_OBJECT
 Q_PLUGIN_METADATA(IID QQmlExtensionInterface_iid)
public:
 void registerTypes(const char* uri)override {qmlRegisterType<ProcessInspector>(uri,1,0,"ProcessInspector");}
};
#include "ProcessInspector.moc"
