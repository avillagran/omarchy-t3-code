import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("BACKEND_OWNER_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Timer{interval:200;repeat:true;running:true;onTriggered:{var b=Test.T3Backend,button=keys.findChild(panel,"startLocalBackend"),menu=keys.findChild(panel,"localBackendMenu")
  if(phase===0){if(!b.viewStateLoaded)return;b.localBackend={owner:"gui",running:true,canControl:false,installed:false};panel.open();phase=1
  }else if(phase===1){check(!button.visible,"GUI owner hides lifecycle button");check(keys.findChild(panel,"localBackendGuiIndicator").visible,"GUI ownership indicator visible");b.localBackend={owner:"stopped",running:false,canControl:true,installed:false};phase=2
  }else if(phase===2){check(button.visible&&button.enabled&&button.name==="play","offline offers start");b.localBackend={owner:"headless",running:true,canControl:true,installed:true};phase=3
  }else if(phase===3){check(button.visible&&button.enabled&&button.name==="server","headless has local controls");keys.mouseClick(button,button.width/2,button.height/2);phase=4
  }else if(phase===4){check(menu.visible&&menu.width>100&&menu.height>60,"lifecycle menu has visible dimensions");check(keys.findChild(panel,"localBackendStop").enabled&&keys.findChild(panel,"localBackendRestart").enabled,"headless allows stop/restart");check(!keys.findChild(panel,"localBackendStart").enabled,"running headless cannot start twice");b.localBackend={owner:"gui",running:true,canControl:false,installed:true};phase=5
  }else if(phase===5){check(!menu.visible,"GUI takeover closes lifecycle menu");check(!keys.findChild(panel,"localBackendStop").enabled&&!keys.findChild(panel,"localBackendRestart").enabled,"ownership change disables open actions");menu.close();b.localBackend={owner:"headless-manual",running:true,canControl:true,installed:false};phase=6
  }else if(phase===6){check(button.visible&&button.enabled,"verified manual headless exposes controls");b.localBackend={owner:"unknown",running:true,canControl:false,installed:false};phase=7
  }else if(phase===7){check(button.visible&&!button.enabled,"unverified process cannot be controlled");panel.close();console.log("BACKEND_OWNER_QML_PASS: GUI read-only, offline start, headless menu, live ownership change and unknown process guard");Qt.quit()}
 }}
}
