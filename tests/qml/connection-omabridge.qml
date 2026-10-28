import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property string automaticStatus:""
 readonly property string peer:"f3fc84495ad12fcc7bcc5490eae2a4e98da1499b2668b7a05648efe899c17910"
 function check(ok,msg){if(!ok){console.error("CONNECTION_OMABRIDGE_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var connectionOmaBridge:({busy:false,error:"",data:{revision:"one",available:true,identity:true,stateDir:"/tmp/x",error:"",routes:{},peers:[{id:root.peer,name:"devbox",remoteServices:["t3","ssh"]},{id:"a".repeat(64),name:"egress-only",remoteServices:["egress"]}]}})
  property var sent:null
  signal connectionOmaBridgeFinished(string action,string error)
  function requestConnectionOmaBridge(action,values){sent=Object.assign({action:action},values || {})}
 }
 FloatingWindow{visible:true;implicitWidth:900;implicitHeight:600
  Test.T3ConnectionOmaBridge{id:view;width:760;anchors.centerIn:parent;bridge:b
   environments:[{id:"devbox",name:"NODE",routes:[{kind:"BearerConnectionTarget",connectionId:"url"}]},{id:"ssh-only",name:"SSH",routes:[{kind:"SshConnectionTarget",connectionId:"s"}]},{id:"cloud",name:"Cloud",routes:[{kind:"RelayConnectionTarget",connectionId:"r"}]}]}
 }
 TestCase{id:keys;when:false}
 Timer{interval:150;repeat:true;running:true;onTriggered:{
  if(phase===0){
   check(b.sent.action==="read","reads OmaBridge snapshot");
   check(!!keys.findChild(view,"omabridgePeer_devbox")&&!!keys.findChild(view,"omabridgePeer_ssh-only")&&!keys.findChild(view,"omabridgePeer_cloud"),"pairing-link and SSH connections are offered, T3 Connect is not");
   var ssh=keys.findChild(view,"omabridgePeer_ssh-only");check(ssh.count===3&&ssh.model[2].indexOf("SSH")>0,"SSH profiles use the peer ssh service");
   var select=keys.findChild(view,"omabridgePeer_devbox");check(select.count===3&&select.model[2].indexOf("T3")>0&&select.currentIndex===0,"pairing links use the peer t3 service, automatic by default");
   check(keys.findChild(view,"omabridgeDefault").checked,"automatic routing is the default");
   automaticStatus=keys.findChild(view,"omabridgeStatus_devbox").text;check(automaticStatus.length>0&&automaticStatus.indexOf("devbox")<0,"automatic status before a peer answers");
   select.currentIndex=2;select.activated(2);check(b.sent.action==="read","route change waits for confirmation");phase++
  }else if(phase===1){
   keys.findChild(view,"omabridgeConfirm").clicked();
   check(b.sent.action==="set"&&b.sent.environmentId==="devbox"&&b.sent.peer===root.peer&&b.sent.service==="t3"&&b.sent.mode==="prefer"&&b.sent.confirmed===true&&b.sent.label===undefined,"confirmed pinned route payload");
   b.connectionOmaBridge=Object.assign({},b.connectionOmaBridge,{data:Object.assign({},b.connectionOmaBridge.data,{routes:{devbox:{peer:root.peer,service:"t3",mode:"prefer"}}})});b.connectionOmaBridgeFinished("set","");phase++
  }else if(phase===2){
   var only=keys.findChild(view,"omabridgeOnly_devbox");check(only.enabled&&!only.checked,"strict mode available once pinned");only.checked=true;only.toggled();phase++
  }else if(phase===3){
   keys.findChild(view,"omabridgeConfirm").clicked();check(b.sent.action==="set"&&b.sent.mode==="only","strict mode payload");b.connectionOmaBridgeFinished("set","");phase++
  }else if(phase===4){
   var select=keys.findChild(view,"omabridgePeer_devbox");check(select.currentIndex===2,"selector reflects the pinned route");select.currentIndex=0;select.activated(0);phase++
  }else if(phase===5){
   keys.findChild(view,"omabridgeConfirm").clicked();check(b.sent.action==="clear"&&b.sent.environmentId==="devbox"&&b.sent.peer===undefined,"back to automatic payload");
   b.connectionOmaBridgeFinished("clear","");phase++
  }else if(phase===6){
   var ssh=keys.findChild(view,"omabridgePeer_ssh-only");ssh.currentIndex=1;ssh.activated(1);phase++
  }else if(phase===7){
   keys.findChild(view,"omabridgeConfirm").clicked();check(b.sent.action==="set"&&b.sent.environmentId==="ssh-only"&&b.sent.mode==="off"&&b.sent.peer===undefined,"saved-route-only payload");
   b.connectionOmaBridge=Object.assign({},b.connectionOmaBridge,{data:Object.assign({},b.connectionOmaBridge.data,{routes:{"ssh-only":{mode:"off"}},active:{devbox:{peer:root.peer,service:"t3",auto:true}}})});b.connectionOmaBridgeFinished("set","");phase++
  }else if(phase===8){
   check(keys.findChild(view,"omabridgePeer_ssh-only").currentIndex===1&&!keys.findChild(view,"omabridgeOnly_ssh-only").enabled,"saved-route-only selection");
   var status=keys.findChild(view,"omabridgeStatus_devbox").text;check(status!==automaticStatus&&status.indexOf("devbox · f3fc8449")>0,"live OmaBridge status names the peer");
   var toggle=keys.findChild(view,"omabridgeDefault");toggle.checked=false;toggle.toggled();phase++
  }else if(phase===9){
   keys.findChild(view,"omabridgeConfirm").clicked();check(b.sent.action==="default"&&b.sent.value==="off"&&b.sent.confirmed===true,"global default payload");
   b.connectionOmaBridgeFinished("default","");phase++
  }else if(phase===10){
   // Automatic sharing: on by default, one row per peer sharing T3, egress-only peers hidden.
   var share=keys.findChild(view,"omabridgeShare");check(share.checked,"automatic sharing is on by default");
   check(!!keys.findChild(view,"omabridgeShareStatus_"+root.peer)&&!keys.findChild(view,"omabridgeShareStatus_"+"a".repeat(64)),"only peers sharing T3 are listed");
   check(!keys.findChild(view,"omabridgeShareRetry_"+root.peer).visible,"no retry while pending");
   b.connectionOmaBridge=Object.assign({},b.connectionOmaBridge,{data:Object.assign({},b.connectionOmaBridge.data,{sharing:{enabled:true,peers:(function(){var p={};p[root.peer]={state:"shared",port:47329};return p})()}})});phase++
  }else if(phase===11){
   check(keys.findChild(view,"omabridgeShareStatus_"+root.peer).text.indexOf("127.0.0.1:47329")>=0,"shared status names the fixed forward");
   b.connectionOmaBridge=Object.assign({},b.connectionOmaBridge,{data:Object.assign({},b.connectionOmaBridge.data,{sharing:{enabled:true,peers:(function(){var p={};p[root.peer]={state:"skipped"};return p})()}})});phase++
  }else if(phase===12){
   var retry=keys.findChild(view,"omabridgeShareRetry_"+root.peer);check(retry.visible,"a removed share can be added again");retry.clicked();
   check(b.sent.action==="share-retry"&&b.sent.peer===root.peer&&b.sent.confirmed===true,"retry payload");
   var share=keys.findChild(view,"omabridgeShare");share.checked=false;share.toggled();check(b.sent.action==="share-retry","turning sharing off waits for confirmation");phase++
  }else if(phase===13){
   keys.findChild(view,"omabridgeConfirm").clicked();check(b.sent.action==="share-enabled"&&b.sent.enabled===false&&b.sent.confirmed===true&&b.sent.label===undefined,"sharing switch payload");
   console.log("CONNECTION_OMABRIDGE_QML_PASS: automatic default, eligible connections and services, confirmed pin/strict/automatic/saved-only/default, live status, automatic sharing");Qt.quit()
  }
 }}
}
