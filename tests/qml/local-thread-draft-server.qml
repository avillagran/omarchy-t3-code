import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client
ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 property int phase:0
 property string key:""
 property string identity:""
 property double deadline:Date.now()+45000
 function check(value,message){if(!value){console.error("LOCAL_DRAFT_SERVER_FAIL "+message);Qt.exit(1);throw Error(message)}}
 Timer{interval:100;repeat:true;running:true;onTriggered:{
  var b=root.bridge
  check(Date.now()<root.deadline,"bounded actual server/client acceptance deadline")
  if(root.phase===0){
   if(!b.viewStateLoaded||!(b.state.projects || []).length||!(b.state.environments || []).some(function(environment){return environment.id==="fixture"&&environment.state==="ready"}))return
   check(b.state.threads.length===0,"actual server starts without thread metadata")
   var project=b.state.projects[0]
   b.visiblePanels=1
   b.requestNewThread({environment:"fixture",projectId:project.id,title:"Private native local draft",modelSelection:{instanceId:"codex",model:"fixture",options:[]},workspaceStrategy:{type:"root"}})
   root.key=b.selectedKey;root.identity=b.selectedThread
   check(b.localDraft&&b.state.threads.length===0,"real worker creation stays local before first send")
   b.edit("Private local draft first accepted message")
   root.phase=1;return
  }
  if(root.phase===1){
   if(b.draft.saveState!=="saved"||b.draft.loading)return
   b.submit(false)
   check(b.draft.attempt&&b.draft.attempt.targetThreadId===root.identity,"actual first send retains the reserved future thread identity")
   root.phase=2;return
  }
  if(root.phase===2){
   check(!b.draft.error,"actual first send reports no client error: "+b.draft.error)
   if(b.localDraft||!b.state.threads.some(function(thread){return thread.id===root.identity})||b.draft.busy||b.draft.attempt)return
   check(b.selectedKey===root.key&&b.draft.text==="","acceptance promotes the same composer and clears only submitted text")
   if(JSON.stringify(b.transcript).indexOf("Private local draft first accepted message")<0)return
   check(!b.selectedInfo.hasRuntimeError,"synthetic provider execution has no runtime error: "+b.selectedInfo.runtimeError)
   if(b.selectedInfo.working||!b.selectedInfo.latestRunId)return
   console.log("LOCAL_DRAFT_SERVER_PASS: actual QML/worker/server, local-before-metadata, durable first launch and projected authored message")
   Qt.quit()
  }
 }}
}
