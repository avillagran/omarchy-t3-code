import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Client
import "UnderTest/ui/js/NativeNewThreadModel.js" as Native
import "UnderTest/ui/js/LocalThreadDrafts.js" as Drafts
ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 Window{id:window;visible:true;width:640;height:300;Client.T3ModelFallback{id:banner;anchors.fill:parent;bridge:root.bridge}}
 Client.T3PreferenceField{id:fallbackPreference;visible:false;field:({key:"fallbackModelSelection",label:"Fallback model",kind:"model",value:null,resetValue:null})}
 TestCase{id:keys;when:false}
 Component{id:panelType;Client.T3Panel{bridge:root.bridge}}
 function check(value,message){if(!value){console.error("NEW_THREAD_MODEL_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 Timer{interval:400;running:true;onTriggered:{
  var request=new XMLHttpRequest();request.open("GET",Qt.resolvedUrl("UnderTest/tests/fixtures/new-thread-model-4301.json"),false);request.send();var data=JSON.parse(request.responseText)
  data.cases.forEach(function(test,index){check(JSON.stringify(Native.resolveNewThreadModelSelection(test.input))===JSON.stringify(test.result),"complete native decision "+index)})
  var sample=data.cases.find(function(test){return !!test.result.fallback}),input=JSON.parse(JSON.stringify(sample.input))
  // Keep actual creation admission independent of the calendar; the complete
  // native oracle cases above retain their explicit timestamp boundaries.
  input.providers.forEach(function(provider){provider.models=provider.models || [{slug:provider.instanceId===input.defaultSelection.instanceId?input.defaultSelection.model:input.fallbackSelection.model,isDefault:true,isCustom:false}];(provider.usageLimits.windows || []).forEach(function(window){delete window.resetsAt})})
  check(keys.findChild(fallbackPreference,"preferenceModel_fallbackModelSelection").emptyText==="None","nullable fallback means disabled, rather than automatic")
  var command={action:"new-thread",environment:"env",projectId:"project",title:"Synthetic",modelSelection:input.defaultSelection,useProjectDefaults:true,workspaceStrategy:{type:"root"}}
  var backend=root.bridge
  backend.viewStateLoaded=true
  backend.state={threads:[],projects:[{id:"project",key:"env/project",environmentId:"env",workspaceRoot:"/tmp/project"}],environments:[{id:"env",state:"offline"}]}
  backend.requestNewThread(command)
  var key=backend.selectedKey,entry=Drafts.metadata(backend,key)
  var initial={draftId:entry.draftId,created:true,alreadyOpen:false,carrySelection:input.defaultSelection,stickySelection:null,sourceDraftId:null}
  var result={mode:"worktree",defaultModelSelection:input.defaultSelection,defaultRuntimeMode:"approval-required",modelDefaults:{fallbackSelection:input.fallbackSelection,providers:input.providers,settings:input.settings}}
  backend.localModelRequests={"settings-proof":{key:key,draftId:entry.draftId,initial:initial}}
  backend.action({op:"settings",environment:"env",requestId:"stale",result:result})
  check(backend.draft.modelSelection.instanceId===input.defaultSelection.instanceId,"stale settings cannot change a local draft")
  backend.action({op:"settings",environment:"env",requestId:"settings-proof",result:result})
  check(backend.draft.modelSelection.instanceId===input.fallbackSelection.instanceId,"actual local creation chooses the native fallback")
  check(Drafts.metadata(backend,key).workspaceStrategy.type==="root","explicit New Local keeps root despite saved worktree preference")
  check(backend.state.threads.length===0&&!backend.managementRequest,"fallback resolution does not materialize an empty thread")
  check(!!root.bridge.modelFallbackNotice,"accepted fallback has an actual composer notice")
  keys.mouseClick(keys.findChild(banner,"useFallbackDefault"))
  check(root.bridge.draft.modelSelection.instanceId===input.defaultSelection.instanceId,"explicit Use default restores the original model in the real composer draft")
  check(!root.bridge.modelFallbackNotice,"explicit restoration dismisses the notice")
  backend.clearSelection()
  backend.openProject("env","project",true)
  check(backend.localDraft&&!backend.pendingProjectOpen&&!backend.managementRequest,"automatic cached project opening reaches a local draft without a fresh server write")
  backend.patchDraft(backend.selectedKey,{text:"Retained before project disappearance"})
  var pendingKey=backend.selectedKey,pendingEntry=Drafts.metadata(backend,pendingKey)
  backend.localModelRequests={"disappearing-project":{key:pendingKey,draftId:pendingEntry.draftId,initial:initial}}
  backend.state={threads:[],projects:[],environments:[]}
  backend.action({op:"settings",environment:"env",requestId:"disappearing-project",result:result})
  check(!backend.managementRequest&&backend.drafts[pendingKey].text==="Retained before project disappearance","project disappearance cannot materialize or erase a stale draft")
  console.log("NEW_THREAD_MODEL_QML_PASS: "+data.cases.length+" whole native decisions, actual backend creation, stale settings fence and explicit local workspace")
  Qt.quit()
 }}
}
