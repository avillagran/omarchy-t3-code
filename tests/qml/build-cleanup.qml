import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
// Storage > This computer: per-kind idle days for automatic build output cleanup
// (worktrees 1 day, checkouts 7 days by default, each switchable off), a manual
// run and the last result.
ShellRoot {
 id:root
 function check(ok,message){if(!ok){console.error("BUILD_CLEANUP_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 QtObject {
  id:bridge
  property var storageCleanup:({busy:false,report:null})
  property var lowDiskThresholdGb:15
  property var buildCleanupWorktreeDays:1
  property var buildCleanupCheckoutDays:7
  property var buildCleanup:({running:false,error:"",report:null})
  property var saved:[]
  property int runs:0
  function storageCleanupAction(){}
  function setLowDiskThreshold(){}
  function setBuildCleanupDays(key,days){saved=saved.concat([[key,days]]);if(key==="buildCleanupCheckoutDays")buildCleanupCheckoutDays=days}
  function runBuildCleanup(){runs++;buildCleanup=({running:true,error:"",report:null})}
 }
 FloatingWindow {id:win;visible:true;implicitWidth:900;implicitHeight:900
  Flickable{anchors.fill:parent;contentHeight:page.implicitHeight
   Test.T3StorageSettings{id:page;width:parent.width;bridge:bridge;settings:({fields:[],storageScopes:[]})}}
 }
 TestCase { id:keys;parent:win.contentItem;when:false }
 Timer { interval:200;running:true;onTriggered:{
  var worktrees=keys.findChild(page,"preferenceDays_buildCleanupWorktreeDays"),checkouts=keys.findChild(page,"preferenceDays_buildCleanupCheckoutDays");
  check(worktrees&&checkouts,"both kinds are configurable");
  check(worktrees.value===1&&checkouts.value===7&&worktrees.to===365,"defaults: worktrees 1 day, checkouts 7 days");
  keys.mouseClick(keys.findChild(page,"preferenceEnabled_buildCleanupCheckoutDays"));
  check(bridge.saved.length===1&&bridge.saved[0][0]==="buildCleanupCheckoutDays"&&bridge.saved[0][1]===null,"checkouts can be switched off");
  keys.mouseClick(keys.findChild(page,"buildCleanupRun"));check(bridge.runs===1,"Clean build output now");
  check(!keys.findChild(page,"buildCleanupRun").enabled,"no second run while one is going");
  bridge.buildCleanup=({running:false,error:"",report:{freedBytes:3221225472,entries:[{dir:"/w/tree/target",bytes:3221225472}]}});
  check(keys.findChild(page,"buildCleanupResult").text===Test.T3I18n.tr("Last build cleanup: {count} folders, {freed} freed.",{count:1,freed:"3.0 GB"}),"last result: "+keys.findChild(page,"buildCleanupResult").text);
  console.log("BUILD_CLEANUP_QML_PASS: per-kind idle days with Off, manual run and last result");Qt.quit()
 }}
 Timer { interval:5000;running:true;onTriggered:{console.error("BUILD_CLEANUP_QML_FAIL timeout");Qt.exit(1)} }
}
