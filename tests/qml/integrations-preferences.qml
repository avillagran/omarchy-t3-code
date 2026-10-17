import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property bool captured:false
 function check(v,m){if(!v){console.error("INTEGRATIONS_PREFERENCES_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 TestCase{id:keys;parent:win.contentItem;when:false}
 FloatingWindow{id:win;visible:true;implicitWidth:1200;implicitHeight:920;Test.T3Preferences{id:prefs;anchors.fill:parent;bridge:Test.T3Backend}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,page=keys.findChild(prefs,"t3Integrations")
  if(phase===0){if(!b.viewStateLoaded)return;b.state={environments:[{id:"fixture",name:"Fixture",state:"ready"}],projects:[],threads:[]};prefs.environment="fixture";prefs.page="integrations";check(prefs.supportsScope&&keys.findChild(prefs,"preferencesTab_integrations"),"native settings route is reachable");phase=1}
  else if(phase===1){check(page&&page.targets[0].environment==="fixture"&&!!b.deviceWatchRequest,"screen bound to actual bridge and scope");var old=b.deviceWatchRequest;prefs.selectEnvironment("*");check(prefs.page==="integrations"&&prefs.scope.targets.length===1,"all-environment selection remains on Integrations");page.editHost(null);phase=2}
  else if(phase===2){var editor=keys.findChild(prefs,"deviceHostEditorPopup");check(editor&&editor.opened,"native host editor opens in preferences overlay");editor.close();prefs.grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-integrations-preferences.png")});phase=3}
  else if(phase===3){if(!captured)return;prefs.page="general";check(!Object.keys(b.deviceView.sessions).length&&!b.deviceWatchRequest,"leaving route releases native device subscriptions");console.log("INTEGRATIONS_PREFERENCES_QML_PASS: reachable settings route, actual singleton bridge, scope selection, host editor overlay and route cleanup");Qt.quit()}
 }}
 Timer{interval:9000;running:true;onTriggered:{console.error("INTEGRATIONS_PREFERENCES_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
