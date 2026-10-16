import QtQuick
import QtQuick.Window
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
import qs.Commons
ShellRoot {
 id:root
 Timer{interval:350;running:true;onTriggered:checks.runChecks()}
 function check(ok,msg){if(!ok){console.error("T3_APPROVALS_QML_FAIL: "+msg);Qt.exit(1)}}
 QtObject {
  id:bridge
  property string selectedKey:"fixture/thread"
  property var catalog:({canWrite:true})
  property var draft:({loading:false,loadError:""})
  property var approvalState:({})
  property var approvalOperations:({})
  property var approvals:[{requestId:"request",requestKind:"command",responseCapability:"live",detail:"printf '<hello>'\nsecond command",options:[{decision:"accept",label:"Approve"},{decision:"decline",label:"Decline"},{decision:"acceptAlways",label:"Allow permanently",warning:"Persistent access"}]}]
  readonly property var activeApproval:approvals.length?approvals[0]:null
  property string decision:""
  function respondApproval(d){decision=d}
 }
 Window {
  id:win;visible:true;width:760;height:500;color:Color.popups.background
  Test.T3Approvals{id:card;width:700;anchors.centerIn:parent;bridge:bridge}
  TestCase {
    id:checks;when:false
   function runChecks(){
    wait(200)
    root.check(card.visible&&findChild(card,"approvalDetail").text===bridge.activeApproval.detail,"full plain command detail")
    var approve=findChild(card,"approval-accept");mouseClick(approve);root.check(bridge.decision==="accept","primary action")
    var more=findChild(card,"approvalMore");mouseClick(more);wait(60)
    var always=card.optionsMenu.itemAt(0);root.check(always&&always.visible,"provider alternative is available in menu");mouseClick(always,always.width/2,always.height/2);root.check(bridge.decision==="acceptAlways","provider decision preserved")
    bridge.approvalState={attempt:{decision:"accept",requestId:"same"},responseError:"rpc_outcome_unknown"};wait(30)
    root.check(!findChild(card,"approval-accept")&&findChild(card,"approvalRetry").visible,"uncertain response locks alternative choices")
    mouseClick(findChild(card,"approvalRetry"));root.check(bridge.decision==="accept","retry original decision")
    bridge.approvals=[Object.assign({},bridge.activeApproval,{responseCapability:"not_resumable"})];wait(30)
    root.check(!card.canRespond&&!findChild(card,"approvalRetry").enabled,"dead provider cannot answer")
    bridge.approvals=[Object.assign({},bridge.activeApproval,{responseCapability:"live"})];bridge.catalog={canWrite:false};root.check(!card.canRespond,"read-only connection cannot decide")
    bridge.catalog={canWrite:true};bridge.approvalState={};card.width=330;wait(80)
    root.check(card.height>80&&card.height<400,"narrow layout remains bounded")
    win.contentItem.grabToImage(function(r){r.saveToFile("/tmp/t3-approvals.png")});wait(120)
    bridge.approvals=[];root.check(!card.visible,"resolved approval disappears")
    console.log("T3_APPROVALS_QML_PASS: native details, provider choices, retry, scope/capability guards and narrow layout");Qt.quit()
   }
  }
 }
}
