import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property var fixtureState: null;property int phase:0;property string request:""
 function check(ok,msg){if(!ok){console.error("T3_COMPLETION_QML_FAIL: "+msg);Qt.exit(1)}}
 function respond(items,id){Test.T3Backend.action({op:"suggest",environment:"fixture",thread:"one",requestId:id || Test.T3Backend.suggestionRequest,result:{items:items}})}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections { target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixtureState)Test.T3Backend.state=root.fixtureState} }
 Timer{interval:250;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend;var c=panel.composerView;var e=c.inputField
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One",modelSelection:{instanceId:"codex",model:"first"}}],projects:[],environments:[]};root.fixtureState=b.state;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;e.forceActiveFocus();b.edit("Revisa @doc");Qt.callLater(function(){e.cursorPosition=b.draft.text.length});phase=2
  }else if(phase===2){check(c.completionVisible&&c.completion.kind==="path","@ opens file picker at cursor "+JSON.stringify({completion:c.completion,text:e.plainText,cursor:e.cursorPosition,focus:e.activeFocus,locked:c.locked,dismissed:c.dismissedCompletion}));request=b.suggestionRequest;respond([{label:"stale",insert:"wrong"}],"old-response");check(b.suggestionItems.length===0,"stale response ignored");respond([{kind:"path",label:"docs/My File.md",description:"file",icon:"file",insert:"[My File.md](docs/My%20File.md) "}]);keys.keyClick(Qt.Key_Return);phase=3
  }else if(phase===3){check(b.draft.text==="Revisa [My File.md](docs/My%20File.md) ","Enter inserts original T3 file serialization");check(!b.draft.attempt,"selecting completion never sends");b.edit("Usa $rev");Qt.callLater(function(){e.cursorPosition=b.draft.text.length});phase=4
  }else if(phase===4){check(c.completionVisible&&c.completion.kind==="skill","$ opens skills");respond([{kind:"skill",label:"$review",insert:"$review ",icon:"folder-code"},{kind:"skill",label:"$review-tests",insert:"$review-tests ",icon:"folder-code"}]);keys.keyClick(Qt.Key_Down);keys.keyClick(Qt.Key_Tab);phase=5
  }else if(phase===5){check(b.draft.text==="Usa $review-tests ","Down and Tab select skill without losing prompt");e.forceActiveFocus();b.edit("/com");Qt.callLater(function(){e.cursorPosition=b.draft.text.length});phase=6
  }else if(phase===6){check(c.completionVisible&&c.completion.kind==="command","/ opens commands");respond([{kind:"command",label:"/compact",insert:"/compact ",icon:"square-pen"}]);keys.keyClick(Qt.Key_Return);phase=7
  }else if(phase===7){check(b.draft.text==="/compact "&&!b.draft.attempt,"command selection inserts without executing");b.edit("@src");Qt.callLater(function(){e.cursorPosition=b.draft.text.length});phase=8
  }else if(phase===8){check(c.completionVisible,"picker reopened");keys.keyClick(Qt.Key_Escape);phase=9
  }else if(phase===9){check(!c.completionVisible&&panel.opened,"Esc dismisses picker without closing panel");check(b.draft.text==="@src","Esc preserves prompt");panel.close();console.log("T3_COMPLETION_QML_PASS: @ paths, $ skills, / commands, stale replies, Enter, Tab, arrows, Esc and no submission");Qt.quit()}
 }}
}
