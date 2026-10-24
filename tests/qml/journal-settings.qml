import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
// Preferences > Memory/Obsidian: hosts are listed (offline is not "no vault"),
// Set up on this computer clones the detected remote here, each wizard step keeps
// its own state, and nothing reads as configured while a step is pending.
ShellRoot {
 id:root
 function check(ok,message){if(!ok){console.error("JOURNAL_SETTINGS_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 readonly property var github:"https://github.com/me/dev-vault.git"
 QtObject {
  id:bridge
  property int reads:0
  property int discoveries:0
  property var actions:[]
  property var vault:({busy:false,error:"",state:{config:{enabled:false,path:"/home/me/Obsidian/Dev",remote:null,sync:{pullOnOpen:true,pullOnThreadStart:true,pushOnSettle:true},agentInstructions:true},local:{state:"missing"},status:{git:false},steps:{remote:false,clone:false,instructions:false,obsidian:true},configured:false,obsidian:{installed:true,how:"native",install:{command:"omarchy-pkg-add obsidian",url:"https://obsidian.md/download"}},sync:{}}})
  property var vaultHosts:({busy:false,loaded:true,error:"",
   hosts:[{environment:"node-id",name:"node",state:"online",vaults:[{path:"~/Obsidian/Dev",remote:root.github,canonical:"github.com/me/dev-vault"}]},{environment:"lab-id",name:"lab",state:"offline",error:"ssh_unavailable",vaults:[]}],
   vaults:[{id:"github.com/me/dev-vault",remote:root.github,kind:"github-private",shareable:true,hosts:[{environment:"node-id",name:"node",path:"~/Obsidian/Dev",remote:root.github}]}]})
  property var vaultAction:({busy:false,error:"",action:"",result:null})
  function requestVault(){reads++}
  function discoverVaults(){discoveries++}
  function runVaultAction(values){actions=actions.concat([values]);vaultAction=({busy:true,error:"",action:values.action,result:null})}
 }
 FloatingWindow {id:win;visible:true;implicitWidth:900;implicitHeight:1400
  Flickable{anchors.fill:parent;contentHeight:page.implicitHeight
   Test.T3JournalSettings{id:page;width:parent.width;bridge:bridge}}
 }
 TestCase { id:keys;parent:win.contentItem;when:false }
 function find(name){return keys.findChild(page,name)}
 Timer { interval:250;running:true;onTriggered:{
  check(bridge.reads===1&&bridge.discoveries===1,"opening the page reads this computer and the hosts");
  check(find("vaultLocalState").text===Test.T3I18n.tr("Not set up on this computer yet."),"local state: "+find("vaultLocalState").text);
  check(find("vaultHost_lab-id").text.indexOf(Test.T3I18n.tr("offline: unknown, not checked"))>=0,"an offline host is unknown, not empty");
  check(find("vaultHost_node-id").text.indexOf(root.github)>=0,"node's vault and remote are listed");
  check(!find("vaultSource").visible,"one vault: no choice to make");
  check(find("vaultSuggestion").visible,"the page proposes copying node's setup first");
  check(find("vaultCopyFrom").text===Test.T3I18n.tr("Copy setup from {host}",{host:"node"}),"button: "+find("vaultCopyFrom").text);
  check(!find("vaultOtherWays").visible,"manual options wait behind Other options");
  keys.mouseClick(find("vaultShowOther"));check(find("vaultOtherWays").visible,"Other options shows them");
  keys.mouseClick(find("vaultCopyFrom"));
  var sent=bridge.actions[0];
  check(sent.action==="setup"&&sent.url===root.github&&sent.environment==="node-id"&&sent.path==="/home/me/Obsidian/Dev","Set up on this computer: "+JSON.stringify(sent));
  // Missing sign-in on this computer: explained, the rest pending.
  bridge.vaultAction=({busy:false,error:"",action:"setup",result:{setup:{complete:false,steps:[{id:"folder",state:"done"},{id:"access",state:"failed",error:"vault_auth_missing",help:{provider:"github",steps:["gh auth login","gh auth setup-git"]}}]}}});
  check(find("vaultSetupStep_access").text.indexOf(Test.T3I18n.tr("This computer is not signed in to this remote."))>=0,"auth step: "+find("vaultSetupStep_access").text);
  check(find("vaultSetupComplete").text===Test.T3I18n.tr("Some steps are still pending."),"not complete");
  // Cloned and enabled, Obsidian missing: still not configured, with install help.
  var s=JSON.parse(JSON.stringify(bridge.vault.state));
  s.config.enabled=true;s.config.remote={kind:"github-private",url:root.github};s.local={state:"clone",remote:root.github};s.status={git:true,branch:"main",ahead:1,behind:0,changes:2,conflict:false};s.steps={remote:true,clone:true,instructions:true,obsidian:false};s.obsidian.installed=false;
  bridge.vault=({busy:false,error:"",state:s});
  check(find("vaultLocalState").text===Test.T3I18n.tr("Not set up on this computer yet."),"pending Obsidian is not configured");
  check(find("vaultStep_obsidian").text.indexOf(Test.T3I18n.tr("pending"))>=0,"Obsidian step pending");
  check(find("vaultObsidianMissing").visible&&find("vaultObsidianMissing").text.indexOf("omarchy-pkg-add obsidian")>=0,"install guidance");
  check(!find("vaultOpenObsidian").visible,"no Open in Obsidian without Obsidian");
  check(find("vaultSyncState").text.indexOf("main")>=0,"sync state shown");
  // Everything done.
  s=JSON.parse(JSON.stringify(s));s.steps.obsidian=true;s.obsidian.installed=true;s.configured=true;
  bridge.vault=({busy:false,error:"",state:s});bridge.vaultAction=({busy:false,error:"",action:"",result:null});
  check(find("vaultLocalState").text===Test.T3I18n.tr("Ready: {path} stays in sync with {remote}.",{path:"/home/me/Obsidian/Dev",remote:root.github}),"ready: "+find("vaultLocalState").text);
  check(!find("vaultSuggestion").visible,"no second setup offer");
  keys.wait(100);keys.mouseClick(find("vaultOpenObsidian"));
  check(bridge.actions[bridge.actions.length-1].action==="open","Open in Obsidian");
  // Two distinct vaults: the source must be chosen.
  var h=JSON.parse(JSON.stringify(bridge.vaultHosts));h.vaults.push({id:"gitlab.com/me/work",remote:"https://gitlab.com/me/work.git",shareable:true,hosts:[{environment:"lab-id",name:"lab",path:"~/Obsidian/Dev",remote:"https://gitlab.com/me/work.git"}]});
  bridge.vaultHosts=h;
  check(find("vaultSource").visible,"several vaults: choose one");
  // A clone here and a host whose remote cannot be read: adopt the clone.
  s=JSON.parse(JSON.stringify(s));s.configured=false;s.config.enabled=false;s.config.remote=null;s.local={state:"clone",remote:root.github,canonical:"github.com/me/dev-vault"};
  bridge.vault=({busy:false,error:"",state:s});
  bridge.vaultHosts=({busy:false,loaded:true,error:"",hosts:[{environment:"node-id",name:"node",state:"online",vaults:[{path:"~/Obsidian/Dev",git:true,remote:null,remoteError:"rpc_rejected: x"}]}],vaults:[{id:"x",remote:null,shareable:false,hosts:[]}]});
  check(find("vaultAdopt").visible&&!find("vaultCopyFrom").visible,"adopt the local clone");
  check(find("vaultSuggestionDetail").text.indexOf(Test.T3I18n.tr("{host} also has a vault at {path}, but its remote could not be read.",{host:"node",path:"~/Obsidian/Dev"}))>=0,"unreadable remote explained");
  check(find("vaultHost_node-id").text.indexOf(Test.T3I18n.tr("remote not readable"))>=0,"host row: "+find("vaultHost_node-id").text);
  bridge.vaultAction=({busy:false,error:"",action:"",result:null});
  keys.wait(100);keys.mouseClick(find("vaultAdopt"));var adopt=bridge.actions[bridge.actions.length-1];
  check(adopt.action==="setup"&&adopt.url===root.github,"adopt: "+JSON.stringify(adopt));
  console.log("JOURNAL_SETTINGS_QML_PASS: hosts, setup here, pending steps, Obsidian and source choice");Qt.quit()
 }}
}
