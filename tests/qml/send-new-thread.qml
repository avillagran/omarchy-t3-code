import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("SEND_NEW_THREAD_QML_FAIL "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,c=panel.composerView,e=c.inputField
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[{key:"fixture/one",id:"one",projectId:"p",projectKey:"fixture/p",environmentId:"fixture",title:"Source",modelSelection:{instanceId:"codex",model:"fixture"},runtimeMode:"approval-required",branch:"main"}],projects:[{key:"fixture/p",id:"p",environmentId:"fixture",title:"Repo"}],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.patchDraft(b.selectedKey,{text:"Keep this prompt",busy:false,attempt:null,loading:false,saveState:"saved"});e.forceActiveFocus();phase=2
  }else if(phase===2){var button=keys.findChild(c,"t3SendAndNewThread");check(button.enabled&&button.tooltipText.indexOf("Ctrl+Alt+Enter")>=0,"adjacent button advertises real shortcut");keys.mouseClick(button);var a=b.draft.attempt;check(a&&a.nextThread.projectId==="p"&&a.nextThread.branch==="main","send carries project and branch");check(!b.managementBusy,"no new thread before send acknowledgement");b.action({op:"send",environment:"fixture",thread:"one",requestId:a.requestId,error:"rpc_outcome_unknown"});check(b.draft.text==="Keep this prompt"&&b.draft.attempt.requestId===a.requestId&&!b.managementBusy,"uncertain send keeps prompt and identical retry");b.action({op:"send",environment:"fixture",thread:"one",requestId:a.requestId});check(b.managementRequest&&b.managementRequest.action==="new-thread"&&b.managementRequest.projectId==="p","acknowledgement opens fresh thread in same project");check(b.draft.text==="","acknowledged source prompt clears");b.managementRequest=null;b.managementBusy=false;b.patchDraft(b.selectedKey,{text:"Keyboard prompt",busy:false,attempt:null});e.forceActiveFocus();phase=3
  }else if(phase===3){keys.keyClick(Qt.Key_Return,Qt.ControlModifier|Qt.AltModifier);check(b.draft.attempt&&b.draft.attempt.nextThread,"keyboard and button share guarded action");console.log("SEND_NEW_THREAD_QML_PASS: real button/shortcut, preserved failed send, same-project handoff only after acknowledgement");panel.close();Qt.quit()}
 }}
}
