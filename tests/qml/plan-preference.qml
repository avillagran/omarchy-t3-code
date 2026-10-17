import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 function check(ok,msg){if(!ok){console.error("PLAN_PREFERENCE_QML_FAIL "+msg);Qt.exit(1)}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,loader=keys.findChild(panel,"t3PreferencesView"),view=loader?loader.item:null
  if(phase===0){if(!b.viewStateLoaded||b.clientSettings.busy)return;b.state={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.selectThread("fixture","one");panel.open();panel.preferencesOpen=true;phase=1}
  else if(phase===1){if(!view||b.clientSettings.busy)return;var field=keys.findChild(view,"preference_planModeEnabled");check(field&&field.enabled&&!field.checked,"legacy Plan preference uses local client revision");field.checked=true;field.toggled();check(b.clientSettings.busy,"toggle waits for native client CAS save");phase=2}
  else if(phase===2){if(b.clientSettings.busy)return;check(b.planModeEnabled&&!b.clientSettings.error,"real isolated native client preference persisted");view.load();phase=3}
  else if(phase===3){if(b.clientSettings.busy)return;check(b.planModeEnabled,"client reload retains flag independently of unavailable server settings");panel.close();console.log("PLAN_PREFERENCE_QML_PASS: local native client CAS, explicit opt-in, readback and server-scope independence");Qt.quit()}
 }}
 Timer{interval:8000;running:true;onTriggered:{console.error("PLAN_PREFERENCE_QML_FAIL timeout");Qt.exit(1)}}
}
