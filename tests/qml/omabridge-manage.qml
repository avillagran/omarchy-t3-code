import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
// The OmaBridge page ported from the Omarchy Control Panel, against a fake
// `omabridge manage` answered through ot3's backend transport.
ShellRoot {
 id:root;property int phase:0;property int wait:0
 readonly property string me:"4b4980a5435d5c3714e458c7c416035070c027fbef47e426c2077c32f677952d"
 readonly property string vps:"9f1e3c1a4b5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f7"
 readonly property string guest:"b21d3e5f5c3ee737761cbbc2bc2a427715b430ac3e7ec391e0a20a7c42d9e888"
 property var forced:null
 property bool consent:false;property bool allHostsPending:false
 property var features:["control-socket","forward-through-server"]
 function check(ok,msg){if(!ok){console.error("OMABRIDGE_MANAGE_QML_FAIL "+msg);Qt.exit(1)}}
 function snapshot(){return {available:true,version:"0.1.0",identity:true,device:{id:me,name:"alpha"},
  peers:[{id:vps,name:"vps",services:["t3"],remoteServices:[],public:["t3"],admin:false,managed:true,bridgeConsent:root.consent,remoteBridgeConsent:true},{id:guest,name:"charlie",services:["ssh"],remoteServices:["t3","ssh"],public:[],admin:false,managed:false,bridgeConsent:false,remoteBridgeConsent:false}],features:root.features,
  pending:[{id:guest,name:"charlie",services:["t3"],expires:1792390449,allHosts:root.allHostsPending}],awaiting:[],
  published:{enabled:true,active:true,external:false,stale:false,services:["t3","ssh"],sshAvailable:true,t3Available:true},
  relay:{mode:"public",url:null},relayHost:{enabled:false,url:null,running:false,error:null,suggested:null},gateways:[],gatewayZones:[],error:""}}
 function reply(req){
  if(forced){var f=forced;forced=null;return f}
  var doc=snapshot()
  if(req.action==="invite")doc.created={path:"/tmp/omabridge-invite.json",expires:1792390449,services:["t3"],manage:false,qrImage:"data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnIHZpZXdCb3g9JzAgMCAxIDEnLz4="}
  if(req.action==="bridge-consent-set"){root.consent=req.allowed;doc=snapshot()}
  if(req.action==="remote"){var r=snapshot();r.device={id:vps,name:"vps"};r.peers=[{id:me,name:"alpha",services:[],remoteServices:["t3"],public:[],admin:true,managed:false}];r.pending=root.allHostsPending?[{id:guest,name:"charlie",services:["t3"],expires:1792390449,allHosts:true}]:[];r.gateways=[{peer:me,name:"alpha",service:"t3",url:"https://gw.example.net:55944/",running:true,error:""}];doc.remote=r}
  if(req.action==="gateway-check")doc.gatewayCheck={reachable:true,sameEnvironment:true}
  return {doc:doc,code:0}
 }
 QtObject{id:b
  property var sent:[];property int seq:0;property string copied:""
  signal omabridgeManageFinished(string requestId,string text,int exitCode)
  function requestOmaBridgeManage(request){seq++;var id="r"+seq;sent=sent.concat([JSON.parse(JSON.stringify(request))]);var answer=root.reply(request);Qt.callLater(function(){b.omabridgeManageFinished(id,JSON.stringify(answer.doc),answer.code)});return id}
  function copyText(value){copied=value}
  function last(){return sent[sent.length-1]}
 }
 FloatingWindow{visible:true;implicitWidth:900;implicitHeight:1000
  Flickable{anchors.fill:parent;contentHeight:view.implicitHeight
   Test.T3OmaBridgeManage{id:view;width:760;x:20;bridge:b}
  }
 }
 TestCase{id:keys;when:false}
 function find(name){return keys.findChild(view,name)}
 // Repeater rows inside the remote dialog's ScrollView are reachable only as visual children.
 function row(item,name){if(item.objectName===name)return item;for(var i=0;i<item.children.length;i++){var hit=row(item.children[i],name);if(hit)return hit}return null}
 Timer{interval:200;repeat:true;running:true;onTriggered:{
  if(view.busy)return
  if(phase===0){
   check(b.sent.length>=1&&b.sent[0].action==="read"&&b.sent[0].confirmed===undefined,"reads on load, unconfirmed");
   check(view.pageState==="ready"&&find("omabridgeDeviceName").text==="alpha","this device shown");
   check(view.pendingCount===1,"pending requests counted for the bar badge");
   check(!find("omabridgeCopySsh_"+root.guest).visible,"no SSH command without connect-stdio");
   find("omabridgeApprove_"+root.guest).clicked();check(b.last().action==="read","approval waits for confirmation");phase++
  }else if(phase===1){
   check(find("omabridgeConfirmDialog").opened,"confirmation shown");find("omabridgeConfirm").clicked();
   var p=b.last();check(p.action==="approve"&&p.peer===root.guest&&p.confirmed===true,"confirmed approval: "+JSON.stringify(p));phase++
  }else if(phase===2){
   check(!find("omabridgeConfirmDialog").opened,"confirmation closes after success");
   root.features=["connect-stdio"];view.refresh();phase=20
  }else if(phase===20){
   check(!find("omabridgeCopySsh_"+root.vps).visible,"no SSH command for a device that does not grant SSH");
   var copy=find("omabridgeCopySsh_"+root.guest);check(copy.visible,"SSH command for a device that grants SSH");copy.clicked();
   check(b.copied==="ssh -o ProxyCommand='omabridge connect --peer charlie --service ssh' charlie","copies the ProxyCommand: "+b.copied);
   find("omabridgeManage_"+root.vps).clicked();phase=3
  }else if(phase===3){
   var p=b.last();check(p.action==="remote"&&p.peer===root.vps&&p.command.action==="read"&&p.confirmed===undefined,"managing a device reads it first, unconfirmed");
   check(find("omabridgeRemoteDialog").opened&&!!view.remoteSnapshot&&view.remoteSnapshot.device.id===root.vps,"remote device shown");
   check(view.dialogHeight>0,"an open dialog reports its height to small hosts");
   find("omabridgeGatewayRecheck").clicked();phase++
  }else if(phase===4){
   var p=b.last();check(p.action==="gateway-check"&&p.url==="https://gw.example.net:55944/","this device's public address checked");
   check(view.gatewayCheck&&view.gatewayCheck.sameEnvironment===true,"check result kept");
   find("omabridgeRemoteClose").clicked();phase++
  }else if(phase===5){
   find("omabridgeInviteButton").clicked();phase++
  }else if(phase===6){
   find("omabridgeInviteCreate").clicked();phase++
  }else if(phase===7){
   var invite=b.sent.filter(function(r){return r.action==="invite"})[0];check(invite&&invite.confirmed===true&&JSON.stringify(invite.services)==='["t3"]',"confirmed invitation sharing T3");
   check(find("omabridgeInviteQrImage").visible&&find("omabridgeInvitePath").text==="/tmp/omabridge-invite.json","invitation shown once");
   check(b.last().action==="read","the secret is replaced by a fresh read right away");
   view.forget();check(view.created===null,"closing forgets the invitation");
   root.forced={doc:{error:"omabridge_peer_unreachable"},code:1};find("omabridgeUnpublishButton").clicked();phase++
  }else if(phase===8){
   find("omabridgeConfirm").clicked();phase++
  }else if(phase===9){
   check(b.last().action==="unpublish"&&b.last().confirmed===true,"confirmed unpublish");
   check(view.errorCode==="omabridge_peer_unreachable"&&view.errorText(view.errorCode,true).indexOf("omabridge")!==0,"readable error: "+view.errorText(view.errorCode,true));
   find("omabridgeConfirmCancel").clicked();root.forced={doc:{error:"omabridge_outdated"},code:1};view.refresh();phase++
  }else if(phase===10){
   check(view.pageState==="outdated"&&find("omabridgeStateTitle").visible,"an OmaBridge without manage says so");
   root.features=["connect-stdio"];view.refresh();phase=30
  }else if(phase===30){
   // Without the feature nothing about all hosts appears or is sent.
   check(view.pageState==="ready"&&!find("omabridgeBridgeConsent_"+root.vps).visible&&!find("omabridgePeerRemoteConsent_"+root.vps).visible,"no consent controls without the feature");
   view.openInvite();check(!find("omabridgeInviteAllHosts").visible,"no all-hosts option without the feature");
   find("omabridgeInviteCreate").clicked();phase++
  }else if(phase===31){
   var plain=b.sent.filter(function(r){return r.action==="invite"}).pop();check(plain.allHosts===undefined,"a plain invitation sends no flag: "+JSON.stringify(plain));
   find("omabridgeInviteClose").clicked();root.features=["all-hosts-invite-v1"];root.allHostsPending=true;view.refresh();phase++
  }else if(phase===32){
   view.openInvite();var all=find("omabridgeInviteAllHosts");
   check(all.visible&&!all.checked&&!find("omabridgeInviteAllHostsWarning").visible,"offered unchecked, warning hidden");
   all.checked=true;check(find("omabridgeInviteAllHostsWarning").visible,"warning follows the box");
   find("omabridgeInviteT3").checked=false;check(!all.visible,"all hosts needs T3");find("omabridgeInviteSsh").checked=true;
   find("omabridgeInviteCreate").clicked();phase++
  }else if(phase===33){
   var sshOnly=b.sent.filter(function(r){return r.action==="invite"}).pop();check(sshOnly.allHosts===undefined&&JSON.stringify(sshOnly.services)==='["ssh"]',"no flag once T3 is off: "+JSON.stringify(sshOnly));
   find("omabridgeInviteClose").clicked();view.openInvite();
   check(!find("omabridgeInviteAllHosts").checked,"reset when reopened");
   find("omabridgeInviteAllHosts").checked=true;find("omabridgeInviteCreate").clicked();phase++
  }else if(phase===34){
   var allHosts=b.sent.filter(function(r){return r.action==="invite"}).pop();check(allHosts.allHosts===true&&allHosts.confirmed===true&&allHosts.services.indexOf("t3")>=0,"opted-in invitation: "+JSON.stringify(allHosts));
   find("omabridgeInviteClose").clicked();
   check(find("omabridgePendingAllHosts_"+root.guest).visible,"pending row says all hosts");
   find("omabridgeApprove_"+root.guest).clicked();check(view.confirmRequest.detail.indexOf(view.t("omabridgeInviteAllHostsWarning"))>=0,"approval warns: "+view.confirmRequest.detail);
   find("omabridgeConfirmCancel").clicked();
   check(find("omabridgePeerRemoteConsent_"+root.vps).visible&&!find("omabridgePeerRemoteConsent_"+root.guest).visible,"hosts that consented to this device are shown");
   check(!find("omabridgeBridgeConsent_"+root.guest).visible,"no consent without a t3 grant");
   var box=find("omabridgeBridgeConsent_"+root.vps);check(box.visible&&!box.checked,"consent state from OmaBridge");
   var sentBefore=b.sent.length;box.click();
   check(b.sent.length===sentBefore&&!box.checked&&find("omabridgeConfirmDialog").opened,"a click only asks");
   find("omabridgeConfirmCancel").clicked();check(!box.checked,"cancelling leaves it as it was");
   box.click();find("omabridgeConfirm").clicked();phase++
  }else if(phase===35){
   var on=b.last();check(on.action==="bridge-consent-set"&&on.peer===root.vps&&on.allowed===true&&on.confirmed===true,"confirmed consent: "+JSON.stringify(on));
   var box=find("omabridgeBridgeConsent_"+root.vps);check(box.checked,"answer moves the toggle");
   box.click();check(find("omabridgeConfirmDialog").opened&&view.confirmRequest.values.allowed===false,"withdrawal asks too");find("omabridgeConfirm").clicked();phase++
  }else if(phase===36){
   var off=b.last();check(off.action==="bridge-consent-set"&&off.allowed===false&&off.confirmed===true,"confirmed withdrawal: "+JSON.stringify(off));
   check(!find("omabridgeBridgeConsent_"+root.vps).checked,"toggle follows the withdrawal");
   find("omabridgeManage_"+root.vps).clicked();phase++
  }else if(phase===37){
   var dialog=find("omabridgeRemoteDialog").contentItem;
   check(row(dialog,"omabridgeRemotePending_"+root.guest).text.indexOf(view.t("omabridgeAllHostsRequest"))>=0&&!row(dialog,"omabridgeRemoteApprove_"+root.guest).visible,"a managing device sees all hosts and cannot approve it");
   find("omabridgeRemoteClose").clicked();
   console.log("OMABRIDGE_MANAGE_QML_PASS: unconfirmed reads, confirmed approve/invite/unpublish, managed device and its public address check, shown-once invitation, readable errors, outdated OmaBridge, all-hosts invitation and bridge consent");Qt.quit()
  }
 }}
}
