import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,message){if(!ok){console.error("SERVER_UPDATES_QML_FAIL: "+message);Qt.exit(1)}}
 QtObject{id:b
  property string selectedEnvironment:"local"
  property var viewState:({})
  property var state:({environments:[{id:"local",name:"Local",state:"ready"},{id:"node",name:"Node",state:"ready"},{id:"desk",name:"Desk",state:"ready"}]})
  property var catalogs:({})
  property var settingsResult:({busy:false,error:"",environment:"local",fields:[]})
  property var validatedT3:({tag:"v0.1.1-nightly.20261020.3386",date:"2026-10-20"})
  property string serverUpdateMode:"auto"
  property var serverUpdateErrors:({})
  property var serverUpdates:({mode:"auto",environments:{
   local:{current:"0.0.46-nightly.20261013.3042",channel:"nightly",latest:"0.1.3-nightly.20261024.3604",canUpdate:true},
   node:{current:"0.0.46-nightly.20261013.3042",channel:"nightly",latest:"0.1.3-nightly.20261024.3604",canUpdate:true,state:"failed",error:"t3_remote_start_failed"},
   desk:{current:"0.0.46-nightly.20261013.3042",channel:"nightly",latest:"0.1.3-nightly.20261024.3604",canUpdate:false,reason:"backend_owned_by_gui"}}})
  property string updated:"";property string mode:""
  function serverVersion(environment){return (serverUpdates.environments[environment] || {}).current || ""}
  function serverUpdate(environment){return serverUpdates.environments[environment] || null}
  function updateServer(environment){updated=environment}
  function setServerUpdateMode(value){mode=value;serverUpdateMode=value}
  function t3VersionNewer(v){return false}
  function requestSettings(environment,key,value){}
  function refreshCatalogIn(environment){}
  function rememberView(value){}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:1200;implicitHeight:900;Test.T3Preferences{id:view;anchors.fill:parent;bridge:b}}
 TestCase{id:keys;parent:win.contentItem;when:false}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  if(phase===0){view.query="T3 server updates";phase=1
  }else if(phase===1){
   check(keys.findChild(view,"preferencesT3Compatibility").visible,"search finds the server updates section")
   var local=keys.findChild(view,"t3ServerUpdateStatus_local");check(local&&local.visible,"available update is shown")
   check(local.text===Test.T3I18n.tr("T3 {version} is available. It installs once the server has been idle for ten minutes.",{version:"0.1.3-nightly.20261024.3604"}),"automatic mode says when it installs: "+local.text)
   var now=keys.findChild(view,"t3ServerUpdateNow_local");check(now.visible,"update now is offered");now.clicked();check(b.updated==="local","update now runs that environment's update")
   var node=keys.findChild(view,"t3ServerUpdateStatus_node");check(node.text.indexOf(Test.T3I18n.tr("the server did not come back after updating."))>=0,"a failed update says why: "+node.text)
   check(keys.findChild(view,"t3ServerUpdateNow_node").visible,"a failed update can be retried")
   var desk=keys.findChild(view,"t3ServerUpdateStatus_desk");check(desk.text.indexOf(Test.T3I18n.tr("the T3 Code desktop app runs this server and updates it itself."))>=0,"a desktop-owned server is explained: "+desk.text)
   check(!keys.findChild(view,"t3ServerUpdateNow_desk").visible,"a desktop-owned server is never updated by ot3")
   keys.findChild(view,"preferenceChoice_t3ServerUpdates").activated(1);check(b.mode==="notify","mode choice is saved");phase=2
  }else if(phase===2){
   check(keys.findChild(view,"t3ServerUpdateStatus_local").text===Test.T3I18n.tr("T3 {version} is available.",{version:"0.1.3-nightly.20261024.3604"}),"notify mode only announces the release")
   var s=JSON.parse(JSON.stringify(b.serverUpdates));s.environments.local.state="running";b.serverUpdates=s;phase=3
  }else if(phase===3){
   check(!keys.findChild(view,"t3ServerUpdateNow_local").visible,"no second update while one runs")
   console.log("SERVER_UPDATES_QML_PASS: status, update now, failures, desktop ownership and mode");Qt.exit(0)
  }
 }}
}
