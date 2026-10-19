import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("CONNECTIONS_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var connectionSettings:({busy:false,error:"",revision:"one",localEnabled:true,environments:[{id:"node",name:"NODE",enabled:true,routes:[{connectionId:"ssh-route",kind:"SshConnectionTarget",ssh:{alias:"node",username:"node",port:22}},{connectionId:"url-route",kind:"BearerConnectionTarget",url:"http://node:3773"}]}]})
  property var state:({environments:[{id:"node",state:"ready"}]})
  property var localBackend:({owner:"gui",running:true,canControl:false})
  property var backendStart:({busy:false})
  property var connectionCloud:({busy:false,error:"",environment:"local",status:null})
  property var connectionNetwork:({busy:false,error:"",data:{}})
  property var connectionPreferences:({busy:false,error:"",client:{},github:{environments:[]}})
  signal connectionNetworkFinished(string action,string error)
  function requestConnectionNetwork(action,values){}
  property var connectionOmaBridge:({busy:false,error:"",data:{revision:"r",available:true,identity:true,peers:[],routes:{}}})
  signal connectionOmaBridgeFinished(string action,string error)
  function requestConnectionOmaBridge(action,values){}
  function requestConnectionCloud(environment,action,values){}
  function manageConnectionPreferences(action,values){}
  property string selectedEnvironment:"node"
  property var connectionAccess:({busy:false,environment:"node",data:{session:{authenticated:true,scopes:[]}},created:null})
  signal connectionAccessFinished(string action,string error)
  function requestConnectionAccess(environment,action,values){}
  function clearConnectionCredential(){}
  property var sent:null
  signal connectionsManaged(string action,string error)
  function manageConnections(action,values){sent=Object.assign({action:action},values)}
  function controlBackend(action){sent={backend:action}}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:950;implicitHeight:760
  Rectangle{anchors.fill:parent;color:"#141414"}
  Test.T3Connections{id:view;width:750;anchors.centerIn:parent;bridge:b}
 }
 TestCase{id:keys;when:false}
 Timer{interval:220;running:true;repeat:true;onTriggered:{
  if(phase===0){check(b.sent.action==="read","opening connections reads canonical catalog");keys.findChild(view,"connectionAdd").clicked();phase=1
  }else if(phase===1){keys.findChild(view,"connectionLabel").text="Work";keys.findChild(view,"connectionSshHost").text="node";keys.findChild(view,"connectionSshUser").text="me";keys.findChild(view,"connectionSshPort").text="2222";keys.findChild(view,"connectionSave").clicked();check(b.sent.action==="save-route"&&b.sent.kind==="ssh"&&b.sent.ssh.port===2222&&b.sent.ssh.username==="me","SSH form sends structured target");b.connectionsManaged("save-route","");view.edit(b.connectionSettings.environments[0],b.connectionSettings.environments[0].routes[1]);phase=2
  }else if(phase===2){check(view.kind==="bearer"&&keys.findChild(view,"connectionUrl").text==="http://node:3773","edit bearer route retains endpoint but exposes no stored token");keys.findChild(view,"connectionPairingCode").text="temporary-code";keys.findChild(view,"connectionSave").clicked();check(b.sent.environmentId==="node"&&b.sent.connectionId==="url-route"&&b.sent.pairingCode==="temporary-code","editing locks identity to selected environment");b.connectionsManaged("save-route","");check(keys.findChild(view,"connectionPairingCode").text==="","pairing code cleared on success");view.rename(b.connectionSettings.environments[0]);keys.findChild(view,"connectionLabel").text="Renamed";view.save();check(b.sent.action==="rename"&&b.sent.label==="Renamed","rename action");b.connectionsManaged("rename","");phase=3
  }else if(phase===3){var toggle=keys.findChild(view,"connectionEnabled_node");toggle.checked=false;toggle.toggled();check(b.sent.action==="enabled"&&b.sent.enabled===false,"disable keeps saved connection");view.remove(b.connectionSettings.environments[0],null);phase=4
  }else if(phase===4){check(b.sent.action==="enabled","remove does not execute before confirmation");keys.findChild(view,"connectionConfirmRemove").clicked();check(b.sent.action==="remove"&&b.sent.confirmed===true,"explicit removal confirmation");b.connectionsManaged("remove","");b.selectedEnvironment="local";b.connectionAccess=Object.assign({},b.connectionAccess,{environment:"local"});b.localBackend={owner:"stopped",running:false,canControl:true};b.backendStart={busy:true,error:""};phase=5
  }else if(phase===5){check(!keys.findChild(view,"connectionStartBackend").enabled,"start remains disabled during activation");b.backendStart={busy:false,error:"service_start_failed"};phase=6
  }else if(phase===6){check(keys.findChild(view,"connectionBackendError").visible&&keys.findChild(view,"connectionBackendError").text.indexOf("service_start_failed")>=0,"lifecycle failure is visible in Connections");check(keys.findChild(view,"connectionStartBackend").enabled,"failed startup can be retried");keys.findChild(view,"connectionStartBackend").clicked();check(b.sent.backend==="start","retry dispatches lifecycle start");console.log("CONNECTIONS_QML_PASS: native catalog load, SSH/pairing forms, identity scope, secret clearing, rename, disable and removal confirmation");Qt.quit()}
 }}
}
