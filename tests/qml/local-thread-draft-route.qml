import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client
import "UnderTest/ui/js/LocalThreadDrafts.js" as Drafts
ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 property int phase:0
 property string source:""
 property string destination:""
 property string draftId:""
 property string reservation:""
 function check(value,message){if(!value){console.error("LOCAL_DRAFT_ROUTE_FAIL "+message);Qt.exit(1);throw Error(message)}}
 Timer{interval:100;repeat:true;running:true;onTriggered:{
  var b=root.bridge
  if(root.phase===0){
   if(!b.viewStateLoaded||!(b.state.projects || []).length)return
   b.visiblePanels=1
   b.state=Object.assign({},b.state,{projects:b.state.projects.concat([{id:"other",key:"second/other",environmentId:"second",title:"Destination",workspaceRoot:"/tmp/other"}]),environments:[{id:"env",state:"offline"},{id:"second",state:"offline"}]})
   b.requestNewThread({environment:"env",projectId:"project",title:"Owned route",modelSelection:{instanceId:"primary",model:"fixture",options:[]}})
   root.source=b.selectedKey
   var entry=Drafts.metadata(b,root.source);root.draftId=entry.draftId;root.reservation=entry.threadReservationId;root.destination="second/"+entry.threadId
   b.edit("Authored during asynchronous routing")
   b.patchDraft(root.source,{modelSelectionExplicit:true})
   var request={localDraftKey:root.source,localInitial:{draftId:root.draftId,selectionEpoch:b.selectionEpoch,created:true},command:{environment:"env",projectId:"project"}}
   check(Drafts.route(b,request,{environment:"second",projectId:"other"}),"a valid route reserves a bounded two-receipt transfer")
   check(!!Drafts.metadata(b,root.source)&&!Drafts.metadata(b,root.destination),"metadata does not move before durable receipts")
   b.edit("Newest authored text while source acknowledgement is pending")
   root.phase=1;return
  }
  if(root.phase===1){
   if(b.workspaceOperation.busy||!Drafts.metadata(b,root.destination))return
   var entry=Drafts.metadata(b,root.destination)
   check(!Drafts.metadata(b,root.source)&&entry.draftId===root.draftId&&entry.threadReservationId===root.reservation,"route preserves the local owner and future thread identity")
   check(b.selectedKey===root.destination&&b.draft.text==="Newest authored text while source acknowledgement is pending","the transfer saves the latest content before switching the route")
   check(b.draft.modelSelectionExplicit&&b.draft.modelSelection.instanceId==="primary","routing preserves explicit model selection")
   check(b.drafts[root.source].text===b.draft.text,"source recovery content remains retained")
   check(b.state.threads.length===0,"routing does not materialize a server thread")
   console.log("LOCAL_DRAFT_ROUTE_QML_PASS: source/target receipts, edited-content race, stable ownership, explicit selection and retained source")
   Qt.quit()
  }
 }}
}
