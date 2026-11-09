import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client
ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 property int phase:0
 property string key:""
 property double openedAt:0
 property int readsBeforeOffline:0
 function check(value,message){if(!value){console.error("LOCAL_DRAFT_CLOCK_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 function commands(){var request=new XMLHttpRequest();request.open("GET","file:///evidence/commands.json",false);request.send();return JSON.parse(request.responseText || "[]")}
 Timer{interval:100;repeat:true;running:true;onTriggered:{
  var b=root.bridge
  if(root.phase===0){
   if(!b.viewStateLoaded||!(b.state.projects || []).length)return
   b.visiblePanels=1;b.catalogs={env:{models:[{instanceId:"primary",model:"fixture",available:true},{instanceId:"backup",model:"other",available:true}],providers:[],canWrite:true}}
   b.requestNewThread({action:"new-thread",environment:"env",projectId:"project",title:"Clock draft",modelSelection:{instanceId:"primary",model:"fixture",options:[]},useProjectDefaults:true,workspaceStrategy:{type:"root"}})
   root.key=b.selectedKey;root.openedAt=Date.now();root.phase=1;return
  }
  if(root.phase===1){
   if(b.workspaceOperation.busy||!b.modelFallbackNotice)return
   check(b.localDraft&&b.draft.modelSelection.instanceId==="backup"&&!b.draft.modelSelectionExplicit,"initial native fallback is an implicit local selection")
   check(!!b.localDraftClockRelease,"actual native minute store is subscribed while the draft is mounted")
   b.edit("Retained while the minute changes")
   root.readsBeforeOffline=root.commands().filter(function(command){return command.op==="settings"&&command.action==="workspace-read"}).length
   b.state=Object.assign({},b.state,{environments:[{id:"env",state:"offline"}]})
   root.phase=2;return
  }
  if(root.phase===2){
   if(b.modelFallbackNotice)return
   var reads=root.commands().filter(function(command){return command.op==="settings"&&command.action==="workspace-read"})
   check(reads.length===root.readsBeforeOffline,"the real wall-clock minute callback reevaluates cached configuration offline without a fresh RPC")
   check(b.draft.modelSelection.instanceId==="backup"&&b.draft.text==="Retained while the minute changes","minute reevaluation expires the banner without overwriting the selected model or authored text")
   check(!root.commands().some(function(command){return command.op==="manage"||command.op==="send"||command.op==="select"}),"minute reevaluation never materializes an unsubmitted thread")
   b.visiblePanels=0
   check(b.localDraftClockRelease===null,"unmount releases the native minute subscription")
   console.log("LOCAL_DRAFT_CLOCK_QML_PASS: real minute boundary after "+(Date.now()-root.openedAt)+"ms, fresh model context, retained selection/text and detached timer")
   Qt.quit()
  }
 }}
}
