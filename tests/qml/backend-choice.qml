import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("BACKEND_CHOICE_QML_FAIL: "+msg);Qt.exit(1)}}
 function stopped(){Test.T3Backend.backendStart={busy:false,error:"",status:""};Test.T3Backend.localBackend={owner:"stopped",running:false,canControl:true,choices:[{backend:"o3",available:true,canControl:true},{backend:"t3",available:true,canControl:true}]}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Timer{interval:200;repeat:true;running:true;onTriggered:{
  var b=Test.T3Backend,button=keys.findChild(panel,"startLocalBackend"),menu=keys.findChild(panel,"localBackendMenu")
  if(phase===0){if(!b.viewStateLoaded)return;stopped();b.selectedKey="local/fixture";b.patchDraft(b.selectedKey,{loading:false,text:"Keep the draft"});panel.open();phase=1
  }else if(phase===1){
   keys.mouseClick(button,button.width/2,button.height/2);check(menu.visible,"offline play opens a chooser");check(!b.backendStart.busy,"opening chooser never starts a server");check(keys.findChild(panel,"localBackendStartO3").enabled&&keys.findChild(panel,"localBackendStart").enabled,"both installed servers are choices");
   phase=10
  }else if(phase===10){phase=99;menu.contentItem.grabToImage(function(g){g.saveToFile("/var/tmp/o3-backend-choice.png");keys.keyClick(Qt.Key_Escape);root.phase=2})
  }else if(phase===2){check(!menu.visible&&!b.backendStart.busy,"Escape cancels without starting");b.startBackend();phase=3
  }else if(phase===3){check(menu.visible,"start request from another surface opens same chooser");var o3=keys.findChild(panel,"localBackendStartO3");keys.mouseClick(o3,o3.width/2,o3.height/2);phase=4
  }else if(phase===4){if(b.backendStart.busy||!b.backendStart.error)return;check(b.backendStart.error==="backend_not_installed","O3 selection reaches native O3 target without silently starting T3");check(b.draft.text==="Keep the draft","choice preserves prompt");stopped();b.controlBackend("start");phase=5
  }else if(phase===5){check(menu.visible,"connection start also asks");var t3=keys.findChild(panel,"localBackendStart");t3.forceActiveFocus();keys.keyClick(Qt.Key_Return);phase=6
  }else if(phase===6){if(b.backendStart.busy||!b.backendStart.error)return;check(b.backendStart.error==="t3_cli_missing","T3 choice reaches its own native launch path");stopped();b.localBackend={owner:"stopped",running:false,canControl:true,choices:[{backend:"o3",available:false,canControl:true},{backend:"t3",available:true,canControl:true}]};keys.mouseClick(button,button.width/2,button.height/2);phase=7
  }else if(phase===7){check(!keys.findChild(panel,"localBackendStartO3").enabled,"uninstalled O3 cannot be selected");check(keys.findChild(panel,"localBackendStartO3").text.indexOf(Test.T3I18n.tr("Not installed"))>=0,"uninstalled choice explains why");menu.close()
   b.localBackend={owner:"headless",running:true,canControl:true,backend:"o3",pid:1,choices:[{backend:"o3",available:true,canControl:true,running:true},{backend:"t3",available:true,canControl:true,running:false}]};phase=8
  }else if(phase===8){keys.mouseClick(button,button.width/2,button.height/2);phase=9
  }else if(phase===9){var toT3=keys.findChild(panel,"localBackendSwitchT3"),toO3=keys.findChild(panel,"localBackendSwitchO3")
   check(menu.visible&&toT3.visible&&toT3.enabled&&!toO3.visible,"running O3 offers switching to T3 only");check(!keys.findChild(panel,"localBackendStart").visible,"running server hides start choices")
   b.localBackend=Object.assign({},b.localBackend,{choices:[b.localBackend.choices[0],{backend:"t3",available:true,canControl:false,running:false}]});check(!toT3.enabled,"switch needs control of the target");b.localBackend=Object.assign({},b.localBackend,{choices:[b.localBackend.choices[0],{backend:"t3",available:true,canControl:true,running:false}]})
   keys.mouseClick(toT3,toT3.width/2,toT3.height/2);phase=11
  }else if(phase===11){if(b.backendStart.busy||!b.backendStart.error)return;check(b.backendStart.error==="backend_not_installed","switch reaches native switch and checks the target before stopping anything");panel.close();console.log("BACKEND_CHOICE_QML_PASS: explicit O3/T3 choices, Escape, keyboard, shared entry points, unavailable target, preserved draft and switching a running server");Qt.quit()}
 }}
}
