import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property bool captured:false
 function check(ok,msg){if(!ok){console.error("PREFERENCES_SCOPE_QML_FAIL "+msg);Qt.exit(1)}}
 property var fixtureFields:[{key:"defaultRuntimeMode",label:"Permissions",kind:"choice",value:"approval-required",choices:[{id:"full-access",name:"Full access"},{id:"approval-required",name:"Supervised"}],source:"mixed",mixed:true,projectScoped:true,overridden:true,scopeWritable:true},{key:"enableProviderUpdateChecks",label:"Check provider updates",kind:"bool",value:true,scopeWritable:false,projectScoped:true}]
 QtObject{id:b
  property string selectedEnvironment:"local"
  property var viewState:({})
  property var state:({environments:[{id:"local",name:"Local",state:"ready"},{id:"remote",name:"Remote",state:"ready"}],projects:[{key:"local/p1",id:"p1",title:"Project",environmentId:"local",workspaceRoot:"/repo",repositoryIdentity:{origin:{canonicalKey:"shared"}}},{key:"remote/p2",id:"p2",title:"Project",environmentId:"remote",workspaceRoot:"/work/repo",repositoryIdentity:{origin:{canonicalKey:"shared"}}}]})
  property var catalogs:({})
  property var settingsResult:({environment:"local",busy:false,error:"",fields:[]})
  property var sent:({});property int serial:0
  function refreshCatalogIn(environment){}
  function requestSettings(environment,key,value,scope,clear){sent={environment:environment,key:key || "",value:value,scope:JSON.parse(JSON.stringify(scope)),clear:!!clear};settingsResult={environment:environment,busy:true,requestId:String(++serial),fields:[],error:""}}
  function reply(){settingsResult={environment:sent.environment,scope:sent.scope,requestId:settingsResult.requestId,busy:false,error:"",fields:root.fixtureFields,writable:true}}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:1200;implicitHeight:900;Test.T3Preferences{id:view;anchors.fill:parent;bridge:b}}
 TestCase{id:keys;parent:win.contentItem;when:false}
 Timer{interval:200;repeat:true;running:true;onTriggered:{
  if(phase===0){check(b.sent.scope.targets.length===1&&b.sent.scope.targets[0].projectId===null,"default read is environment-wide");b.reply();view.environment="*";phase=1}
  else if(phase===1){check(b.sent.scope.targets.length===2&&b.sent.scope.targets.every(function(t){return t.projectId===null}),"all environments explicitly captured");b.reply();view.selectProject("repository/shared");phase=2}
  else if(phase===2){check(view.page==="projects"&&b.sent.scope.targets.length===2,"one grouped project targets both native checkouts");check(b.sent.scope.targets[0].projectId==="p1"&&b.sent.scope.targets[1].projectId==="p2","native project IDs preserved");b.reply();phase=3}
  else if(phase===3){var choice=keys.findChild(view,"preferenceChoice_defaultRuntimeMode"),source=keys.findChild(view,"preferenceSource_defaultRuntimeMode");check(choice.currentIndex===-1&&source.visible,"mixed value is explicit");check(!keys.findChild(view,"preference_enableProviderUpdateChecks").enabled,"environment-only row is not writable at project scope");view.grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-preferences-scoped.png")});phase=30}
  else if(phase===30){if(!root.captured)return;keys.findChild(view,"inheritPreference_defaultRuntimeMode").clicked();check(b.sent.clear&&b.sent.key==="defaultRuntimeMode"&&b.sent.scope.targets.length===2,"inherit removes overrides on captured checkouts");b.reply();view.checkoutKey="remote/p2";phase=4}
  else if(phase===4){check(b.sent.scope.targets.length===1&&b.sent.scope.targets[0].environment==="remote"&&b.sent.scope.targets[0].projectId==="p2","checkout axis narrows the project");b.reply();view.save("defaultRuntimeMode","full-access");check(b.sent.scope.targets.length===1&&b.sent.scope.targets[0].projectId==="p2"&&b.sent.value==="full-access","save retains exact selected checkout");b.reply();view.checkoutKey="removed";phase=5}
  else if(phase===5){check(b.sent.scope.targets.length===0,"stale checkout never broadens to all projects");console.log("PREFERENCES_SCOPE_QML_PASS: explicit environment/project/checkout axes, mixed values, inherit and stale selection");Qt.quit()}
 }}
 Timer{interval:8000;running:true;onTriggered:{console.error("PREFERENCES_SCOPE_QML_FAIL timeout");Qt.exit(1)}}
}
