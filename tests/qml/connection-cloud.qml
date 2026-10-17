import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("CONNECTION_CLOUD_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var connectionCloud:({environment:"local",requestId:"one",busy:false,error:"",status:{authenticated:false,linked:false,desired:false}})
  property var sent:null
  function requestConnectionCloud(environment,action,values){sent=Object.assign({environment:environment,action:action},values || {})}
  function cancelConnectionCloud(id){sent={cancel:id}}
  function confirmConnectionCloudInstall(id,approved){sent={install:id,approved:approved}}
  function copyText(value){sent={copy:value}}
 }
 FloatingWindow{visible:true;implicitWidth:1100;implicitHeight:950;Test.T3ConnectionCloud{id:view;width:800;anchors.centerIn:parent;bridge:b;environment:"local"}}
 TestCase{id:keys;when:false}
 Timer{interval:150;repeat:true;running:true;onTriggered:{
  if(phase===0){check(b.sent.action==="read","reads native account state");keys.findChild(view,"connectionCloudLogin").clicked();check(b.sent.action==="login"&&b.sent.environment==="local","native login dispatched");b.connectionCloud=Object.assign({},b.connectionCloud,{busy:true,stage:"authorization",device:{verificationUrl:"https://example.invalid/device",userCode:"TEST-CODE",expiresInMinutes:5}});phase++
  }else if(phase===1){check(keys.findChild(view,"connectionCloudDeviceCode").text==="TEST-CODE","native device code visible");keys.findChild(view,"connectionCloudCopyCode").clicked();check(b.sent.copy==="TEST-CODE","copy only user code");keys.findChild(view,"connectionCloudCancel").clicked();check(b.sent.cancel==="one","cancels only active authorization job");b.connectionCloud=Object.assign({},b.connectionCloud,{busy:false,device:null,status:{authenticated:true,linked:true,desired:true}});phase++
  }else if(phase===2){keys.findChild(view,"connectionCloudUnlink").clicked();check(b.sent.cancel==="one","unlink waits for confirmation");phase++
  }else if(phase===3){keys.findChild(view,"connectionCloudConfirm").clicked();check(b.sent.action==="unlink"&&b.sent.confirmed,"explicit native account unlink");view.environment="node";phase++
  }else if(phase===4){check(!view.local&&!view.busy,"remote environment cannot mutate local account");console.log("CONNECTION_CLOUD_QML_PASS: native sign-in, device approval, scoped cancellation, confirmed unlink and remote isolation");Qt.quit()}
 }}
}
