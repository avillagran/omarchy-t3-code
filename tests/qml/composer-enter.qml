import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int submissions:0
 property var fixture:({threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]})
 function check(v,m){if(!v){console.error("COMPOSER_ENTER_QML_FAIL "+m);Qt.exit(1)}}
 function write(value){var b=Test.T3Backend;b.patchDraft(b.selectedKey,{text:value,attempt:null,busy:false,error:""});panel.composerView.inputField.forceActiveFocus();Qt.callLater(function(){panel.composerView.inputField.cursorPosition=value.length})}
 TestCase{id:keys;when:false;parent:panel.composerView.inputField}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){Test.T3Backend.state=root.fixture}function onMessageSubmitted(key){root.submissions++}}
 Timer{interval:180;repeat:true;running:true;onTriggered:{var b=Test.T3Backend,c=panel.composerView,e=c.inputField
  if(phase===0){if(!b.viewStateLoaded)return;b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1}
  else if(phase===1){if(b.draft.loading)return;write("- Send this bullet");phase=2}
  else if(phase===2){keys.keyClick(Qt.Key_Return);check(submissions===1&&b.draft.attempt.text==="- Send this bullet","plain Enter sends even a list line without adding a newline");write("line one");phase=3}
  else if(phase===3){keys.keyClick(Qt.Key_Return,Qt.ShiftModifier);phase=4}
  else if(phase===4){check(b.draft.text==="line one\n"&&submissions===1,"Shift+Enter inserts newline without sending");write("replace this");phase=5}
  else if(phase===5){e.select(0,e.text.length);keys.keyClick(Qt.Key_Enter,Qt.ShiftModifier);phase=6}
  else if(phase===6){check(b.draft.text==="\n"&&submissions===1,"keypad Shift+Enter replaces selected text with a newline");write("keypad send");phase=7}
  else if(phase===7){keys.keyClick(Qt.Key_Enter);check(submissions===2,"keypad Enter sends");write("expanded line");panel.expandComposer();phase=8}
  else if(phase===8){e.forceActiveFocus();keys.keyClick(Qt.Key_Return);phase=9}
  else if(phase===9){check(b.draft.text==="expanded line\n"&&submissions===2,"expanded editor keeps plain Enter for multiline editing");keys.keyClick(Qt.Key_Return,Qt.ControlModifier);check(submissions===3,"Ctrl+Enter remains a send alias in expanded editor");panel.close();console.log("COMPOSER_ENTER_QML_PASS: Enter/keypad send, Shift+Enter newline and selection replacement, lists, expanded editing and Ctrl+Enter alias");Qt.quit()}
 }}
}
