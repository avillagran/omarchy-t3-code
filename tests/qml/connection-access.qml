import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("CONNECTION_ACCESS_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property string selectedEnvironment:"node"
  property var granted:["orchestration:read","terminal:read","terminal:operate","access:read","access:write","relay:read","relay:write"]
  property var connectionAccess:({busy:false,error:"",environment:"node",data:{endpoint:"http://192.0.2.1:3773",descriptor:{label:"NODE",serverVersion:"1.2"},session:{authenticated:true,permissions:granted,auth:{policy:"remote-reachable"}},clients:[{sessionId:"self",current:true,connected:true,client:{label:"ot3"}},{sessionId:"phone",current:false,client:{label:"Phone"},permissions:["orchestration:read"]}],pairingLinks:[{id:"link",label:"Tablet",expiresAt:"2030-01-01T00:00:00Z"}],connect:{linked:true,publishAgentActivity:false,managedTunnelActive:true,holdWebhooksWhileOffline:true}},created:null})
  property var sent:null
  signal connectionAccessFinished(string action,string error)
  function requestConnectionAccess(env,action,values){sent=Object.assign({environment:env,action:action},values || {})}
  function clearConnectionCredential(){connectionAccess=Object.assign({},connectionAccess,{created:null})}
  function copyText(value){sent={copy:value}}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:980;implicitHeight:1000
  Rectangle{id:canvas;anchors.fill:parent;color:"#141414"
   Test.T3ConnectionAccess{id:view;width:760;anchors.centerIn:parent;bridge:b;environment:b.selectedEnvironment}
  }
 }
 TestCase{id:keys;when:false}
 Timer{interval:150;running:true;repeat:true;onTriggered:{
  if(phase===0){check(b.sent.environment==="node"&&b.sent.action==="read","reads selected environment");view.expanded=true;phase++
  }else if(phase===1){check(!keys.findChild(view,"connectionRevokeClient_self").enabled,"current client cannot revoke itself");keys.findChild(view,"connectionCreateLink").clicked();phase++
  }else if(phase===2){check(view.selectedScopes.indexOf("access:write")<0&&view.selectedScopes.indexOf("access:read")<0&&view.selectedScopes.indexOf("relay:write")<0,"native defaults exclude administration");view.toggleScope("terminal:read",false);check(view.selectedScopes.indexOf("terminal:operate")<0,"unchecking read also unchecks terminal operation");view.toggleScope("terminal:operate",true);check(view.selectedScopes.indexOf("terminal:read")>=0,"terminal operation includes read");keys.findChild(view,"connectionNewLinkLabel").text="Phone";keys.findChild(view,"connectionIssueLink").clicked();check(b.sent.action==="create-link"&&b.sent.environment==="node"&&b.sent.label==="Phone"&&b.sent.scopes.indexOf("terminal:read")>=0,"native pairing payload scoped to selected server");
   b.connectionAccess=Object.assign({},b.connectionAccess,{created:{id:"new",credential:"ONE-TIME",pairingUrl:"http://192.0.2.1:3773/pair#token=ONE-TIME",expiresAt:"2030-01-01T00:00:00Z"}});phase++
  }else if(phase===3){check(keys.findChild(view,"connectionCreatedCode").text==="ONE-TIME","new credential shown explicitly");check(!keys.findChild(view,"connectionPairingQr").visible,"no QR when unavailable");keys.findChild(view,"connectionCopyCode").clicked();check(b.sent.copy.indexOf("#token=ONE-TIME")>0,"copies full native pairing URL");b.selectedEnvironment="other";phase++
  }else if(phase===4){check(!b.connectionAccess.created,"changing environment clears secret");check(b.sent.environment==="other"&&b.sent.action==="read","changing environment requests new permissions");check(!view.can("access:write"),"previous server permissions never leak");b.selectedEnvironment="node";view.expanded=true;phase++
  }else if(phase===5){view.revoke("revoke-client",b.connectionAccess.data.clients[1]);check(b.sent.action==="read","revoke waits for confirmation");phase++
  }else if(phase===6){keys.findChild(view,"connectionConfirmRevoke").clicked();check(b.sent.action==="revoke-client"&&b.sent.sessionId==="phone"&&b.sent.confirmed&&b.sent.environment==="node","confirmed revocation scoped to current server");b.connectionAccessFinished("revoke-client","");check(b.sent.action==="read","successful revoke refreshes native clients");phase=7;canvas.grabToImage(function(image){image.saveToFile("/tmp/ot3-connection-access.png");console.log("CONNECTION_ACCESS_QML_PASS: selected-server scope, native grant defaults, terminal dependency, pairing code/link, secret clearing, current-client guard and confirmed revocation");Qt.quit()})}
 }}
}
