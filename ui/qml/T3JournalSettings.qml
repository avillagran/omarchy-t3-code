import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
// Preferences → Memory/Obsidian (docs/JOURNAL_VAULT.md). "This computer" is the
// device ot3 runs on; hosts are only read. Nothing here is called configured
// while a step is pending.
ColumnLayout {
 id:root
 objectName:"journalSettings"
 property var bridge:null
 property string query:""
 readonly property color fg:Color.popups.text
 readonly property var state:bridge&&bridge.vault?bridge.vault.state||null:null
 readonly property var hosts:bridge&&bridge.vaultHosts?bridge.vaultHosts:({hosts:[],vaults:[]})
 readonly property var action:bridge&&bridge.vaultAction?bridge.vaultAction:({})
 readonly property var config:state?state.config:({})
 readonly property var local:state?state.local:({})
 readonly property var sync:state?state.status||({}):({})
 readonly property var steps:state?state.steps:({})
 readonly property bool cloned:local.state==="clone"
 readonly property bool configured:!!state&&state.configured===true
 readonly property var shareable:(hosts.vaults||[]).filter(function(v){return v.shareable})
 property string chosen:""
 readonly property var source:shareable.find(function(v){return v.id===root.chosen})||shareable[0]||null
 property string folder:""
 readonly property string folderPath:folder.trim()||(config.path||"~/Obsidian/Dev")
 property string manualUrl:""
 property string githubName:"dev-vault"
 // What the page proposes first: copy the setup of a host that already uses a vault,
 // or adopt a clone already on this computer. Everything else is under Other options.
 property bool showOther:false
 readonly property string sourceHosts:source?source.hosts.map(function(h){return h.name}).join(", "):""
 readonly property bool localMatches:!!source&&cloned&&local.canonical===source.id
 readonly property var unreadable:{var all=root.hosts.hosts||[];for(var i=0;i<all.length;i++){var v=(all[i].vaults||[]).find(function(x){return !x.remote&&x.git&&x.remoteError!=="no-origin"});if(v)return {name:all[i].name,path:v.path}}return null}
 readonly property var identity:state&&state.identity?state.identity:({})
 readonly property var suggestedIdentity:identity.suggested||({})
 property string identityName:""
 property string identityEmail:""
 readonly property string suggestion:!state||configured?"":config.enabled&&cloned&&identity.set===false?"identity":source?"copy":cloned&&!!local.remote?"clone":""
 readonly property bool settingUp:action.busy&&action.action==="setup"
 function copyFromHost(){var h=root.source.hosts[0];root.setup({url:root.source.remote,hostRemote:h.remote,environment:h.environment})}
 readonly property var setupResult:action.action==="setup"&&action.result?action.result.setup:null
 spacing:Style.space(24)
 Component.onCompleted:if(bridge){bridge.requestVault();bridge.discoverVaults()}
 function matches(t){return !query.trim()||T3I18n.tr(t).toLocaleLowerCase().indexOf(query.trim().toLocaleLowerCase())>=0}
 function message(code){
  if(!code)return ""
  var m=/^vault_path_occupied(: (.*))?$/.exec(code)
  if(m)return T3I18n.tr(m[2]==="other-remote"?"This folder is a clone of another repository. Choose another folder.":m[2]==="not-directory"?"There is a file at this path. Choose another folder.":"This folder already has other files. Nothing was changed; choose another folder.")
  var known={vault_invalid_remote:"Use a git remote URL: https://…, ssh://…, user@host:path or a folder path.",vault_auth_missing:"This computer is not signed in to this remote.",vault_remote_unreachable:"The remote cannot be reached from this computer.",vault_remote_missing:"The remote repository was not found.",vault_remote_needs_ssh:"This vault is a repository on that host. Connect to it over SSH or OmaBridge to clone it.",vault_url_has_credentials:"Remove the password or token from the URL. This computer's own sign-in is used.",vault_conflict:"Your changes conflict with the remote. Resolve them in the vault folder; local commits were kept.",vault_git_identity_missing:"The vault has no git name and email for commits yet.",vault_git_missing:"Install git on this computer.",vault_busy:"Another vault operation is running.",vault_invalid_path:"Choose a folder inside your home, not the home itself.",vault_obsidian_missing:"Obsidian is not installed on this computer.",vault_not_cloned:"The vault is not cloned on this computer yet.",vault_gh_missing:"Install the GitHub CLI (gh) to create the repository.",vault_instructions_malformed:"An agent instruction file has an unfinished vault block. Fix it by hand.",disconnected:"T3 Code is not connected."}
  return known[code]?T3I18n.tr(known[code]):T3I18n.error(code)
 }
 function when(ms){if(!ms)return T3I18n.tr("never");var d=new Date(ms);return d.toLocaleString(Qt.locale(),Locale.ShortFormat)}
 function setup(values){bridge.runVaultAction(Object.assign({action:"setup",path:root.folderPath},values))}
 function saveSync(key,value){var patch={sync:{}};patch.sync[key]=value;bridge.runVaultAction({action:"save",patch:patch})}
 component Caption: Text{Layout.fillWidth:true;wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.fg;opacity:0.75;font.pixelSize:Style.font.caption}
 component Field: Controls.TextField{id:field;Layout.fillWidth:true;implicitHeight:Style.space(34);color:root.fg;font.pixelSize:Style.font.caption;selectByMouse:true;leftPadding:Style.space(10);rightPadding:Style.space(10)
  background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025);border.width:1;border.color:field.activeFocus?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.1)}}

 // ---------------------------------------------------------------- suggestion
 T3PreferenceSection {objectName:"vaultSuggestion";Layout.fillWidth:true;title:"Recommended";icon:"star";visible:!!root.suggestion&&!root.query.trim()
  body:Component {ColumnLayout {width:parent.width;spacing:Style.space(8)
   Item{implicitHeight:Style.space(4)}
   Text{objectName:"vaultSuggestionTitle";Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);wrapMode:Text.Wrap;textFormat:Text.PlainText;color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.DemiBold
    text:root.suggestion==="identity"?T3I18n.tr("Commits in the vault need a git name and email."):root.suggestion==="copy"?T3I18n.tr("{host} already uses a development vault. Set up this computer the same way.",{host:root.sourceHosts}):T3I18n.tr("There is a vault clone at {path}, not set up in ot3 yet.",{path:root.config.path})}
   Caption{objectName:"vaultSuggestionDetail";Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12)
    text:root.suggestion==="identity"?(root.suggestedIdentity.email?T3I18n.tr(root.suggestedIdentity.from==="vault"?"This is the last author of the vault. It is set for this vault only; nothing global changes.":"Your projects on this computer commit with this identity. It is set for this vault only; nothing global changes."):T3I18n.tr("It is set for this vault only; nothing global changes."))
     :root.suggestion==="copy"?(T3I18n.tr("Vault: {remote}",{remote:(root.source||{}).remote})+"\n"+(root.localMatches?T3I18n.tr("Your clone at {path} is the same vault: it is reused, nothing is downloaded again.",{path:root.config.path}):T3I18n.tr("It is cloned to {path} with this computer's own sign-in; nothing is copied from {host}.",{path:root.folderPath,host:root.sourceHosts}))+"\n"+T3I18n.tr("Then the agent instructions, this computer's row in Machines.md and Obsidian are set up here."))
     :(T3I18n.tr("Vault: {remote}",{remote:root.local.remote})+(root.unreadable?"\n"+T3I18n.tr("{host} also has a vault at {path}, but its remote could not be read.",{host:root.unreadable.name,path:root.unreadable.path}):"")+"\n"+T3I18n.tr("Then the agent instructions, this computer's row in Machines.md and Obsidian are set up here."))}
   RowLayout{visible:root.suggestion==="identity";Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);spacing:Style.space(8)
    Field{objectName:"vaultIdentityName";placeholderText:T3I18n.tr("Name");text:root.identityName||root.suggestedIdentity.name||"";onTextEdited:root.identityName=text}
    Field{objectName:"vaultIdentityEmail";placeholderText:T3I18n.tr("Email");text:root.identityEmail||root.suggestedIdentity.email||"";onTextEdited:root.identityEmail=text}
   }
   Flow{Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);Layout.bottomMargin:Style.space(10);spacing:Style.space(8)
    T3PickerButton{objectName:"vaultUseIdentity";visible:root.suggestion==="identity";primary:true;implicitHeight:Style.space(30);enabled:!root.action.busy&&!!(root.identityName||root.suggestedIdentity.name)&&!!(root.identityEmail||root.suggestedIdentity.email)
     text:root.action.busy&&root.action.action==="identity"?T3I18n.tr("Syncing…"):T3I18n.tr("Use for this vault and sync");onClicked:root.bridge.runVaultAction({action:"identity",name:(root.identityName||root.suggestedIdentity.name).trim(),email:(root.identityEmail||root.suggestedIdentity.email).trim()})}
    T3PickerButton{objectName:"vaultCopyFrom";visible:root.suggestion==="copy";primary:true;implicitHeight:Style.space(30);enabled:!root.action.busy;text:root.settingUp?T3I18n.tr("Setting up…"):T3I18n.tr("Copy setup from {host}",{host:root.sourceHosts});onClicked:root.copyFromHost()}
    T3PickerButton{objectName:"vaultAdopt";visible:root.suggestion==="clone";primary:true;implicitHeight:Style.space(30);enabled:!root.action.busy;text:root.settingUp?T3I18n.tr("Setting up…"):T3I18n.tr("Use this clone");onClicked:root.setup({path:root.config.path,url:root.local.remote})}
    T3PickerButton{objectName:"vaultShowOther";visible:root.suggestion!=="identity";primary:false;implicitHeight:Style.space(30);text:T3I18n.tr(root.showOther?"Hide other options":"Other options");onClicked:root.showOther=!root.showOther}
   }
  }}
 }
 // The wizard's steps, each with its own state.
 T3PreferenceSection {objectName:"vaultProgress";Layout.fillWidth:true;title:"Setup progress";icon:"circle-check";visible:!!root.setupResult||root.settingUp||!!root.action.error&&(root.action.action==="setup"||root.action.action==="create")
  body:Component {ColumnLayout {width:parent.width;spacing:Style.space(8)
   Item{implicitHeight:Style.space(4)}
   Caption{visible:root.settingUp;Layout.leftMargin:Style.space(12);text:T3I18n.tr("Setting up…")}
   Repeater{model:root.setupResult?root.setupResult.steps:[]
    ColumnLayout{required property var modelData;Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);spacing:Style.space(4)
     Caption{objectName:"vaultSetupStep_"+modelData.id;opacity:1;color:modelData.state==="failed"?Color.urgent:root.fg
      text:(modelData.state==="done"?"✓ ":modelData.state==="failed"?"✕ ":"○ ")+T3I18n.tr(({folder:"Folder",access:"Access with this computer's sign-in",clone:"Clone",identity:"Git name and email for commits",instructions:"Agent instructions",device:"Device registered in Machines.md",obsidian:"Obsidian"})[modelData.id]||modelData.id)+(modelData.reused||modelData.reuse?" · "+T3I18n.tr("existing clone reused"):"")+(modelData.error?" · "+root.message(modelData.error):modelData.state==="pending"?" · "+T3I18n.tr("pending"):"")}
     Caption{visible:!!modelData.help;text:modelData.help?T3I18n.tr("Sign in on this computer, then try again: {steps}",{steps:modelData.help.steps.join("  ·  ")}):""}
    }}
   Caption{objectName:"vaultSetupComplete";visible:!!root.setupResult;Layout.leftMargin:Style.space(12);opacity:1;text:root.setupResult&&root.setupResult.complete?T3I18n.tr("Set up on this computer."):T3I18n.tr("Some steps are still pending.")}
   Caption{visible:!!root.action.error&&(root.action.action==="setup"||root.action.action==="create");Layout.leftMargin:Style.space(12);Layout.bottomMargin:Style.space(10);color:Color.urgent;opacity:1;text:root.message(root.action.error)}
   Item{implicitHeight:Style.space(6)}
  }}
 }

 // ---------------------------------------------------------------- this computer
 T3PreferenceSection {Layout.fillWidth:true;title:"On this computer";icon:"hard-drive";visible:root.matches("On this computer")||root.matches("Open in Obsidian")
  body:Component {ColumnLayout {width:parent.width;spacing:Style.space(8)
   Item{implicitHeight:Style.space(4)}
   Caption{objectName:"vaultLocalState";Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);opacity:1
    text:!root.state?T3I18n.tr("Loading…"):root.configured?T3I18n.tr("Ready: {path} stays in sync with {remote}.",{path:root.config.path,remote:(root.config.remote||{}).url}):root.cloned&&!root.config.remote?T3I18n.tr("Not set up in ot3 yet; there is a vault clone at {path}.",{path:root.config.path}):T3I18n.tr("Not set up on this computer yet.")}
   Repeater{model:root.state&&root.config.enabled?[{id:"clone",label:"Clone of the remote"},{id:"identity",label:"Git name and email for commits"},{id:"instructions",label:"Agent instructions"},{id:"obsidian",label:"Obsidian installed"}]:[]
    Caption{required property var modelData;objectName:"vaultStep_"+modelData.id;Layout.leftMargin:Style.space(12);text:(root.steps[modelData.id]?"✓ ":"○ ")+T3I18n.tr(modelData.label)+(root.steps[modelData.id]?"":" · "+T3I18n.tr("pending"))}}
   Caption{objectName:"vaultSyncState";visible:root.cloned&&!!root.sync.git;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12)
    text:root.sync.conflict?T3I18n.tr("Conflict: resolve it in the vault folder. Local commits were kept."):T3I18n.tr("Branch {branch} · {ahead} to push · {behind} to pull · {changes} changed files · last pull {pull} · last push {push}",{branch:root.sync.branch||"—",ahead:root.sync.ahead||0,behind:root.sync.behind||0,changes:root.sync.changes||0,pull:root.when(((root.state.sync||{}).pull||{}).at),push:root.when(((root.state.sync||{}).push||{}).at)})}
   Caption{visible:!!((root.state&&root.state.sync||{}).pull||{}).error||!!((root.state&&root.state.sync||{}).push||{}).error;Layout.leftMargin:Style.space(12);color:Color.urgent;opacity:1
    text:root.message((((root.state||{}).sync||{}).push||{}).error||(((root.state||{}).sync||{}).pull||{}).error||"")}
   Caption{objectName:"vaultObsidianMissing";visible:!!root.state&&!root.state.obsidian.installed;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12)
    text:!root.state?"":root.state.obsidian.install.command?T3I18n.tr("Obsidian is not installed. Install it with {command}, or from {url}.",{command:root.state.obsidian.install.command,url:root.state.obsidian.install.url}):T3I18n.tr("Obsidian is not installed. Download it from {url}.",{url:root.state.obsidian.install.url})}
   Flow{Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);Layout.bottomMargin:Style.space(10);spacing:Style.space(8)
    T3PickerButton{objectName:"vaultOpenObsidian";visible:root.cloned&&!!root.state&&root.state.obsidian.installed;primary:root.configured;implicitHeight:Style.space(30);enabled:!root.action.busy;text:T3I18n.tr("Open in Obsidian");onClicked:root.bridge.runVaultAction({action:"open"})}
    T3PickerButton{objectName:"vaultSyncNow";visible:root.configured||root.cloned&&!!root.config.remote;primary:false;implicitHeight:Style.space(30);enabled:!root.action.busy;text:T3I18n.tr(root.action.busy&&root.action.action==="push"?"Syncing…":"Sync now");onClicked:root.bridge.runVaultAction({action:"push"})}
    T3PickerButton{objectName:"vaultRepair";visible:root.cloned&&!!root.config.remote;primary:false;implicitHeight:Style.space(30);enabled:!root.action.busy;text:T3I18n.tr("Repair");onClicked:root.bridge.runVaultAction({action:"repair"})}
   }
   Caption{objectName:"vaultActionError";visible:!!root.action.error&&root.action.action!=="setup";Layout.leftMargin:Style.space(12);Layout.bottomMargin:Style.space(10);color:Color.urgent;opacity:1;text:root.message(root.action.error)}
  }}
 }

 // ---------------------------------------------------------------- hosts
 T3PreferenceSection {Layout.fillWidth:true;title:"On your hosts";icon:"server";visible:root.matches("On your hosts")
  body:Component {ColumnLayout {width:parent.width;spacing:Style.space(8)
   Item{implicitHeight:Style.space(4)}
   Caption{Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);text:T3I18n.tr("Vaults set up on your connected environments. They are only read; the clone is made on this computer with its own sign-in.")}
   Caption{objectName:"vaultHostsLoading";visible:root.hosts.busy;Layout.leftMargin:Style.space(12);text:T3I18n.tr("Looking at your hosts…")}
   Caption{objectName:"vaultNoHosts";visible:!root.hosts.busy&&root.hosts.loaded&&!(root.hosts.hosts||[]).length;Layout.leftMargin:Style.space(12);text:T3I18n.tr("No other host is connected. Clone a remote or create a vault in Other options.")}
   Repeater{model:root.hosts.hosts||[]
    Caption{required property var modelData;objectName:"vaultHost_"+modelData.environment;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);opacity:modelData.state==="online"?0.85:0.65
     text:modelData.name+" · "+(modelData.state==="offline"?T3I18n.tr("offline: unknown, not checked"):modelData.state==="unsupported"?T3I18n.tr("this T3 version cannot report it"):!modelData.vaults.length?T3I18n.tr("no vault"):modelData.vaults.map(function(v){return v.path+" → "+(v.remote||T3I18n.tr(v.git&&v.remoteError!=="no-origin"?"remote not readable":"no remote"))}).join(" · "))}}
   T3Select{objectName:"vaultSource";visible:root.shareable.length>1;Layout.leftMargin:Style.space(12);Layout.preferredWidth:Style.space(320);popupAbove:false;model:root.shareable.map(function(v){return {id:v.id,name:v.remote+" ("+v.hosts.map(function(h){return h.name}).join(", ")+")"}});textRole:"name";currentIndex:Math.max(0,root.shareable.findIndex(function(v){return v.id===root.chosen}));onActivated:function(i){root.chosen=root.shareable[i].id}}
   Flow{Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);Layout.bottomMargin:Style.space(10);spacing:Style.space(8)
    T3PickerButton{objectName:"vaultRefreshHosts";primary:false;implicitHeight:Style.space(30);enabled:!root.hosts.busy;text:T3I18n.tr("Look again");onClicked:root.bridge.discoverVaults()}
   }
  }}
 }

 // ---------------------------------------------------------------- set up
 T3PreferenceSection {objectName:"vaultOtherWays";Layout.fillWidth:true;title:root.suggestion?"Other ways to set up":"Set up";icon:"folder-git-2";visible:!!root.state&&!root.configured&&(root.showOther||!root.suggestion)&&root.matches("Set up")
  body:Component {ColumnLayout {width:parent.width;spacing:Style.space(8)
   Item{implicitHeight:Style.space(4)}
   Caption{Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);text:T3I18n.tr("Folder on this computer. An existing clone of the same remote is reused; a folder with other files is never overwritten.")}
   Field{objectName:"vaultFolder";Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);text:root.folderPath;onTextEdited:root.folder=text}
   Caption{Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);text:T3I18n.tr("Or clone a remote by its URL (GitHub, or a repository on one of your hosts):")}
   RowLayout{Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);spacing:Style.space(8)
    Field{objectName:"vaultManualUrl";placeholderText:"https://github.com/you/dev-vault.git";onTextEdited:root.manualUrl=text}
    T3PickerButton{objectName:"vaultCloneUrl";primary:false;implicitHeight:Style.space(30);enabled:!root.action.busy&&!!root.manualUrl.trim();text:T3I18n.tr("Clone");onClicked:root.setup({url:root.manualUrl.trim()})}
   }
   Caption{Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);text:T3I18n.tr("Or create a new vault in that folder, with a private GitHub repository (needs gh):")}
   RowLayout{Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);spacing:Style.space(8)
    Field{objectName:"vaultGithubName";text:root.githubName;onTextEdited:root.githubName=text}
    T3PickerButton{objectName:"vaultCreate";primary:false;implicitHeight:Style.space(30);enabled:!root.action.busy;text:T3I18n.tr("Create vault");onClicked:root.bridge.runVaultAction({action:"create",path:root.folderPath,github:root.githubName.trim()})}
   }
   Item{implicitHeight:Style.space(6)}
  }}
 }

 // ---------------------------------------------------------------- sync and agents
 T3PreferenceSection {Layout.fillWidth:true;title:"Sync and agents";icon:"git-branch";visible:!!root.state&&!!root.config.remote&&root.matches("Sync and agents")
  body:Component {Column {width:parent.width
   T3PreferenceField{width:parent.width;controlName:"vaultPullOnOpen";field:({key:"vaultPullOnOpen",label:"Pull when ot3 opens",kind:"bool",value:root.config.sync.pullOnOpen});writable:!root.action.busy;onChanged:function(value){root.saveSync("pullOnOpen",value)}}
   T3PreferenceField{width:parent.width;divided:true;controlName:"vaultPullOnStart";field:({key:"vaultPullOnStart",label:"Pull when a thread starts",kind:"bool",value:root.config.sync.pullOnThreadStart});writable:!root.action.busy;onChanged:function(value){root.saveSync("pullOnThreadStart",value)}}
   T3PreferenceField{width:parent.width;divided:true;controlName:"vaultPushOnSettle";field:({key:"vaultPushOnSettle",label:"Commit and push when threads settle",description:"Changes in the vault are committed as journal(<project>) and pushed. A conflict stops the sync without losing anything.",kind:"bool",value:root.config.sync.pushOnSettle});writable:!root.action.busy;onChanged:function(value){root.saveSync("pushOnSettle",value)}}
   T3PreferenceField{width:parent.width;divided:true;controlName:"vaultAgentInstructions";field:({key:"vaultAgentInstructions",label:"Agent instructions",description:"A marked block in ~/.claude/CLAUDE.md and ~/.codex/AGENTS.md on this computer points agents at the vault. The rest of those files is kept.",kind:"bool",value:root.config.agentInstructions});writable:!root.action.busy;onChanged:function(value){root.bridge.runVaultAction({action:"save",patch:{agentInstructions:value}})}}
  }}
 }
}
