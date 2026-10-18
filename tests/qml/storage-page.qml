import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var sent:({})
 function check(ok,message){if(!ok){console.error("STORAGE_PAGE_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 property var caps:({storageCleanup:true,storageCleanupRun:true,worktreesDirectory:true,projectWorktreeCleanup:true})
 property var rules:({worktreeKeepWhen:"uncommitted-changes",worktreeAfterDays:8,worktreeOnMerge:false,worktreeOnDelete:true,worktreeUnchanged:false})
 function data(mode){var fields=[{key:"worktreesDirectory",label:"Worktree location",kind:"text",value:"~/trees",resetValue:""},{key:"worktreeCleanup",kind:"worktree-policy",value:mode==="inherit"?null:{mode:mode},mode:mode,modeMixed:false,rules:root.rules,rulesMixed:{}}];Object.keys(rules).concat(["browserArtifactsAfterDays","logsAfterDays"]).forEach(function(key){fields.push({key:"storageCleanup."+key,label:key,value:rules[key]===undefined?null:rules[key],choices:key==="worktreeKeepWhen"?[{id:"uncommitted-changes",name:"Uncommitted changes"}]:[]})});return {fields:fields,storageScopes:[{target:{environment:"fixture"},capabilities:root.caps}]}}
 FloatingWindow {id:win;visible:true;implicitWidth:1000;implicitHeight:1000;Rectangle{id:surface;anchors.fill:parent;color:"#101214";Test.T3StorageSettings{id:page;anchors.left:parent.left;anchors.right:parent.right;settings:root.data("inherit");writable:true;onSettingChanged:function(key,value){root.sent={key:key,value:value}};onStaged:function(key,value){var next=Object.assign({},page.drafts);next[key]=value;page.drafts=next}}}}
 TestCase {id:keys;parent:win.contentItem;when:false}
 Timer {interval:150;running:true;repeat:true;onTriggered:{
  if(phase===0){check(page.supported&&page.rules.length===5&&page.artifacts.length===2,"original global rule/artifact groups");var directory=keys.findChild(page,"preferenceText_worktreesDirectory");check(directory&&directory.visible,"worktree location reachable");directory.text=" ~/new-trees ";directory.editingFinished();check(root.sent.key==="worktreesDirectory"&&root.sent.value===" ~/new-trees ","directory commits on blur through existing settings path");var logs=keys.findChild(page,"preferenceEnabled_storageCleanup.logsAfterDays");keys.mouseClick(logs);check(root.sent.key==="storageCleanup.logsAfterDays"&&root.sent.value===8,"retention uses native eight-day enable default");page.projectScope=true;phase=1}
  else if(phase===1){check(page.rules.length===0&&page.artifacts.length===0,"inherit hides custom rules and environment-only retention");var select=keys.findChild(page,"preferenceChoice_worktreeCleanup");select.activated(2);check(root.sent.key==="worktreeCleanup"&&root.sent.value.mode==="custom"&&Object.keys(root.sent.value.rules).length===0,"custom mode delegates per-target rule completion to native scope planner");page.settings=root.data("custom");phase=2}
  else if(phase===2){check(page.rules.length===5,"custom project rules visible");var toggle=keys.findChild(page,"preference_storageCleanup.worktreeOnMerge");keys.mouseClick(toggle);check(root.sent.key==="worktreeCleanup"&&root.sent.value.rules.worktreeOnMerge===true&&Object.keys(root.sent.value.rules).length===1,"project patch preserves other target-specific rules");keys.findChild(page,"preferenceChoice_worktreeCleanup").activated(1);check(root.sent.value.mode==="off"&&!root.sent.value.rules,"Off has exact native shape");page.settings=root.data("off");phase=3}
  else if(phase===3){check(page.rules.length===0,"Off hides deletion controls");keys.findChild(page,"preferenceChoice_worktreeCleanup").activated(0);check(root.sent.value===null,"Inherit removes project policy override");var d=root.data("custom");d.fields[1].modeMixed=true;page.settings=d;phase=4}
  else if(phase===4){check(page.rules.length===0,"mixed modes hide policy-specific controls");var d=root.data("custom");d.storageScopes=[{capabilities:{storageCleanup:true,projectWorktreeCleanup:false}}];page.settings=d;phase=5}
  else if(phase===5){check(!page.supported&&keys.findChild(page,"storageUnsupported").visible,"unsupported selected machine shows original scope notice");page.projectScope=false;page.settings=root.data("inherit");page.query="rotated";phase=6}
  else if(phase===6){check(page.rules.length===0&&page.artifacts.length===1,"native storage search filters original descriptions");page.query="";surface.grabToImage(function(r){check(r.saveToFile("/tmp/ot3-storage-page.png"),"capture native page");console.log("STORAGE_PAGE_QML_PASS: global/project rules, capability notices, per-target policy patches, retention, blur commit and search");Qt.quit()});phase=7}
 }}
 Timer {interval:7000;running:true;onTriggered:{console.error("STORAGE_PAGE_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
