import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Client
import "UnderTest/ui/js/LocalThreadDrafts.js" as Drafts
import "UnderTest/ui/js/NativeDraftProviders.js" as Providers
ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 property string first:""
 property string second:""
 property string creation:""
 property int phase:0
 property string firstAttempt:""
 property string acceptedAttempt:""
 function check(value,message){if(!value){console.error("LOCAL_THREAD_DRAFTS_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 function commandLog(){var request=new XMLHttpRequest();request.open("GET","file:///evidence/commands.json",false);request.send();return request.responseText?JSON.parse(request.responseText):[]}
 Timer{interval:50;running:true;repeat:true;onTriggered:{
  var b=root.bridge
  if(root.phase===0){
   if(!b.viewStateLoaded||!(b.state.projects || []).length)return
   var request=new XMLHttpRequest();request.open("GET",Qt.resolvedUrl("UnderTest/tests/fixtures/local-thread-draft-4301.json"),false);request.send();var native=JSON.parse(request.responseText)
   native.providerCases.forEach(function(test,index){root.check(JSON.stringify(Providers.resolveProjectDefaultProviderModelSelection(test.providers,test.settings,test.projectId))===JSON.stringify(test.selection),"whole native provider/default graph "+index)})
   native.workspaceCases.forEach(function(test,index){root.check(Providers.resolveNewThreadEnvMode(test.input)===test.mode,"whole native initial workspace choice "+index)})
   var sticky={viewState:{},rememberView:function(values){this.viewState=Object.assign({},this.viewState,values)}}
   function normalized(map){var result={};Object.keys(map || {}).forEach(function(key){var selection=map[key];result[key]={instanceId:selection.instanceId,model:selection.model,options:selection.options || []}});return result}
   native.stickyCases.forEach(function(test,index){Drafts.rememberSelection(sticky,test.selection,false);root.check(JSON.stringify(normalized(sticky.viewState.stickyModelSelectionByProvider))===JSON.stringify(normalized(test.state.stickyModelSelectionByProvider))&&sticky.viewState.stickyActiveProvider===test.state.stickyActiveProvider,"actual native sticky action "+index)})
   b.visiblePanels=1
   b.catalogs={env:{models:[{instanceId:"primary",model:"fixture",available:true},{instanceId:"backup",model:"other",available:true}],providers:[],canWrite:true}}
   b.requestNewThread({action:"new-thread",environment:"env",projectId:"project",title:"Private local",modelSelection:{instanceId:"primary",model:"fixture",options:[]},workspaceStrategy:{type:"root"}})
   root.first=b.selectedKey;root.creation=Drafts.metadata(b,root.first).threadReservationId
   check(b.localDraft,"new chat opens a local draft before metadata exists")
   check(b.state.threads.length===0&&!b.managementRequest,"opening never creates a server thread")
   check(b.threadItems.length===1&&b.selectedInfo.projectId==="project","actual sidebar/composer can reach the virtual draft")
   b.edit("User-owned retained prompt")
   b.requestNewThread({action:"new-thread",environment:"env",projectId:"project",title:"Second",modelSelection:{instanceId:"primary",model:"fixture",options:[]},workspaceStrategy:{type:"root"}})
   root.second=b.selectedKey;check(root.first!==root.second,"a draft with user content is never reused")
   check(b.drafts[root.first].text==="User-owned retained prompt","new chat preserves the existing draft")
   b.requestNewThread({action:"new-thread",environment:"env",projectId:"project",title:"Reuse",workspaceStrategy:{type:"root"}})
   check(b.selectedKey===root.second,"the existing empty project draft is reused")
   b.chooseModel(1);check(b.draft.modelSelectionExplicit&&b.viewState.stickyActiveProvider==="backup","explicit model updates the actual sticky-provider store")
   b.selectThread("env",Drafts.metadata(b,root.first).threadId)
   root.phase=1;return
  }
  if(root.phase===1){
   if(b.draft.loading||b.draft.busy||b.draft.saveState!=="saved"||(b.drafts[root.second] || {}).saveState!=="saved"||Object.keys(b.dirtyDrafts).length)return
   var before=root.commandLog();check(!before.some(function(command){return command.op==="manage"||command.op==="send"||command.op==="select"}),"unsubmitted drafts issue no phantom thread RPCs")
   b.clearSelection();b.drafts={}
   b.requestNewThread({action:"new-thread",environment:"env",projectId:"project",title:"Reload",workspaceStrategy:{type:"root"}})
   check(!!b.pendingLocalDraftCreate,"reuse waits for the saved local draft instead of guessing it is empty")
   b.selectThread("env",Drafts.metadata(b,root.second).threadId)
   root.phase=2;return
  }
  if(root.phase===2){
   if(b.pendingLocalDraftCreate || !b.drafts[root.first] || b.drafts[root.first].loading)return
   check(b.selectedKey===root.second,"a late local-draft load cannot reclaim a changed route")
   check(b.drafts[root.first].text==="User-owned retained prompt","loading preserves actual stored authored content")
   b.requestNewThread({action:"new-thread",environment:"env",projectId:"project",title:"Reload again",workspaceStrategy:{type:"root"}})
   root.phase=3;return
  }
  if(root.phase===3){
   if(b.pendingLocalDraftCreate || b.selectedKey!==root.second || b.draft.loading)return
   check(b.draft.modelSelectionExplicit&&b.draft.modelSelection.instanceId==="backup","empty-draft reuse preserves its saved explicit provider choice")
   b.selectThread("env",Drafts.metadata(b,root.first).threadId)
   b.submit(false);check(b.draft.attempt&&b.draft.attempt.op==="manage","first send reserves one durable creation+message attempt")
   check(b.draft.attempt.targetThreadId==="omarchy-thread-"+root.creation&&b.draft.attempt.requestId!==root.creation,"future thread identity is independent from each durable launch command")
   root.firstAttempt=b.draft.attempt.requestId
   root.phase=4;return
  }
  if(root.phase===4){
   if(b.draft.busy||b.managementBusy||b.draft.error!=="rpc_outcome_unknown")return
   check(b.draft.text==="User-owned retained prompt"&&b.draft.attempt.requestId===root.firstAttempt,"uncertain admission retains authored content and the exact attempt identity")
   b.action({op:"manage",environment:"env",requestId:"stale",result:{action:"new-thread",threadId:"foreign"}})
   check(b.draft.attempt.requestId===root.firstAttempt&&b.localDraft,"a stale receipt cannot promote or clear the local draft")
   b.submit(false);root.phase=5;return
  }
  if(root.phase===5){
   if(b.draft.busy||b.managementBusy||b.draft.attempt)return
   check(b.draft.text==="User-owned retained prompt"&&!!Drafts.metadata(b,root.first),"known rejection restores an editable local draft without deleting its content")
   b.edit("Revised retained prompt");b.submit(false)
   root.acceptedAttempt=b.draft.attempt.requestId
   check(root.acceptedAttempt!==root.firstAttempt&&b.draft.attempt.targetThreadId==="omarchy-thread-"+root.creation,"known rejection gets a new command and keeps the future thread")
   b.selectThread("env",Drafts.metadata(b,root.second).threadId)
   root.phase=6;return
  }
  if(root.phase===6){
   if(b.managementBusy||Drafts.metadata(b,root.first))return
   var commands=root.commandLog(),launches=commands.filter(function(command){return command.op==="manage"})
   check(launches.length===3&&launches[0].requestId===launches[1].requestId&&launches[2].requestId===root.acceptedAttempt,"uncertain retry is immutable and known rejection permits a new attempt")
   check(launches.every(function(command){return command.launchWithMessage&&command.generateTitle&&command.targetThreadId==="omarchy-thread-"+root.creation}),"every attempt uses the existing atomic launch contract and stable thread")
   check(launches[2].text==="Revised retained prompt","accepted first send retains the exact revised authored message")
   check(b.selectedKey===root.second&&b.drafts[root.first].text===""&&b.state.threads.length===1,"accepted background send promotes and clears only its submitted draft without stealing selection")
   check(b.drafts[root.second].modelSelection.instanceId==="backup","promotion preserves the other local draft and sticky choice")
   console.log("LOCAL_THREAD_DRAFTS_QML_PASS: native provider36/sticky8, local creation/reload/reuse, explicit choice, stale scope, uncertain/known failure and atomic first send")
   Qt.quit()
  }
 }}
}
