import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
// "Install on another device with your AI": the invitation is created first
// (confirmed by the dialog), the prompt names only its path, and a new chat
// opens with that prompt typed in.
ShellRoot {
 id:root;property int phase:0
 property string promptError:""
 function check(ok,msg){if(!ok){console.error("OMABRIDGE_INSTALL_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var state:({projects:[{id:"repo",environmentId:"local",title:"Repo",isScratch:false},{id:"scratch",environmentId:"local",title:"Scratch",isScratch:true},{id:"far",environmentId:"remote",title:"Far",isScratch:true}]})
  property var sent:[];property var promptRequest:null;property var chat:null;property int seq:0
  signal omabridgeManageFinished(string requestId,string text,int exitCode)
  signal omabridgeInstallPromptReady(string requestId,string prompt,string error)
  function requestOmaBridgeManage(request){seq++;var id="m"+seq;sent=sent.concat([request]);var doc={available:true,device:{id:"a".repeat(64),name:"alpha"},published:{active:true,enabled:true,services:["t3","ssh"]}}
   if(request.action==="invite")doc.created={path:"/home/u/Downloads/omabridge-invite-20261020-120000.json",expires:1792390449,services:request.services}
   Qt.callLater(function(){b.omabridgeManageFinished(id,JSON.stringify(doc),0)});return id}
  function requestOmaBridgeInstallPrompt(values){seq++;var id="p"+seq;promptRequest=values;Qt.callLater(function(){b.omabridgeInstallPromptReady(id,root.promptError?"":"PROMPT",root.promptError)});return id}
  function startPromptedChat(environment,projectId,text){chat={environment:environment,projectId:projectId,text:text};return true}
 }
 FloatingWindow{visible:true;implicitWidth:900;implicitHeight:900
  Item{anchors.fill:parent;Test.T3OmaBridgeInstall{id:dialog;bridge:b}}
 }
 TestCase{id:keys;when:false}
 function find(name){return keys.findChild(dialog.contentItem,name)}
 Timer{interval:200;repeat:true;running:true;onTriggered:{
  if(dialog.busy)return
  if(phase===0){dialog.begin();phase++
  }else if(phase===1){
   check(dialog.opened&&b.sent.length===1&&b.sent[0].action==="read","reads this device first");
   check(dialog.projects.length===2&&dialog.projects[dialog.defaultProject].id==="scratch","local projects only, scratch by default");
   check(!find("omabridgeInstallStart").enabled,"needs a host");
   find("omabridgeInstallHost").text="vps";find("omabridgeInstallProfile_ot3").clicked();check(dialog.profile==="ot3","profile chosen");
   find("omabridgeInstallProfile_headless").clicked();
   root.promptError="invalid_host";find("omabridgeInstallStart").clicked();phase++
  }else if(phase===2){
   var invite=b.sent[b.sent.length-1];check(invite.action==="invite"&&invite.confirmed===true&&invite.manage===true&&invite.ttl===3600&&invite.qr===false&&JSON.stringify(invite.services)==='["t3"]',"one-use invitation that asks to manage, sharing T3 only: "+JSON.stringify(invite));
   var p=b.promptRequest;check(p.host==="vps"&&p.profile==="headless"&&p.manage===true&&p.invitation==="/home/u/Downloads/omabridge-invite-20261020-120000.json"&&p.device==="alpha","prompt built from the invitation path: "+JSON.stringify(p));
   check(find("omabridgeInstallError").visible&&b.chat===null&&dialog.opened,"a refused prompt opens no chat");
   root.promptError="";find("omabridgeInstallManage").toggle();find("omabridgeInstallSsh").toggle();find("omabridgeInstallStart").clicked();phase++
  }else if(phase===3){
   var invite=b.sent[b.sent.length-1];check(invite.manage===false&&JSON.stringify(invite.services)==='["t3","ssh"]',"management off, SSH shared on request");
   check(b.chat&&b.chat.environment==="local"&&b.chat.projectId==="scratch"&&b.chat.text==="PROMPT","new chat with the prompt in the scratch project");
   check(!dialog.opened&&dialog.invitation==="","dialog closes and forgets the invitation path");
   console.log("OMABRIDGE_INSTALL_QML_PASS: device read, local projects with scratch default, confirmed managing invitation, prompt from its path only, refused prompt, SSH and management options, prefilled chat");Qt.quit()
  }
 }}
}
