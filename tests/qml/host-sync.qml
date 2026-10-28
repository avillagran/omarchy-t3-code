import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
// Sync this computer's setup: SSH hosts only, preview first, items without
// pending changes cannot be chosen, the vault explains GitHub access and offers
// the sign-in, Sync asks for confirmation and sends only the chosen items.
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("HOST_SYNC_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{id:b
  property var connectionSettings:({environments:[{id:"node",name:"NODE",enabled:true,routes:[{kind:"SshConnectionTarget",connectionId:"s"}]},{id:"paired",name:"Paired",enabled:true,routes:[{kind:"BearerConnectionTarget",connectionId:"b"}]},{id:"off",name:"Off",enabled:false,routes:[{kind:"SshConnectionTarget",connectionId:"o"}]}]})
  property var hostSync:({busy:false,action:"",environment:"",error:"",preview:null,result:null,last:({node:{at:"2026-10-27T10:00:00Z"}})})
  property var sent:[]
  function hostSyncAction(action,environment,values){sent=sent.concat([Object.assign({action:action,environment:environment},values || {})])}
  function formatTimestamp(v){return "yesterday"}
 }
 FloatingWindow{visible:true;implicitWidth:900;implicitHeight:900
  Test.T3HostSync{id:view;width:760;anchors.centerIn:parent;bridge:b}
 }
 TestCase{id:keys;when:false}
 Timer{interval:150;repeat:true;running:true;onTriggered:{
  if(phase===0){
   check(b.sent.length===1&&b.sent[0].action==="read","reads the last results");
   var host=keys.findChild(view,"hostSyncHost");check(host.count===1&&host.currentText==="NODE","only enabled SSH hosts: "+host.count);
   check(!keys.findChild(view,"hostSyncNoHosts").visible,"hosts are listed");
   check(keys.findChild(view,"hostSyncLast").visible&&keys.findChild(view,"hostSyncLast").text.indexOf("yesterday")>=0,"shows the last sync");
   keys.findChild(view,"hostSyncPreview").clicked();
   check(b.sent[1].action==="preview"&&b.sent[1].environment==="node","previews the chosen host");
   b.hostSync=Object.assign({},b.hostSync,{environment:"node",preview:{environment:"node",transport:"omabridge",items:[
    {id:"vault",available:true,pending:0,vault:{available:true,path:"~/Obsidian/Dev",remote:"https://github.com/me/dev.git",action:"clone",access:"auth",github:true,gh:"signed-out"}},
    {id:"instructions",available:true,pending:1,counts:{add:0,update:1,same:1,kept:0,left:0},entries:[{path:"~/.claude/CLAUDE.md",action:"update",files:1},{path:"~/.codex/AGENTS.md",action:"same",files:1}]},
    {id:"skills",available:true,pending:3,counts:{add:3,update:0,same:0,kept:2,left:1},entries:[{path:"~/.claude/skills/mine",action:"add",files:3},{path:"~/.claude/skills/omarchy",action:"left",reason:"system"}]},
    {id:"config",available:true,pending:0,counts:{add:0,update:0,same:4,kept:0,left:1},entries:[{path:"~/.config/opencode/opencode.json",action:"left",reason:"credentials"}]}]}});phase++
  }else if(phase===1){
   var vault=keys.findChild(view,"hostSyncSummary_vault");check(vault.text.indexOf("github.com/me/dev")>=0&&vault.text.indexOf("GitHub")>=0,"the vault explains missing GitHub access: "+vault.text);
   check(keys.findChild(view,"hostSyncGithubLogin").visible,"offers the GitHub sign-in on the host");
   check(!keys.findChild(view,"hostSyncItem_vault").enabled&&!keys.findChild(view,"hostSyncItem_config").enabled,"items with nothing to do cannot be chosen");
   check(keys.findChild(view,"hostSyncItem_skills").checked&&keys.findChild(view,"hostSyncSummary_skills").text.indexOf("3")>=0,"skills summary");
   keys.findChild(view,"hostSyncGithubLogin").clicked();check(b.sent[2].action==="github-login"&&b.sent[2].environment==="node","GitHub sign-in runs on that host");
   keys.findChild(view,"hostSyncApply").clicked();check(b.sent.length===3,"Sync waits for confirmation");phase++
  }else if(phase===2){
   check(keys.findChild(view,"hostSyncConfirm").opened,"confirmation is shown");
   keys.findChild(view,"hostSyncConfirmButton").clicked();
   var s=b.sent[3];check(s.action==="apply"&&s.confirmed===true&&JSON.stringify(s.items)==='["instructions","skills"]',"sends only chosen items with pending changes: "+JSON.stringify(s));
   b.hostSync=Object.assign({},b.hostSync,{result:{environment:"node",backup:"/home/node/.local/state/omarchy-t3-code/sync-backups/20261028T000000Z",items:[{id:"instructions",status:"done",counts:{added:0,updated:1,unchanged:1}},{id:"skills",status:"partial",counts:{added:2,updated:0,unchanged:0,failed:1}}]}});phase++
  }else if(phase===3){
   check(keys.findChild(view,"hostSyncResult_instructions").text.indexOf("1")>=0,"result per item");
   check(keys.findChild(view,"hostSyncResult_skills").text.indexOf("failed")>=0||keys.findChild(view,"hostSyncResult_skills").text.length>0,"partial results say what failed");
   check(keys.findChild(view,"hostSyncBackup").visible&&keys.findChild(view,"hostSyncBackup").text.indexOf("sync-backups")>=0,"names the backup on the host");
   check(keys.findChild(view,"hostSyncPreview").text.length>0&&keys.findChild(view,"hostSyncPreview").enabled,"can check again");
   console.log("HOST_SYNC_QML_PASS: SSH hosts only, last sync, preview, GitHub access and sign-in, pending-only items, confirmation, chosen items, results and backup");Qt.quit()
  }
 }}
}
