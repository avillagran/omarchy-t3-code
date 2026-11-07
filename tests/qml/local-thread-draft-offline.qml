import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client
ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 property int phase:0
 property int reads:0
 function check(value,message){if(!value){console.error("LOCAL_DRAFT_OFFLINE_FAIL "+message);Qt.exit(1);throw Error(message)}}
 function commands(){var request=new XMLHttpRequest();request.open("GET","file:///evidence/commands.json",false);request.send();return JSON.parse(request.responseText || "[]")}
 function remote(commands){return commands.filter(function(command){return ["settings","catalog","manage","send","select"].indexOf(command.op)>=0})}
 Timer{interval:100;repeat:true;running:true;onTriggered:{
  var b=root.bridge
  if(root.phase===0){
   if(!b.viewStateLoaded||!(b.state.projects || []).length)return
   b.visiblePanels=1
   b.state=Object.assign({},b.state,{environments:[{id:"env",state:"offline"}]})
   b.catalogs={};b.localProjectContexts={};b.clearSelection()
   root.phase=10;return
  }
  if(root.phase===10){
   root.reads=root.remote(root.commands()).length
   b.requestNewThread({action:"new-thread",environment:"env",projectId:"project",title:"Offline first config"})
   check(b.localDraft&&!b.managementBusy,"cached project opens an editable local draft before first configuration")
   check(!b.draft.modelSelection,"missing configuration does not invent a provider")
   b.edit("Authored offline before configuration")
   b.submit(false)
   check(!b.draft.attempt&&!b.draft.busy,"offline submission cannot reserve or dispatch an uncertain remote attempt")
   root.phase=1;return
  }
  if(root.phase===1){
   if(b.draft.saveState!=="saved")return
   check(root.remote(root.commands()).length===root.reads,"opening and editing offline requires no fresh RPC or eager server write")
   check(b.draft.text==="Authored offline before configuration","offline authored content remains intact")
   check(b.threadItems.length===1&&b.state.threads.length===0,"the sidebar exposes the local draft without phantom server metadata")
   b.localProjectContexts={"env/project":{mode:"worktree",modeSource:"project",defaultModelSelection:{instanceId:"codex",model:"fixture",options:[]},modelDefaults:{providers:[],settings:{providerInstances:{codex:{driver:"codex",enabled:true}},projectSettingsOverrides:{}}}}}
   b.requestNewThread({environment:"env",projectId:"project",title:"Cached configured draft"})
   check(b.draft.modelSelection.instanceId==="codex"&&b.selectedInfo.workspaceStrategy.type==="worktree","cached defaults apply offline without requiring new configuration")
   root.phase=2;return
  }
  if(root.phase===2){
   if(b.draft.saveState!=="saved")return
   check(root.remote(root.commands()).length===root.reads,"cached configured creation still requires no remote read or write")
   console.log("LOCAL_DRAFT_OFFLINE_QML_PASS: cached project, first-config absence, durable content and no remote writes")
   Qt.quit()
  }
 }}
}
