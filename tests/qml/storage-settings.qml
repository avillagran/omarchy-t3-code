import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property var seen:[]
 function check(ok,message){if(!ok){console.error("STORAGE_SETTINGS_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 FloatingWindow { id:win;visible:true;implicitWidth:850;implicitHeight:400
  Column { anchors.fill:parent
   Test.T3PreferenceField { id:storage;width:parent.width;writable:true;field:({key:"storageCleanup.logsAfterDays",label:"Delete old rotated logs",kind:"days",value:null,minimum:1,maximum:3650,enabledValue:8});onChanged:function(v){root.seen=root.seen.concat([v])} }
   Test.T3PreferenceField { id:legacy;width:parent.width;writable:true;field:({key:"sidebarAutoSettleAfterDays",label:"Auto-settle after inactivity (days)",kind:"days",value:null,minimum:1,maximum:90});onChanged:function(v){root.seen=root.seen.concat([v])} }
  }
 }
 TestCase { id:keys;parent:win.contentItem;when:false }
 Timer { interval:150;running:true;onTriggered:{
  var toggle=keys.findChild(storage,"preferenceEnabled_storageCleanup.logsAfterDays");
  var days=keys.findChild(storage,"preferenceDays_storageCleanup.logsAfterDays");
  check(days.to===3650&&!days.enabled&&!toggle.checked,"original retention bounds and Off sentinel");
  keys.mouseClick(toggle);check(root.seen.length===1&&root.seen[0]===8,"shared native switch enables storage retention at original eight days");
  keys.mouseClick(keys.findChild(legacy,"preferenceEnabled_sidebarAutoSettleAfterDays"));check(root.seen[1]===3,"existing settlement control retains its own default");
  console.log("STORAGE_SETTINGS_QML_PASS: shared native retention control uses original storage bounds/default without changing settlement defaults");Qt.quit()
 }}
 Timer { interval:5000;running:true;onTriggered:{console.error("STORAGE_SETTINGS_QML_FAIL timeout");Qt.exit(1)} }
}
