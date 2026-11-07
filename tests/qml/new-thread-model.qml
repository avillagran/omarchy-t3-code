import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Client
import "UnderTest/ui/js/NativeNewThreadModel.js" as Native
ShellRoot {
 id:root
 readonly property var bridge:Client.T3Backend
 Window{id:window;visible:true;width:640;height:300;Client.T3ModelFallback{id:banner;anchors.fill:parent;bridge:root.bridge}}
 TestCase{id:keys;when:false}
 Component{id:panelType;Client.T3Panel{bridge:root.bridge}}
 function check(value,message){if(!value){console.error("NEW_THREAD_MODEL_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 Timer{interval:400;running:true;onTriggered:{
  var request=new XMLHttpRequest();request.open("GET",Qt.resolvedUrl("UnderTest/tests/fixtures/new-thread-model-4301.json"),false);request.send();var data=JSON.parse(request.responseText)
  data.cases.forEach(function(test,index){check(JSON.stringify(Native.resolveNewThreadModelSelection(test.input))===JSON.stringify(test.result),"complete native decision "+index)})
  var sample=data.cases.find(function(test){return !!test.result.fallback}),input=sample.input
  var command={action:"new-thread",environment:"env",projectId:"project",title:"Synthetic",modelSelection:input.defaultSelection,useProjectDefaults:true,workspaceStrategy:{type:"root"}}
  root.bridge.workspaceOperation={busy:true,requestId:"settings-proof",request:{command:command},action:"read"}
  root.bridge.action({op:"settings",environment:"env",requestId:"stale",result:{}})
  check(root.bridge.workspaceOperation.busy,"stale settings result cannot affect creation")
  root.bridge.action({op:"settings",environment:"env",requestId:"settings-proof",result:{mode:"worktree",defaultModelSelection:input.defaultSelection,defaultRuntimeMode:"approval-required",modelDefaults:{fallbackSelection:input.fallbackSelection,providers:input.providers,settings:input.settings}}})
  check(root.bridge.managementRequest.modelSelection.instanceId===input.fallbackSelection.instanceId,"actual creation consumer chooses the native fallback")
  check(root.bridge.managementRequest.workspaceStrategy.type==="root","explicit New Local keeps root despite saved worktree preference")
  check(!!root.bridge.managementRequest.modelFallback,"actual creation retains fallback explanation")
  root.bridge.state={threads:[{key:"env/thread",id:"thread",environmentId:"env",modelSelection:input.fallbackSelection}],projects:[],environments:[]}
  root.bridge.selectedEnvironment="env";root.bridge.selectedThread="thread";root.bridge.selectedKey="env/thread"
  root.bridge.modelFallbackNotices={"env/thread":{selection:input.fallbackSelection,defaultSelection:input.defaultSelection,provider:sample.result.fallback.provider,resetsAt:sample.result.fallback.resetsAt}}
  check(!!root.bridge.modelFallbackNotice,"accepted fallback has an actual composer notice")
  keys.mouseClick(keys.findChild(banner,"useFallbackDefault"))
  check(root.bridge.draft.modelSelection.instanceId===input.defaultSelection.instanceId,"explicit Use default restores the original model in the real composer draft")
  check(!root.bridge.modelFallbackNotice,"explicit restoration dismisses the notice")
  root.bridge.managementBusy=false;root.bridge.managementRequest=null
  root.bridge.state={threads:[],projects:[{id:"project",environmentId:"env",key:"env/project",defaultModelSelection:input.defaultSelection}],environments:[]}
  root.bridge.openProject("env","project",true)
  check(root.bridge.pendingProjectOpen.stage==="models","automatic project opening waits for current native model context")
  var pending=root.bridge.pendingProjectOpen
  root.bridge.action({op:"settings",environment:"env",requestId:pending.requestId,result:{defaultModelSelection:input.defaultSelection,modelDefaults:{fallbackSelection:input.fallbackSelection,providers:input.providers}}})
  check(root.bridge.managementRequest.modelSelection.instanceId===input.fallbackSelection.instanceId,"automatic project opening uses the same actual fallback consumer")
  root.bridge.managementBusy=false;root.bridge.managementRequest=null
  root.bridge.openProject("env","project",true);pending=root.bridge.pendingProjectOpen
  root.bridge.state={threads:[],projects:[],environments:[]}
  root.bridge.action({op:"settings",environment:"env",requestId:pending.requestId,result:{defaultModelSelection:input.defaultSelection,modelDefaults:{fallbackSelection:input.fallbackSelection,providers:input.providers}}})
  check(!root.bridge.managementRequest && !root.bridge.pendingProjectOpen,"project disappearance before the read completes never creates a stale thread")
  console.log("NEW_THREAD_MODEL_QML_PASS: "+data.cases.length+" whole native decisions, actual backend creation, stale settings fence and explicit local workspace")
  Qt.quit()
 }}
}
