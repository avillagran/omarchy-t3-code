import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var fixtureState:null
 property int submissions:0
 function check(ok,msg){if(!ok){console.error("T3_SEND_SCROLL_QML_FAIL: "+msg);Qt.exit(1)}}
 function append(role,text){var b=Test.T3Backend;b.messagesAboutToChange();b.transcript=b.transcript.concat([{id:"added-"+phase,role:role,text:text,html:"<p>"+text+"</p>"}]);b.messagesUpdated()}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{
  target:Test.T3Backend
  function onSnapshotUpdated(){if(root.fixtureState)Test.T3Backend.state=root.fixtureState}
  function onMessageSubmitted(key){root.submissions++}
 }
 Timer{interval:250;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,c=panel.conversationView
  if(phase>0 && panel.restoringConversation)return
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};fixtureState=b.state;b.selectThread("fixture","one");var rows=[];for(var i=0;i<45;i++)rows.push({id:"row-"+i,role:"assistant",text:"Message "+i,html:"<p>Message "+i+"</p><p>More text</p>"});b.transcript=rows;panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;c.positionViewAtIndex(8,ListView.Beginning);panel.captureState();check(!c.atYEnd,"start from old messages");b.patchDraft(b.selectedKey,{text:"Test prompt",busy:false,attempt:null,attachments:[]});panel.composerView.inputField.forceActiveFocus();Qt.callLater(function(){panel.composerView.inputField.cursorPosition=b.draft.text.length});phase=101
  }else if(phase===101){keys.keyClick(Qt.Key_Return);phase=102
  }else if(phase===102){check(submissions===0&&b.draft.text.endsWith("\n"),"Enter inserts newline without sending");keys.keyClick(Qt.Key_Return,Qt.ControlModifier);check(submissions===1,"CtrlEnter emits accepted send intent");phase=2
  }else if(phase===2){check(c.atYEnd,"send immediately returns to end");append("user","Test prompt");phase=3
  }else if(phase===3){check(c.atYEnd,"new sent message stays visible");b.patchDraft(b.selectedKey,{text:"",busy:false,attempt:null});append("assistant",Array(22).fill("A growing response<br>").join(""));phase=4
  }else if(phase===4){check(c.atYEnd,"response growth and composer shrink follow end");c.forceActiveFocus();keys.keyClick(Qt.Key_Home);phase=5
  }else if(phase===5){check(panel.submissionFollowKey===""&&!c.atYEnd,"manual navigation releases follow");panel.captureState();append("assistant","New response while reading history");phase=6
  }else if(phase===6){check(!c.atYEnd,"incoming update respects manual reading");var y=c.contentY;b.patchDraft(b.selectedKey,{text:"",busy:false,attempt:null});b.submit();check(submissions===1&&c.contentY===y,"empty rejected submit does not jump");b.messageSubmitted("fixture/other");check(panel.submissionFollowKey==="","other thread cannot steal scroll");panel.close();console.log("T3_SEND_SCROLL_QML_PASS: accepted send, sent message, response growth, composer shrink, manual navigation and rejected/other-thread submissions");Qt.quit()}
 }}
}
