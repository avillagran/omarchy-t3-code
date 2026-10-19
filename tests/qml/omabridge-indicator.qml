import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property int settings:0;property int before:0
 readonly property string guest:"b21d3e5f5c3ee737761cbbc2bc2a427715b430ac3e7ec391e0a20a7c42d9e888"
 function check(ok,msg){if(!ok){console.error("OMABRIDGE_INDICATOR_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var connectionOmaBridge:({busy:false,error:"",created:null,paired:null,data:{available:true,device:{id:"a".repeat(64),name:"alpha"},peers:[{id:"c".repeat(64),name:"node",services:[],remoteServices:["t3"]}],pending:[],awaiting:[],published:{enabled:true,active:true,services:["t3"],sshAvailable:false}}})
  property var sent:[];property int cleared:0
  property var viewState:({omabridgeIndicator:true})
  signal connectionOmaBridgeFinished(string action,string error)
  function requestConnectionOmaBridge(action,values){sent=sent.concat([Object.assign({action:action},values || {})])}
  function clearOmaBridgeInvite(){cleared++;connectionOmaBridge=Object.assign({},connectionOmaBridge,{created:null})}
  function copyText(value){}
  function rememberView(values){}
  function last(){return sent[sent.length-1]}
 }
 FloatingWindow{visible:true;implicitWidth:900;implicitHeight:700
  Test.T3OmaBridgeIndicator{id:indicator;bridge:b;x:10;y:10}
  Test.T3OmaBridgeQuick{id:view;bridge:b;width:420;x:60;y:10;onSettingsRequested:root.settings++}
 }
 TestCase{id:keys;when:false}
 Timer{interval:150;repeat:true;running:true;onTriggered:{
  if(phase===0){
   check(b.sent.length>=1&&b.sent[0].action==="read","reads the OmaBridge snapshot on load");
   check(indicator.pendingCount===0,"no badge without pending requests");
   var status=keys.findChild(view,"omabridgeIndicatorStatus").text;check(status.indexOf("T3")>=0&&status.indexOf("1")>=0,"status names published services and peers: "+status);
   check(!keys.findChild(view,"omabridgeIndicatorPreference").visible,"the bar toggle stays in Preferences");
   check(keys.findChild(view,"omabridgePublish").visible&&keys.findChild(view,"omabridgeInvite").enabled&&keys.findChild(view,"omabridgePairFile").visible,"publish, invite and pair are available from the bar");
   b.connectionOmaBridge=Object.assign({},b.connectionOmaBridge,{data:Object.assign({},b.connectionOmaBridge.data,{pending:[{id:root.guest,name:"charlie",services:["t3"],expires:1792390449}],awaiting:[{id:"0b0a3e64-3a6b-4a4a-9c38-1f4f1c1e1a11",name:"node",expires:1792390449}]})});phase++
  }else if(phase===1){
   check(indicator.pendingCount===1,"badge for a pending request");
   var n=b.sent.length;keys.findChild(indicator,"omabridgeIndicator").pressed(Qt.LeftButton);check(indicator.popupOpen&&b.sent.length===n+1&&b.last().action==="read","opening refreshes");
   indicator.close();check(!indicator.popupOpen,"popup closes");phase++
  }else if(phase===2){
   keys.findChild(view,"omabridgeApprove_"+root.guest).clicked();check(b.last().action==="read","approval needs a second step");phase++
  }else if(phase===3){
   check(view.device.dialogHeight>0,"an open dialog reports its height to the host");
   keys.findChild(view,"omabridgeDeviceConfirm").clicked();var p=b.last();check(p.action==="approve"&&p.peer===root.guest&&p.expires===1792390449&&p.confirmed===true,"confirmed approval payload");
   b.connectionOmaBridgeFinished("approve","");phase++
  }else if(phase===4){
   check(view.device.dialogHeight===0,"confirmation closes after approval");
   keys.findChild(view,"omabridgeRetry_0b0a3e64-3a6b-4a4a-9c38-1f4f1c1e1a11").clicked();check(b.last().action==="pair-retry","check again payload");
   keys.findChild(view,"omabridgeRevoke_"+"c".repeat(64)).clicked();keys.findChild(view,"omabridgeDeviceConfirm").clicked();check(b.last().action==="revoke"&&b.last().confirmed===true,"revoke from the bar");
   b.connectionOmaBridgeFinished("revoke","");phase++
  }else if(phase===5){
   // Closing the bar popup closes an open invitation and forgets its secret.
      var c=b.cleared;keys.findChild(view,"omabridgeInvite").clicked();check(b.cleared===c+1,"a new invitation starts clean");
   keys.findChild(view,"omabridgeInviteCreate").clicked();check(b.last().action==="invite"&&JSON.stringify(b.last().services)==='["t3"]',"invite from the bar");phase++
  }else if(phase===6){
   // What closing the bar popup does (the popup window itself cannot be shown offscreen).
   check(view.device.dialogHeight>0,"the invitation is still open");root.before=b.cleared;view.device.closeDialogs();phase++
  }else if(phase===7){
   check(b.cleared===root.before+1&&view.device.dialogHeight===0,"closing the dialogs forgets the invitation");phase++
  }else if(phase===8){
   keys.findChild(view,"omabridgeIndicatorSettings").clicked();check(root.settings===1,"settings hand-off");
   console.log("OMABRIDGE_INDICATOR_QML_PASS: snapshot refresh, status, pending badge, full management from the bar (approve, check again, revoke, invite), dialog sizing, settings hand-off");Qt.quit()
  }
 }}
}
