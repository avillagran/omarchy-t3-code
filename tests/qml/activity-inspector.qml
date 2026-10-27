import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var fixture:null
 property string oldToken:""
 property var oldResponse:null
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 function check(ok,msg){if(!ok){console.error("ACTIVITY_INSPECTOR_QML_FAIL "+msg);Qt.exit(1)}}
 function ref(id){return {sourceThreadId:"ancestor",sourceItemId:id,type:"command_execution",runId:"run",providerTurnId:"turn",revision:"live"}}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,popup=keys.findChild(panel,"t3ToolPopup"),inspector=keys.findChild(panel,"t3ToolInspector")
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"},{id:"two",key:"fixture/two",environmentId:"fixture",title:"Two"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one");b.transcript=[{id:"activity",role:"activity",activity:{command:"printf",category:"run",active:false,inspector:ref("item"),entries:[]}}];panel.open();phase=1}
  else if(phase===1){if(b.draft.loading||panel.restoringConversation)return;check(!inspector,"transcript never creates tool inspector controls");var row=panel.conversationView.itemAtIndex(0);keys.mouseClick(keys.findChild(row,"t3ActivityInspect"),10,10);inspector=keys.findChild(panel,"t3ToolInspector");check(popup.visible&&inspector&&inspector.busy,"activity click creates controls, opens and requests");check(!popup.releaseContent(),"visible inspector cannot unload");check(inspector.pendingScope.sourceThreadId==="ancestor"&&inspector.pendingScope.threadId==="one","inherited source retained separately from viewer");oldToken=inspector.requestId;oldResponse={scope:Object.assign({},inspector.pendingScope),available:true,title:"printf",status:"completed",sections:[{key:"output",label:"Output",text:"<b>literal</b>",truncated:false}],raw:"{}",metadata:"{}"};b.action({op:"tool-inspector",requestId:oldToken,result:oldResponse});phase=2}
  else if(phase===2){check(inspector.detail.title==="printf"&&!inspector.busy,"native response forwarded to inspector");var output=keys.findChild(inspector,"t3ToolInspectorText_output");check(output&&output.text==="<b>literal</b>"&&output.textFormat===TextEdit.PlainText,"visible literal output");b.selectThread("fixture","two");b.action({op:"tool-inspector",requestId:oldToken,result:oldResponse});check(!popup.visible&&!inspector.opened&&!inspector.detail.title,"thread change closes and rejects late output");check(popup.releaseContent(),"closed inspector releases controls");check(!keys.findChild(panel,"t3ToolInspectorLoader").item,"inspector loader empty after release");popup.inspect(ref("new-item"));inspector=keys.findChild(panel,"t3ToolInspectorLoader").item;check(inspector&&inspector.busy&&inspector.threadId==="two","unloaded inspector reopens scoped to current chat");popup.close();panel.close();console.log("ACTIVITY_INSPECTOR_QML_PASS: actual row click, inherited native reference, scoped response and close on thread switch");Qt.quit()}
 }}
 Timer{interval:8000;running:true;onTriggered:{console.error("ACTIVITY_INSPECTOR_QML_FAIL timeout");Qt.exit(1)}}
}
