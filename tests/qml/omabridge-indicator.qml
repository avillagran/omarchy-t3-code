import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property int settings:0
 readonly property string guest:"b21d3e5f5c3ee737761cbbc2bc2a427715b430ac3e7ec391e0a20a7c42d9e888"
 property var pending:[]
 function check(ok,msg){if(!ok){console.error("OMABRIDGE_INDICATOR_QML_FAIL "+msg);Qt.exit(1)}}
 function snapshot(){return {available:true,version:"0.1.0",device:{id:"a".repeat(64),name:"alpha"},peers:[{id:"c".repeat(64),name:"node",services:[],remoteServices:["t3"],public:[],admin:false,managed:false}],
  pending:root.pending,awaiting:[],published:{enabled:true,active:true,services:["t3"],sshAvailable:false,t3Available:true},relay:{mode:"public"},relayHost:{},gateways:[],gatewayZones:[],error:""}}
 QtObject{id:b
  property var sent:[];property int seq:0
  property var viewState:({omabridgeIndicator:true})
  signal omabridgeManageFinished(string requestId,string text,int exitCode)
  function requestOmaBridgeManage(request){seq++;var id="r"+seq;sent=sent.concat([request]);var text=JSON.stringify(root.snapshot());Qt.callLater(function(){b.omabridgeManageFinished(id,text,0)});return id}
  function copyText(value){}
  function rememberView(values){}
  function last(){return sent[sent.length-1]}
 }
 FloatingWindow{visible:true;implicitWidth:900;implicitHeight:900
  Test.T3OmaBridgeIndicator{id:indicator;bridge:b;x:10;y:10}
  Test.T3OmaBridgeQuick{id:view;bridge:b;width:420;x:60;y:10;onSettingsRequested:root.settings++}
 }
 TestCase{id:keys;when:false}
 Timer{interval:200;repeat:true;running:true;onTriggered:{
  if(view.manage.busy || indicator.busy)return
  if(phase===0){
   check(b.sent.length>=1&&b.sent[0].action==="read","reads this device's OmaBridge on load");
   check(indicator.pendingCount===0,"no badge without pending requests");
   var status=view.statusText();check(status.indexOf("T3")>=0&&status.indexOf("1")>=0,"status names published services and peers: "+status);
   check(!!keys.findChild(view,"omabridgeInviteButton")&&!!keys.findChild(view,"omabridgePairButton")&&!!keys.findChild(view,"omabridgeRelayButton"),"the full OmaBridge page is in the bar popup");
   root.pending=[{id:root.guest,name:"charlie",services:["t3"],expires:1792390449}];indicator.refresh();phase++
  }else if(phase===1){
   check(indicator.pendingCount===1,"badge for a pending request");
   var n=b.sent.length;keys.findChild(indicator,"omabridgeIndicator").pressed(Qt.LeftButton);check(indicator.popupOpen&&b.sent.length===n+1&&b.last().action==="read","opening refreshes");view.manage.refresh();phase++
  }else if(phase===2){
   keys.findChild(view,"omabridgeApprove_"+root.guest).clicked();check(b.last().action==="read","approval needs a second step");
   check(view.manage.dialogHeight>0,"an open dialog reports its height to the popup");
   // What closing the bar popup does to its page (the popup window cannot be shown offscreen).
   indicator.close();view.manage.forget();phase++
  }else if(phase===3){
   check(!indicator.popupOpen&&view.manage.dialogHeight===0,"closing the popup closes its dialogs");
   keys.findChild(view,"omabridgeIndicatorSettings").clicked();check(root.settings===1,"settings hand-off");
   console.log("OMABRIDGE_INDICATOR_QML_PASS: device read, status, pending badge, the full page from the bar, dialog sizing and closing, settings hand-off");Qt.quit()
  }
 }}
}
