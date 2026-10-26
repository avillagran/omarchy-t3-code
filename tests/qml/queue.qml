import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var calls:[]
 property var queueItems:[{runId:"q1",messageId:"m1",text:"First queued",attachments:[]},{runId:"q2",messageId:"m2",text:"Second queued",attachments:[{type:"file",id:"kept",name:"keep.txt",mimeType:"text/plain",sizeBytes:5}]}]
 function check(ok,msg){if(!ok){console.error("T3_QUEUE_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 TestCase{id:keys;when:false}
 QtObject{
  id:fake
  property var viewState:({});property var draft:({});property var queueOperation:({});property var queuedItems:root.queueItems
  property var queue:({items:root.queueItems,held:false,canReorder:true,canSteer:true,activeRunId:"active"})
  function rememberView(v){viewState=Object.assign({},viewState,v)}
  function queueAction(action,values){root.calls=root.calls.concat([Object.assign({action:action},values)])}
  function beginQueueEdit(item){root.calls=root.calls.concat([{action:"edit",runId:item.runId}])}
 }
 Test.PinnedKeyboardPanel{
  id:window;anchorItem:null;bar:null;contentWidth:700;contentHeight:260;padding:8
  Test.T3Queue{id:control;anchors.fill:parent;bridge:fake}
 }
 Timer{interval:230;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}]};b.selectThread("fixture","one");phase=1
  }else if(phase===1){if(b.draft.loading)return;b.queue={items:root.queueItems};b.edit("My normal unsent draft");b.beginQueueEdit(root.queueItems[1]);check(b.draft.text==="Second queued"&&b.draft.queueEdit.backup.text==="My normal unsent draft","queued editor protects normal draft");check(b.draft.attachments[0].retained,"existing queued attachment is retained");b.edit("Temporary edit");b.cancelQueueEdit();check(b.draft.text==="My normal unsent draft"&&!b.draft.queueEdit,"Cancel restores normal draft");b.beginQueueEdit(root.queueItems[0]);b.edit("Updated queued text");b.submit();var a=b.draft.attempt;check(a.op==="queue"&&a.action==="edit"&&a.runId==="q1","save uses queued edit, not message dispatch");b.action({op:"queue",environment:"fixture",thread:"one",requestId:a.requestId,result:{sequence:1}});check(b.draft.text==="My normal unsent draft"&&!b.draft.queueEdit,"ACK restores normal draft");b.beginQueueEdit(root.queueItems[0]);b.edit("Submitted version");b.submit();var late=b.draft.attempt;b.patchDraft(b.selectedKey,{text:"Newer version"});b.action({op:"queue",environment:"fixture",thread:"one",requestId:late.requestId,result:{sequence:2}});check(b.draft.text==="Newer version"&&!!b.draft.queueEdit&&!b.draft.busy,"late ACK preserves a newer queued edit");b.cancelQueueEdit();window.open=true;phase=2
  }else if(phase===2){check(control.visible&&control.items.length===2,"dedicated queue component renders all queued rows");keys.mouseClick(keys.findChild(control,"queueEdit_q1"),14,14);phase=3
  }else if(phase===3){check(root.calls[0].action==="edit"&&root.calls[0].runId==="q1","edit icon targets queued row");var steer=keys.findChild(control,"queueSteer_q1");steer.forceActiveFocus();keys.keyClick(Qt.Key_Return);keys.mouseClick(keys.findChild(control,"queueRemove_q2"),14,14);phase=4
  }else if(phase===4){check(root.calls.some(function(c){return c.action==="steer"&&c.targetRunId==="active"}),"Steer targets active run");check(root.calls.some(function(c){return c.action==="cancel"&&c.runId==="q2"}),"remove targets queued run only");var grip=keys.findChild(control,"queueGrip_q2");keys.mousePress(grip,8,10);keys.mouseMove(grip,8,-25,20);keys.mouseRelease(grip,8,-25);phase=5
  }else if(phase===5){check(root.calls.some(function(c){return c.action==="reorder"&&c.runId==="q2"&&c.beforeRunId==="q1"}),"native drag reorders queue");fake.queue={items:root.queueItems,held:true,canReorder:false,canSteer:false};phase=6
  }else if(phase===6){check(!keys.findChild(control,"queueSteer_q1").enabled,"Steer follows provider capability");keys.mouseClick(keys.findChild(control,"t3QueueResume"),20,10);check(root.calls.some(function(c){return c.action==="resume"}),"held queue has Resume action");var toggle=keys.findChild(control,"t3QueueToggle");toggle.forceActiveFocus();keys.keyClick(Qt.Key_Return);phase=7
  }else if(phase===7){check(!control.expanded&&fake.viewState.queueExpanded===false,"queue collapses by keyboard and persists choice");window.open=false;console.log("T3_QUEUE_QML_PASS: dedicated queue, edit/cancel/ACK draft protection, attachments, mouse/keyboard actions, drag and capabilities");Qt.quit()}
 }}
}
