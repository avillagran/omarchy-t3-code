import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var answers:[]
 function check(value,message){if(!value){console.error("BROWSER_PERMISSION_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 QtObject {
  id:testBridge
  property var browserPermission:null
  function answerBrowserPermission(allow){root.answers.push({requestId:browserPermission.requestId,allow:allow});browserPermission=null}
 }
 FloatingWindow {
  visible:true;implicitWidth:700;implicitHeight:400
  Item{id:host;anchors.fill:parent
   Test.T3BrowserPermission{id:prompt;bridge:testBridge;popupHost:host}
   TestCase{id:keys;name:"browser-permission";when:windowShown}
  }
 }
 Timer {interval:250;running:true;repeat:true;onTriggered:{
  if(phase===0){testBridge.browserPermission={requestId:"one",origin:"https://example.test"};phase=1}
  else if(phase===1){check(prompt.opened,"background prompt opens without browser selection");check(keys.findChild(prompt,"browserClipboardBlock").activeFocus,"Block has default focus");keys.keyClick(Qt.Key_Return);phase=2}
  else if(phase===2){check(answers.length===1&&!answers[0].allow,"Enter blocks by default");testBridge.browserPermission={requestId:"two",origin:"https://second.test"};phase=3}
  else if(phase===3){keys.findChild(prompt,"browserClipboardAllow").clicked();phase=4}
  else if(phase===4){check(answers.length===2&&answers[1].requestId==="two"&&answers[1].allow,"explicit Allow targets current origin request");testBridge.browserPermission={requestId:"three",origin:"https://escape.test"};phase=5}
  else if(phase===5){keys.keyClick(Qt.Key_Escape);phase=6}
  else if(phase===6){check(answers.length===3&&!answers[2].allow,"Escape blocks");testBridge.browserPermission={requestId:"four",origin:"https://closed.test"};phase=7}
  else if(phase===7){testBridge.browserPermission=null;phase=8}
  else if(phase===8){check(!prompt.opened&&answers.length===3,"source cancellation does not fabricate a user choice");console.log("BROWSER_PERMISSION_QML_PASS: global modal, default Block, explicit Allow, Escape, current request identity and cancellation");Qt.quit()}
 }}
 Timer{interval:7000;running:true;onTriggered:{console.error("BROWSER_PERMISSION_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
