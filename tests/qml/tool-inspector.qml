import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test

// No T3Backend, account, clipboard process, network or live activity is used.
ShellRoot {
 id:root
 property int phase:0
 property int requests:0
 property string token:""
 property var captured:({})
 property string copied:""
 property string firstToken:""
 property var firstScope:({})
 function check(ok,message){if(!ok){console.error("TOOL_INSPECTOR_QML_FAIL "+message);Qt.exit(1)}}
 function reference(id){return {sourceThreadId:"ancestor",sourceItemId:id,type:"command_execution",runId:"source-run",providerTurnId:"source-turn",revision:"2026-10-17T12:00:01.000Z"}}
 function response(scope){return {scope:scope,available:true,title:"printf fixture",status:"completed",exitCode:0,failed:false,sections:[{key:"input",label:"Input",text:"printf '<b>literal</b>'",truncated:false},{key:"output",label:"Output",text:"<b>literal</b>\nfixture output",truncated:false}],metadata:"{\"runId\":\"source-run\"}",metadataTruncated:false,raw:"{\"type\":\"command_execution\"}",rawTruncated:true,emptyOutput:false,outputOmitted:false}}
 QtObject {
  id:fakeBridge
  property var scrollPolicy:({mouseNative:false,touchpadNative:false})
  signal copyFinished(string id,string error)
  function copyText(text){root.copied=text;Qt.callLater(function(){fakeBridge.copyFinished("copy-fixture","")});return "copy-fixture"}
 }
 TestCase{id:probe;when:false}
 Test.T3ToolInspector {
  id:inspector
  width:640;height:480
  bridge:fakeBridge;environment:"fixture";threadId:"viewer"
  onRequested:function(id,scope){root.requests++;root.token=id;root.captured=scope}
 }
 Timer {
  interval:150;running:true;repeat:true
  onTriggered:{
   if(root.phase===0){
    check(requests===0&&!inspector.opened,"construction never fetches")
    check(inspector.openItem(reference("source-item")),"explicit open requests real detail")
    check(requests===1&&inspector.busy,"single request and loading state")
    check(captured.sourceThreadId==="ancestor"&&captured.threadId==="viewer","inherited source separate from viewer")
    check(captured.runId==="source-run"&&captured.providerTurnId==="source-turn","run and turn identities retained")
    check(!inspector.reload()&&requests===1,"no duplicate read while busy")
    firstToken=token;firstScope=captured
    var wrong=response(Object.assign({},captured,{environment:"another"}))
    check(!inspector.accept(token,wrong,"")&&inspector.busy,"reject mismatched response scope")
    check(inspector.accept(token,response(captured),"")&&!inspector.busy,"accept exact scope and token")
    root.phase=1
   }else if(root.phase===1){
    var output=probe.findChild(inspector,"t3ToolInspectorText_output")
    check(!!output&&output.text==="<b>literal</b>\nfixture output","real output body shown")
    check(output.textFormat===TextEdit.PlainText&&output.readOnly&&output.selectByMouse,"untrusted output is selectable plaintext")
    var copy=probe.findChild(inspector,"t3ToolInspectorCopy_output");check(!!copy,"output copy control exists");copy.clicked()
    root.phase=2
   }else if(root.phase===2){
    check(copied==="<b>literal</b>\nfixture output","copy exact bounded output, not metadata or HTML")
    check(probe.findChild(inspector,"t3ToolInspectorCopy_output").copyState==="copied","copy acknowledgement")
    probe.findChild(inspector,"t3ToolInspectorTab_raw").clicked()
    check(requests===1,"tab switches never fetch again")
    root.phase=3
   }else if(root.phase===3){
    var raw=probe.findChild(inspector,"t3ToolInspectorText_raw")
    check(!!raw&&raw.text==="{\"type\":\"command_execution\"}","raw tab displays native item")
    check(inspector.displayedSections[0].truncated,"raw truncation explicit")
    inspector.reload();check(requests===2,"explicit refresh")
    check(inspector.accept(token,{},"rpc_rejected")&&!!inspector.error&&!inspector.busy,"failure ends loading and exposes retry")
    probe.findChild(inspector,"t3ToolInspectorReload").clicked();check(requests===3&&inspector.busy,"retry uses same native scope")
    inspector.openItem(reference("new-item"));check(requests===4,"new item invalidates pending read")
    check(!inspector.accept(firstToken,response(firstScope),""),"old success rejected")
    check(!inspector.accept(firstToken,{},"late_error"),"old failure rejected")
    inspector.accept(token,{scope:captured,available:false,sections:[],raw:""},"")
    check(inspector.detail.available===false&&!inspector.error,"missing persisted item has unavailable state")
    root.phase=4
   }else if(root.phase===4){
    inspector.openItem(reference("reasoning-item"));var oldToken=token,oldScope=captured
    inspector.threadId="other-viewer"
    check(!inspector.opened&&!inspector.busy&&!inspector.detail.title,"viewer change clears private item")
    check(!inspector.accept(oldToken,response(oldScope),""),"thread switch rejects late result")
    inspector.openItem(reference("source-item"));oldToken=token;oldScope=captured
    inspector.environment="other-environment"
    check(!inspector.opened&&!inspector.accept(oldToken,response(oldScope),""),"environment switch rejects late result")
    var count=requests
    check(!inspector.openItem({id:"activity-row-without-source"})&&requests===count,"missing reference fails locally, never guesses row ID")
    check(inspector.error==="invalid_tool_reference","inspectability error exposed")
    inspector.dismiss(true)
    check(!inspector.opened&&!inspector.busy&&inspector.requestId==="","close invalidates read")
    console.log("TOOL_INSPECTOR_QML_PASS: lazy source-scoped reads, identities, plaintext, tabs, copy, missing/errors/retry, stale result rejection and close")
    Qt.quit()
   }
  }
 }
 Timer{interval:7000;running:true;onTriggered:{console.error("TOOL_INSPECTOR_QML_FAIL timeout");Qt.exit(1)}}
}
