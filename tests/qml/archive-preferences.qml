import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property bool captured:false
 function check(ok,msg){if(!ok){console.error("ARCHIVE_PREFERENCES_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property string selectedEnvironment:"local"
  property var viewState:({})
  property var state:({environments:[{id:"local",name:"Local",state:"ready"},{id:"remote",name:"Remote",state:"ready"}],projects:[{key:"local/p",id:"p",title:"Project",environmentId:"local",workspaceRoot:"/repo"}]})
  property var catalogs:({})
  property var settingsResult:({environment:"local",busy:false,error:"permission_denied",fields:[]})
  property var clientSettings:({data:{confirmThreadDelete:true}})
  property var archiveView:({signature:"",sessions:{},error:""})
  property var archiveActions:({});property var archiveCommands:({});property var sent:[]
  property int reads:0
  function requestSettings(){reads++}
  function refreshCatalogIn(){}
  function dismissArchive(){archiveView={signature:"",sessions:{},error:""}}
  function requestArchive(targets,force){sent=JSON.parse(JSON.stringify(targets));var sessions={};targets.forEach(function(t){sessions[t.environment]={busy:false,error:"",scope:{environment:t.environment,projectIds:t.projectId===null?null:[t.projectId]},data:{originKey:t.environment,viewId:"fixture",writable:true,groups:[{project:{id:"p",title:"Omarchy",projectIcon:{kind:"emoji",emoji:"🌱"}},threads:[{id:"one",projectId:"p",environment:t.environment,title:"Review sidebar behavior",archivedAt:"2026-10-17T10:00:00.000Z",createdAt:"2026-10-16T12:00:00.000Z"},{id:"two",projectId:"p",environment:t.environment,title:"HTML renderer",archivedAt:"2026-10-16T10:00:00.000Z",createdAt:"2026-10-15T12:00:00.000Z"}]}]}}});archiveView={signature:JSON.stringify(targets),sessions:sessions,error:targets.length?"":"settings_scope_empty"}}
  function archiveBlocking(){return false}
  function archiveKey(r){return r.environment+"/"+r.originKey+"/"+(r.threadId || r.id)}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:1200;implicitHeight:900;Test.T3Preferences{id:view;anchors.fill:parent;bridge:b}}
 TestCase{id:keys;when:false}
 Timer{interval:160;repeat:true;running:true;onTriggered:{
  if(phase===0){var reads=b.reads;check(keys.findChild(view,"preferencesTab_archived"),"archive navigation entry");view.page="archived";check(b.sent.length===1&&b.sent[0].projectId===null,"archive starts in selected native environment");check(b.reads===reads,"archive does not require unrelated server settings permission");view.environment="*";phase=1}
  else if(phase===1){check(b.sent.length===2,"all connected environments");view.selectProject("local/p");check(view.page==="archived"&&b.sent.length===1&&b.sent[0].projectId==="p","project picker scopes archive without switching page");phase=2}
  else if(phase===2){var list=keys.findChild(view,"t3ArchivedThreadList");check(list&&list.count===2,"same archive component is in preferences");view.grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-archive-preferences.png")});phase=3}
  else if(phase===3){if(!captured)return;view.checkoutKey="removed";phase=4}
  else if(phase===4){check(b.sent.length===0,"removed checkout cannot broaden archive scope");var reads=b.reads;view.page="general";check(b.reads>reads,"leaving archive reloads server preferences for current scope");console.log("ARCHIVE_PREFERENCES_QML_PASS: navigation, native scoped environment/project/checkout selection, no unrelated settings read, stale selection and preferences restoration");Qt.quit()}
 }}
 Timer{interval:8000;running:true;onTriggered:{console.error("ARCHIVE_PREFERENCES_QML_FAIL timeout");Qt.exit(1)}}
}
