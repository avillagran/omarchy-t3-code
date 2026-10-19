import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 readonly property string me:"4b4980a5435d5c3714e458c7c416035070c027fbef47e426c2077c32f677952d"
 readonly property string guest:"b21d3e5f5c3ee737761cbbc2bc2a427715b430ac3e7ec391e0a20a7c42d9e888"
 function check(ok,msg){if(!ok){console.error("OMABRIDGE_DEVICE_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var connectionOmaBridge:({busy:false,error:"",created:null,paired:null,data:{revision:"r",available:true,identity:false,device:null,peers:[],pending:[],awaiting:[],routes:{}}})
  property var sent:null;property int cleared:0;property string copied:""
  signal connectionOmaBridgeFinished(string action,string error)
  function requestConnectionOmaBridge(action,values){sent=Object.assign({action:action},values || {})}
  function clearOmaBridgeInvite(){cleared++;connectionOmaBridge=Object.assign({},connectionOmaBridge,{created:null})}
  function copyText(value){copied=value}
  function update(data){connectionOmaBridge=Object.assign({},connectionOmaBridge,{data:Object.assign({},connectionOmaBridge.data,data)})}
 }
 FloatingWindow{visible:true;implicitWidth:900;implicitHeight:700
  Test.T3OmaBridgeDevice{id:view;width:760;anchors.centerIn:parent;bridge:b}
 }
 TestCase{id:keys;when:false}
 function confirm(){keys.findChild(view,"omabridgeDeviceConfirm").clicked()}
 Timer{interval:150;repeat:true;running:true;onTriggered:{
  if(phase===0){
   var create=keys.findChild(view,"omabridgeCreateIdentity");check(create.visible,"identity can be created");check(!keys.findChild(view,"omabridgeInvite").enabled,"no invitation without an identity");
   create.clicked();check(b.sent===null,"identity creation waits for confirmation");phase++
  }else if(phase===1){
   confirm();check(b.sent.action==="identity-create"&&b.sent.confirmed===true,"confirmed identity payload");
   b.update({identity:true,device:{id:root.me,name:"alpha"},pending:[{id:root.guest,name:"charlie",services:["t3"],expires:1792390449}],awaiting:[{id:"0b0a3e64-3a6b-4a4a-9c38-1f4f1c1e1a11",name:"node",expires:1792390449}],peers:[{id:root.guest,name:"charlie",services:["t3"],remoteServices:["ssh"]}]});
   b.connectionOmaBridgeFinished("identity-create","");phase++
  }else if(phase===2){
   check(keys.findChild(view,"omabridgeDeviceName").text==="alpha"&&!keys.findChild(view,"omabridgeCreateIdentity").visible,"identity shown");
   var services=keys.findChild(view,"omabridgePeerServices_"+root.guest).text;check(services.indexOf("SSH")>=0&&services.indexOf("T3")>=0,"peer services in both directions: "+services);
   keys.findChild(view,"omabridgeApprove_"+root.guest).clicked();check(b.sent.action==="identity-create","approval waits for confirmation");phase++
  }else if(phase===3){
   confirm();check(b.sent.action==="approve"&&b.sent.peer===root.guest&&b.sent.expires===1792390449&&b.sent.confirmed===true,"confirmed approval payload");b.connectionOmaBridgeFinished("approve","");phase++
  }else if(phase===4){
   keys.findChild(view,"omabridgeRevoke_"+root.guest).clicked();confirm();check(b.sent.action==="revoke"&&b.sent.peer===root.guest&&b.sent.confirmed===true,"confirmed revocation payload");b.connectionOmaBridgeFinished("revoke","");phase++
  }else if(phase===5){
   keys.findChild(view,"omabridgeRetry_0b0a3e64-3a6b-4a4a-9c38-1f4f1c1e1a11").clicked();check(b.sent.action==="pair-retry"&&b.sent.id==="0b0a3e64-3a6b-4a4a-9c38-1f4f1c1e1a11","check again payload");
   view.pairFile("file:///tmp/my%20invite.json");confirm();check(b.sent.action==="pair"&&b.sent.path==="/tmp/my invite.json"&&b.sent.confirmed===true,"pair from a chosen file");b.connectionOmaBridgeFinished("pair","");phase++
  }else if(phase===6){
   keys.findChild(view,"omabridgeInvite").clicked();check(b.cleared===1,"a new invitation starts clean");phase++
  }else if(phase===7){
   keys.findChild(view,"omabridgeInviteCreate").clicked();check(b.sent.action==="invite"&&JSON.stringify(b.sent.services)==='["t3"]'&&b.sent.qr===false,"default invitation shares T3 only");
   var ssh=keys.findChild(view,"omabridgeInviteSsh");ssh.toggle();var qr=keys.findChild(view,"omabridgeInviteQr");qr.toggle();var t3=keys.findChild(view,"omabridgeInviteT3");t3.toggle();
   keys.findChild(view,"omabridgeInviteCreate").clicked();check(JSON.stringify(b.sent.services)==='["ssh"]'&&b.sent.qr===true,"SSH-only invitation with QR");
   b.connectionOmaBridge=Object.assign({},b.connectionOmaBridge,{created:{path:"/tmp/omabridge-invite.json",expires:1792390449,services:["ssh"],qrImage:"data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnIHZpZXdCb3g9JzAgMCAxIDEnLz4="}});b.connectionOmaBridgeFinished("invite","");phase++
  }else if(phase===8){
   check(keys.findChild(view,"omabridgeInviteQrImage").visible&&keys.findChild(view,"omabridgeInvitePath").text==="/tmp/omabridge-invite.json"&&!keys.findChild(view,"omabridgeInviteCreate").visible,"created invitation shown once");
   keys.findChild(view,"omabridgeInviteDiscard").clicked();check(b.sent.action==="invite-discard"&&b.sent.path==="/tmp/omabridge-invite.json","delete the invitation file");
   keys.findChild(view,"omabridgeInviteDialog").close();phase++
  }else if(phase===9){
   check(b.cleared===2&&b.connectionOmaBridge.created===null,"closing forgets the invitation secret");
   check(view.errorText("omabridge_not_published").length>0&&view.errorText("omabridge_not_published").indexOf("omabridge_")<0,"readable publish error");
   console.log("OMABRIDGE_DEVICE_QML_PASS: identity creation, confirmed approve/revoke/pair, check again, invitation services/QR, shown-once secret cleared on close");Qt.quit()
  }
 }}
}
