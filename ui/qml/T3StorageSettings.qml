import QtQuick
import QtQuick.Layouts
import qs.Commons
ColumnLayout {
 id:root
 objectName:"storageSettings"
 required property var settings
 property bool projectScope:false
 property bool writable:false
 property string query:""
 property var drafts:({})
 signal settingChanged(string key,var value)
 signal staged(string key,string value)
 readonly property var fields:settings.fields || []
 readonly property var scopes:settings.storageScopes || []
 readonly property var policy:fields.find(function(f){return f.key==="worktreeCleanup"}) || ({})
 readonly property string mode:policy.mode || (policy.source==="project"&&policy.value?policy.value.mode:"inherit") || "inherit"
 readonly property bool supported:scopes.length>0&&scopes.every(function(s){return s.capabilities.storageCleanup&&(!root.projectScope||s.capabilities.projectWorktreeCleanup)})
 readonly property bool directorySupported:scopes.length>0&&scopes.every(function(s){return s.capabilities.worktreesDirectory})
 readonly property bool keepWhenSupported:scopes.length>0&&scopes.every(function(s){return s.capabilities.storageCleanupRun})
 readonly property var ruleKeys:["worktreeOnDelete","worktreeAfterDays","worktreeOnMerge","worktreeUnchanged","worktreeKeepWhen"]
 readonly property var labels:({worktreeOnDelete:"Delete worktrees with deleted threads",worktreeAfterDays:"Delete inactive worktrees",worktreeOnMerge:"Delete merged worktrees",worktreeUnchanged:"Delete unchanged worktrees",worktreeKeepWhen:"Keep worktrees with local changes",browserArtifactsAfterDays:"Delete old browser artifacts",logsAfterDays:"Delete old rotated logs"})
 function description(key,value){
  if(key==="worktreeOnDelete")return "Remove unused worktrees when active or archived threads are deleted. Worktrees with local changes are kept."
  if(key==="worktreeAfterDays")return "Remove worktrees after their threads have been inactive for this many days. Branches and thread history are kept."
  if(key==="worktreeOnMerge")return "Remove worktrees whose pull request is merged and whose commits are included in the default branch."
  if(key==="worktreeUnchanged")return "Remove worktrees with no commits beyond the default branch."
  if(key==="worktreeKeepWhen")return value==="any-local-files"?"Includes ignored files, except node_modules.":value==="tracked-changes"?"Deletes untracked and ignored files, including .env.":"Keeps edits and untracked files. Deletes ignored files, including .env."
  if(key==="browserArtifactsAfterDays")return "Delete saved browser captures after this many days. Older capture links will no longer open."
  if(key==="logsAfterDays")return "Delete inactive rotated log files after this many days. Current logs are kept."
  return ""
 }
 function matches(f){return !query.trim()||[f.label,f.description].some(function(t){return T3I18n.tr(t||"").toLocaleLowerCase().indexOf(root.query.trim().toLocaleLowerCase())>=0||String(t||"").toLocaleLowerCase().indexOf(root.query.trim().toLocaleLowerCase())>=0})}
 function field(key){
  var original=fields.find(function(f){return f.key==="storageCleanup."+key})
  if(!original&&!projectScope)return null
  var value=projectScope?(policy.rules||{})[key]:original.value
  if(typeof value==="undefined")return null
  return Object.assign({},original||{},{key:"storageCleanup."+key,label:labels[key],description:description(key,value),value:value,kind:key.endsWith("Days")?"days":key==="worktreeKeepWhen"?"choice":"bool",minimum:1,maximum:3650,enabledValue:8,retention:true,mixed:projectScope?!!(policy.rulesMixed||{})[key]:original.mixed,projectScoped:false})
 }
 function updateRule(key,value){if(!writable)return;if(projectScope){var patch={};patch[key]=value;settingChanged("worktreeCleanup",{mode:"custom",rules:patch})}else settingChanged("storageCleanup."+key,value)}
 readonly property var rules:supported&&(!projectScope||!policy.modeMixed&&mode==="custom")?ruleKeys.filter(function(k){return k!=="worktreeKeepWhen"||root.keepWhenSupported}).map(field).filter(function(f){return f&&root.matches(f)}):[]
 readonly property var artifacts:supported&&!projectScope?["browserArtifactsAfterDays","logsAfterDays"].map(field).filter(function(f){return f&&root.matches(f)}):[]
 readonly property var directory:{var f=fields.find(function(f){return f.key==="worktreesDirectory"});return f?Object.assign({},f,{commitOnBlur:true,placeholder:"Default",description:"Folder where new worktrees are created, on any drive, such as D:\\worktrees or ~/worktrees. Existing worktrees stay where they are. Leave empty to use the T3 home folder."}):null}
 readonly property var modeField:({key:"worktreeCleanup",label:"Automatic worktree cleanup",kind:"choice",value:mode,mixed:!!policy.modeMixed,choices:[{id:"inherit",name:"Inherit"},{id:"off",name:"Off"},{id:"custom",name:"Custom"}],description:mode==="off"?"Keep this project's worktrees until you delete them manually.":mode==="custom"?"Use these rules for this project.":"Use each machine's worktree cleanup settings."})
 spacing:Style.space(24)
 Text {objectName:"storageUnsupported";visible:!root.supported&&!root.settings.busy;Layout.fillWidth:true;text:T3I18n.tr(root.projectScope?"Update the selected machines to configure project worktree cleanup.":"Update the selected environments to use storage cleanup, or choose a machine that supports it.");color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 T3PreferenceSection {Layout.fillWidth:true;visible:root.supported&&(root.rules.length>0||root.projectScope&&root.matches(root.modeField)||!root.projectScope&&root.directorySupported&&root.directory&&root.matches(root.directory));title:"Worktrees";icon:"git-branch"
  body:Component {Column {width:parent.width
   T3PreferenceField {width:parent.width;visible:!root.projectScope&&root.directorySupported&&!!root.directory&&root.matches(root.directory||{});field:root.directory||({key:"worktreesDirectory",value:""});writable:root.writable;edit:root.drafts.worktreesDirectory;onStaged:function(value){root.staged("worktreesDirectory",value)};onChanged:function(value){root.settingChanged("worktreesDirectory",value)}}
   T3PreferenceField {width:parent.width;visible:root.projectScope&&root.matches(root.modeField);field:root.modeField;writable:root.writable;onChanged:function(value){if(value==="inherit")root.settingChanged("worktreeCleanup",null);else root.settingChanged("worktreeCleanup",value==="off"?{mode:"off"}:{mode:"custom",rules:{}})}}
   Repeater {model:root.rules;T3PreferenceField {required property var modelData;required property int index;width:parent.width;field:modelData;divided:index>0||root.projectScope||root.directorySupported;writable:root.writable;onChanged:function(value){root.updateRule(modelData.key.slice("storageCleanup.".length),value)}}}
  }}
 }
 T3PreferenceSection {Layout.fillWidth:true;visible:root.artifacts.length>0;title:"Artifacts and logs";icon:"archive"
  body:Component {Column {width:parent.width;Repeater {model:root.artifacts;T3PreferenceField {required property var modelData;required property int index;width:parent.width;field:modelData;divided:index>0;writable:root.writable;onChanged:function(value){root.updateRule(modelData.key.slice("storageCleanup.".length),value)}}}}}
 }
}
