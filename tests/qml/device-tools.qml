import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
  id: root
  function check(ok,msg) { if (!ok) { console.error("DEVICE_TOOLS_QML_FAIL "+msg); Qt.exit(1) } }
  QtObject {
    id: backend
    property var deviceScreen: ({requestId:"android-screen",stage:"live",target:{platform:"android",hostKind:"local"}})
    property var deviceSettings: ({busy:false,error:"",detail:{settings:{appearance:"dark",textSize:"default",networkEnabled:true,reduceMotion:false},foregroundApp:{id:"example.app"}}})
    property var commands: []
    function deviceSetting(action) { if (action) commands=commands.concat([action]) }
  }
  TestCase { id: keys; when: false }
  FloatingWindow {
    visible: true; implicitWidth: 660; implicitHeight: 980
    Test.T3DeviceTools { id: tools; anchors.fill: parent; bridge: backend }
  }
  property int phase: 0
  function control(id) { var c=keys.findChild(tools,id); check(!!c,"missing "+id); return c }
  Timer { interval: 160; running: true; repeat: true; onTriggered: {
    if (phase===0) {
      check(!tools.blocked,"a live local device enables tools"); check(tools.foreground==="example.app","foreground app is shown")
      control("t3DeviceLaunchApp").clicked(); check(backend.commands[0].type==="launchApp" && backend.commands[0].appId==="example.app","launch uses foreground default")
      control("t3DeviceAppId").text="other.app"; control("t3DeviceTerminateApp").clicked(); check(backend.commands[1].appId==="other.app","explicit app overrides foreground")
      check(!control("t3DeviceSendPush").visible && !control("t3DeviceShake").visible,"iOS-only mutations are hidden on Android")
      control("t3DeviceLatitude").text="91"; control("t3DeviceLongitude").text="0"; check(!control("t3DeviceSetLocation").enabled,"invalid latitude cannot send")
      control("t3DeviceLatitude").text="0"; control("t3DeviceLongitude").text="-180"; check(control("t3DeviceSetLocation").enabled,"zero and boundary coordinates are valid")
      control("t3DeviceSetLocation").clicked(); check(backend.commands[2].latitude===0 && backend.commands[2].longitude===-180,"location action uses numeric coordinates")
      control("t3DeviceGrantPermission").clicked(); check(backend.commands[3].type==="setPermission" && backend.commands[3].permission==="camera" && backend.commands[3].decision==="grant","permission controls send native action")
      backend.deviceSettings=Object.assign({},backend.deviceSettings,{busy:true}); check(tools.blocked && !control("t3DeviceLaunchApp").enabled,"pending command disables further commands")
      backend.deviceScreen={requestId:"ios-screen",stage:"live",target:{platform:"ios",hostKind:"ssh"}}
      backend.deviceSettings={busy:false,error:"",detail:{settings:{appearance:"light",textSize:"large",liquidGlass:"clear",colorFilter:"none",voiceOver:false},foregroundApp:{id:"ios.app"}}}
      phase=1
    } else if (phase===1) {
      check(control("t3DeviceAppId").text==="" && control("t3DeviceLatitude").text==="","switching devices clears app/location drafts")
      check(control("t3DeviceSendPush").visible && control("t3DeviceResetPermission").visible && !control("t3DeviceOrientation").visible,"platform controls follow iOS")
      control("t3DevicePushText").text="Synthetic alert"; control("t3DeviceSendPush").clicked(); check(backend.commands[4].type==="sendPush" && backend.commands[4].appId==="ios.app","push targets selected app")
      control("t3DeviceLiquidGlass").activated(1); check(backend.commands[5].type==="setLiquidGlass" && backend.commands[5].value==="tinted","liquid glass native action")
      control("t3DeviceColorFilter").activated(4); check(backend.commands[6].type==="setColorFilter" && backend.commands[6].value==="blue-yellow","color filter native action")
      backend.deviceScreen=Object.assign({},backend.deviceScreen,{target:{platform:"ios",hostKind:"simlock"}}); check(tools.blocked,"pool-only hosts cannot run local host commands")
      check(backend.commands.every(function(c){return !c.hostId && !c.deviceId}),"actions never override the captured device target")
      console.log("DEVICE_TOOLS_QML_PASS: app, location, permissions, platform controls, serialization and captured targets"); Qt.quit()
    }
  } }
}
