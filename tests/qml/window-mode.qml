import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Io
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property int cycles:0;property int changes:0;property var composer:null;property int serial:0;property string address:"";property bool inspected:false
 function check(ok,msg){if(!ok){console.error("WINDOW_MODE_QML_FAIL "+msg);Qt.exit(1)}}
 Process{id:inspect;command:["hyprctl","-j","clients"];stdout:StdioCollector{waitForEnd:true;onStreamFinished:{var client=JSON.parse(text).find(function(c){return c.pid===Quickshell.processId&&c.title==="O3 Code"});root.check(!!client&&!client.xwayland,"Hyprland sees native managed window");if(client)root.address=client.address;root.inspected=true}}}
 Process{id:wmClose;command:["hyprctl","dispatch",'hl.dsp.window.close({ window = "address:'+root.address+'" })']}
 FloatingWindow{id:other;title:"T3 keyboard test other";visible:false;implicitWidth:200;implicitHeight:100;TextInput{focus:true;text:"Other window"}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend;onOpenedChanged:root.changes++}
 Timer{interval:250;repeat:true;running:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],environments:[]};b.selectThread("fixture","one");b.patchDraft(b.selectedKey,{loading:false,text:"Retain this draft",attachments:[]});b.transcript=Array.from({length:50},function(_,i){return {id:"m"+i,role:"assistant",text:"Historical message "+i+"\n"+"Read this older message. ".repeat(15),createdAt:i+1}});panel.open();phase=1
  }else if(phase===1){if(panel.restoringConversation)return;panel.followingTail=false;panel.conversationView.positionViewAtBeginning();phase=11
  }else if(phase===11){composer=panel.composerView.inputField;serial=b.requestedSerial;var button=keys.findChild(panel,"toggleWindowMode");check(!!button&&button.glyph==="\uf2d0","header uses Nerd Font window glyph");button.clicked();phase=15
  }else if(phase===15){inspect.running=true;phase=2
  }else if(phase===2){if(!inspected)return;if(panel.restoringConversation)return;check(panel.opened&&panel.detached&&panel.normalWindow.visible,"normal window replaces panel");check(changes===1&&b.requestedSerial===serial,"switch does not close logical panel or restart subscription");check(panel.composerView.inputField===composer&&composer.plainText==="Retain this draft","same editor and draft retained");check(b.viewState.detached===true,"mode saved");check(!panel.followingTail&&panel.conversationView.atYBeginning,"reading position survives detach");check(panel.shortcutActive,"native window receives shortcuts while focused "+JSON.stringify({app:Quickshell.appId,toplevel:panel.managedToplevel?{app:panel.managedToplevel.appId,active:panel.managedToplevel.activated}:null}));other.visible=true;phase=21
  }else if(phase===21){var target=ToplevelManager.toplevels.values.find(function(t){return t.title===other.title});if(!target)return;target.activate();phase=22
  }else if(phase===22){check(!panel.shortcutActive,"unfocused normal window cannot consume global send");other.visible=false;panel.setDetached(false);phase=3
  }else if(phase===3){if(panel.restoringConversation)return;check(panel.opened&&!panel.normalWindow.visible&&!panel.detached,"reattach to bar");check(changes===1&&composer.plainText==="Retain this draft","reattach preserves logical visibility and draft");cycles++;if(cycles<4){panel.setDetached(true);phase=2;return}panel.setDetached(true);panel.close();panel.open();phase=4
  }else if(phase===4){check(panel.detached&&panel.normalWindow.visible,"reopen retains native mode");inspected=false;inspect.running=true;phase=45
  }else if(phase===45){if(!inspected)return;wmClose.running=true;phase=5
  }else if(phase===5){check(!panel.opened&&!panel.normalWindow.visible,"close hides native surface "+JSON.stringify({opened:panel.opened,visible:panel.normalWindow.visible,address:root.address}));panel.open();phase=6
  }else if(phase===6){check(panel.normalWindow.visible,"reopen after compositor close");panel.close();console.log("WINDOW_MODE_QML_PASS: detach/reattach, same editor/draft, no subscription restart, mode persistence and close/reopen");Qt.quit()}
 }}
}
