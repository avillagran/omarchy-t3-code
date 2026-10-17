import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("CONNECTION_ROUTING_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Timer{interval:200;repeat:true;running:true;onTriggered:{var b=Test.T3Backend
  if(!b.viewStateLoaded)return
  b.state={threads:[],projects:[],environments:[{id:"origin",state:"ready"},{id:"node",state:"ready"}]};b.selectedEnvironment="origin";b.selectedThread="";b.selectedKey="";
  b.requestNewThread({action:"new-thread",environment:"origin",projectId:"source-project",title:"Balanced",modelSelection:{instanceId:"codex",model:"fixture",options:[]},runtimeMode:"approval-required",automatic:true,useProjectDefaults:true});
  root.check(b.workspaceOperation.action==="routing","automatic draft first resolves native host resources");var id=b.workspaceOperation.requestId;
  b.action({op:"connection-route",requestId:id,result:{environment:"node",projectId:"node-project",modelSelection:{instanceId:"codex",model:"fixture",options:[]},balanced:true},error:""});
  root.check(b.workspaceOperation.action==="read"&&b.workspaceOperation.request.command.environment==="node"&&b.workspaceOperation.request.command.projectId==="node-project","workspace preference belongs to chosen environment and matching project");
  var request=b.workspaceOperation.request;root.check(!request.command.automatic&&!request.command.worktreePath,"chosen target is fixed before launch");id=b.workspaceOperation.requestId;
  b.action({op:"settings",requestId:id,result:{mode:"worktree",startFromOrigin:true,defaultModelSelection:{instanceId:"codex",model:"destination-default",options:[]},defaultRuntimeMode:"auto-accept-edits"},error:""});
  root.check(b.managementRequest.environment==="node"&&b.managementRequest.projectId==="node-project"&&b.managementRequest.workspaceStrategy.type==="worktree","real launch command targets selected machine with native worktree strategy");root.check(b.managementRequest.modelSelection.model==="destination-default"&&b.managementRequest.runtimeMode==="auto-accept-edits","fresh native defaults come from chosen destination before launch");var stable=b.managementRequest.requestId;b.managementBusy=false;b.manage({environment:"wrong"});root.check(b.managementRequest.requestId===stable&&b.managementRequest.environment==="node","retry identity keeps chosen machine");
  console.log("CONNECTION_ROUTING_QML_PASS: native automatic selection, matching project, destination workspace settings and stable launch retry");Qt.quit()
 }}
}
