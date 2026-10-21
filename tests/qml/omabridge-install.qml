import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
// Set up a new host with an agent: the chat runs on the chosen host, starts in
// that host's scratch project, the invitation is created first (confirmed by
// the dialog), moved to that host, and the prompt names only its path there.
ShellRoot {
 id:root;property int phase:0
 property string promptError:""
 function check(ok,msg){if(!ok){console.error("OMABRIDGE_INSTALL_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var state:({environments:[{id:"local",name:"T3 Code",state:"ready"},{id:"node",name:"node",state:"ready"},{id:"down",name:"down",state:"connecting"}],
   projects:[{id:"repo",environmentId:"local",title:"Repo",workspaceRoot:"/home/u/repo"},{id:"scratch",environmentId:"local",title:"No project",workspaceRoot:"/home/u/.t3/scratch"},
    {id:"node-repo",environmentId:"node",title:"Repo",workspaceRoot:"/srv/repo"},{id:"node-scratch",environmentId:"node",title:"No project",workspaceRoot:"/srv/chats/",isScratch:false}]})
  property var catalogs:({node:{scratchWorkspaceRoot:"/srv/chats"}})
  property var sent:[];property var installs:[];property var chat:null;property int seq:0;property var asked:[]
  signal omabridgeManageFinished(string requestId,string text,int exitCode)
  signal omabridgeInstallAnswer(string requestId,var result,string error)
  function ensureCatalog(env){asked=asked.concat([env])}
  function requestOmaBridgeManage(request){seq++;var id="m"+seq;sent=sent.concat([request]);var doc={available:true,device:{id:"a".repeat(64),name:"alpha"},published:{active:true,enabled:true,services:["t3","ssh"]}}
   if(request.action==="invite")doc.created={path:"/home/u/Downloads/omabridge-invite-20261020-120000.json",expires:1792390449,services:request.services}
   Qt.callLater(function(){b.omabridgeManageFinished(id,JSON.stringify(doc),0)});return id}
  function requestOmaBridgeInstall(op,values){seq++;var id="i"+seq;installs=installs.concat([Object.assign({op:op},values)])
   var result=op==="omabridge-invite-handoff"?{path:values.environment==="local"?values.invitation:"/srv/home/.cache/omabridge/omabridge-invite-20261020-120000.json"}:(root.promptError?null:{prompt:"PROMPT"})
   Qt.callLater(function(){b.omabridgeInstallAnswer(id,result,op==="omabridge-install-prompt"?root.promptError:"")});return id}
  function startPromptedChat(environment,projectId,text){chat={environment:environment,projectId:projectId,text:text};return true}
 }
 FloatingWindow{visible:true;implicitWidth:900;implicitHeight:900
  Item{anchors.fill:parent;Test.T3OmaBridgeInstall{id:dialog;bridge:b}}
 }
 TestCase{id:keys;when:false}
 function find(name){return keys.findChild(dialog.contentItem,name)}
 function last(op){return b.installs.filter(function(r){return r.op===op}).slice(-1)[0]}
 Timer{interval:200;repeat:true;running:true;onTriggered:{
  if(dialog.busy)return
  if(phase===0){
   check(dialog.environments.length===2&&dialog.environments[dialog.defaultEnvironment].id==="local","connected hosts, this device first");
   dialog.begin(1);phase++
  }else if(phase===1){
   check(dialog.opened&&b.sent[0].action==="read"&&b.asked.indexOf("node")>=0,"reads this device and that host's settings");
   check(dialog.environment==="node"&&find("omabridgeInstallRunsOn").text==="node","runs on the chosen host");
   check(dialog.scratch&&dialog.scratch.id==="node-scratch","that host's scratch project, from its settings");
   check(!find("omabridgeInstallStart").enabled,"needs the new host");
   find("omabridgeInstallHost").text="vps";root.promptError="invalid_host";find("omabridgeInstallStart").clicked();phase++
  }else if(phase===2){
   var invite=b.sent[b.sent.length-1];check(invite.action==="invite"&&invite.confirmed===true&&invite.manage===true&&invite.ttl===3600&&JSON.stringify(invite.services)==='["t3"]',"one-use invitation that asks to manage: "+JSON.stringify(invite));
   var hand=last("omabridge-invite-handoff");check(hand&&hand.environment==="node"&&hand.invitation==="/home/u/Downloads/omabridge-invite-20261020-120000.json","the invitation moves to that host");
   var p=last("omabridge-install-prompt");check(p.host==="vps"&&p.runsOn==="node"&&p.invitation==="/srv/home/.cache/omabridge/omabridge-invite-20261020-120000.json"&&p.device==="alpha","prompt names its path there: "+JSON.stringify(p));
   check(find("omabridgeInstallError").visible&&b.chat===null&&dialog.opened,"a refused prompt opens no chat");
   root.promptError="";find("omabridgeInstallStart").clicked();phase++
  }else if(phase===3){
   check(b.chat&&b.chat.environment==="node"&&b.chat.projectId==="node-scratch"&&b.chat.text==="PROMPT","clean chat on that host with the prompt");
   check(!dialog.opened&&dialog.invitation==="","dialog closes and forgets the invitation path");
   dialog.begin(0);phase++
  }else if(phase===4){
   check(dialog.environment==="local"&&dialog.scratch.id==="scratch","this device's scratch project by its folder");
   find("omabridgeInstallHost").text="vps";find("omabridgeInstallManage").toggle();find("omabridgeInstallSsh").toggle();find("omabridgeInstallStart").clicked();phase++
  }else if(phase===5){
   var invite=b.sent[b.sent.length-1];check(invite.manage===false&&JSON.stringify(invite.services)==='["t3","ssh"]',"management off, SSH shared on request");
   check(last("omabridge-install-prompt").runsOn===""&&b.chat.environment==="local"&&b.chat.projectId==="scratch","here the invitation stays put and the chat opens here");
   console.log("OMABRIDGE_INSTALL_QML_PASS: connected hosts, chat on the chosen host in its scratch project, confirmed managing invitation moved there, prompt from its path, refused prompt, local run, options");Qt.quit()
  }
 }}
}
