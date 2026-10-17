import QtQuick
import QtQuick.Window
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/NativeKeybindings.js" as Native
import "UnderTest/ui/js/ShortcutRouting.js" as Routes
ShellRoot{
 id:root;property int phase:0;property var fired:[];property int attempts:0
 function check(ok,msg){if(!ok){console.error("SHORTCUT_ROUTING_QML_FAIL "+msg);Qt.exit(1)}}
 function compile(rules){return Native.nativeKeys.compileResolvedKeybindingsConfig(rules)}
 FloatingWindow{id:win;title:"ot3 shortcut routing fixture";visible:true;implicitWidth:500;implicitHeight:250
  Item{id:area;anchors.fill:parent;focus:true
   Rectangle{anchors.fill:parent;color:"#101418"}
   Text{anchors.centerIn:parent;text:"Native shortcut routing fixture";color:"white"}
   Test.T3ShortcutRouter{id:router;active:true;primary:root.compile([{command:"sidebar.toggle",key:"mod+k"},{command:"chat.find",key:"mod+k",when:"composerFocus"},{command:"preview.toggle",key:"mod+x"},{command:"sidebar.toggle",key:"mod+x"},{command:"preview.toggle",key:"mod+x"},{command:"composer.sendAndNewThread",key:"mod+alt+enter"}]);scripts:root.compile([{command:"script.check.run",key:"mod+j"},{command:"script.other.run",key:"mod+x"}]);context:({composerFocus:true});supports:function(command){return command==="sidebar.toggle"||command==="chat.find"||command.indexOf("script.")===0||command==="composer.sendAndNewThread"};onTriggered:function(command){root.fired=root.fired.concat([command])}}
  }
 }
 TestCase{id:keys;parent:area;when:false}
 Process{id:focusOwn;command:["hyprctl","dispatch",'hl.dsp.focus({ window = "pid:'+Quickshell.processId+'" })'];stdout:StdioCollector{onStreamFinished:if(root.attempts++<3)console.log("FOCUS_RESULT",text.trim(),area.Window.active)}}
 Timer{interval:160;repeat:true;running:true;onTriggered:{var k=Native.nativeKeys
  if(phase===0){var top=ToplevelManager.toplevels.values.find(function(t){return t.title===win.title});if(!area.Window.active){if(!focusOwn.running)focusOwn.running=true;return}area.forceActiveFocus();check(router.routes.some(function(r){return r.command==="chat.find"}),"native last matching condition wins");check(!router.routes.some(function(r){return r.command==="sidebar.toggle"}),"unsupported last claimant still blocks older supported command");check(!router.routes.some(function(r){return r.command==="script.other.run"}),"unsupported primary app claimant blocks active script");check(router.routes.some(function(r){return r.command==="script.check.run"}),"active environment script fallback");keys.keyClick(Qt.Key_K,Qt.ControlModifier);phase=1}
  else if(phase===1){check(fired[0]==="chat.find","actual Qt shortcut resolves native precedence");keys.keyClick(Qt.Key_J,Qt.ControlModifier);phase=2}
  else if(phase===2){check(fired[1]==="script.check.run","actual active script registration");keys.keyClick(Qt.Key_Return,Qt.ControlModifier|Qt.AltModifier);phase=3}
  else if(phase===3){check(fired[2]==="composer.sendAndNewThread","DOM Enter maps to actual Qt Return");router.context={composerFocus:false};phase=4}
  else if(phase===4){keys.keyClick(Qt.Key_K,Qt.ControlModifier);phase=5}
  else if(phase===5){check(fired[3]==="sidebar.toggle","context changes rebuild effective registrations");router.primary=compile([{command:"sidebar.toggle",key:"mod+enter"},{command:"sidebar.toggle",key:"mod+shift+enter"}]);check(router.routes.length===2&&router.routes.every(function(r){return r.source==="active"}),"only active scripts remain with reserved send keys");check(!router.claimsSequence("Ctrl+Return"),"native bindings cannot steal user send key");check(k.projectScriptIdFromCommand("script.check.run")==="check"&&k.projectScriptIdFromCommand("script.bad/path.run")===null,"native script schema predicate");console.log("SHORTCUT_ROUTING_QML_PASS: original reverse priority/conditions, unsupported claimers, primary versus active scripts, actual Qt keys/Enter, dynamic context and reserved user send keys");Qt.quit()}
 }}
 Timer{interval:8000;running:true;onTriggered:{console.error("SHORTCUT_ROUTING_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
