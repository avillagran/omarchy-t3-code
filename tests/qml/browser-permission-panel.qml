import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Client
ShellRoot {
 id:root
 property int phase:0
 function check(value,message){if(!value){console.error("BROWSER_PERMISSION_PANEL_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 FloatingWindow{visible:true;implicitWidth:32;implicitHeight:32}
 Loader{id:panelLoader;sourceComponent:Client.T3Panel{bridge:Client.T3Backend}}
 TestCase{id:keys;name:"browser-permission-panel";when:false;parent:panelLoader.item?panelLoader.item.panelPopup.surfaceItem:null}
 Timer{interval:250;running:true;repeat:true;onTriggered:{
  var panel=panelLoader.item,bridge=Client.T3Backend;if(!panel)return
  if(phase===0){check(!panel.opened,"panel initially hidden");bridge.browserPermission={requestId:"actual-panel",origin:"https://background.test",environment:"fixture"};phase=1}
  else if(phase===1){
   var popup=keys.findChild(panel,"t3BrowserClipboardPermission")
   check(panel.opened&&popup&&popup.visible&&panel.nativeModalOpen,"actual panel reveals global background-origin modal")
   check(panel.panelPopup.focusTarget===keys.findChild(panel,"browserClipboardBlock"),"actual panel chooses Block as its map-time keyboard target")
   // Sway restores ordinary-window focus when the existing Hyprland panel
   // changes Exclusive to OnDemand. Admit the fixture surface explicitly;
   // this is not acceptance of the compositor-specific 75-ms prime policy.
   panel.panelPopup.focusPrimed=false;phase=2
  }else if(phase===2){
   if(!keys.findChild(panel,"browserClipboardBlock").activeFocus)return
   keys.keyClick(Qt.Key_Return);phase=3
  }else if(phase===3){
   check(bridge.browserPermission===null,"Enter in the real admitted panel blocks the current request")
   bridge.browserPermission={requestId:"close-panel",origin:"https://close.test",environment:"fixture"};phase=4
  }else if(phase===4){
   panel.close();check(!panel.opened&&bridge.browserPermission===null,"explicit panel close blocks and clears current request")
   console.log("BROWSER_PERMISSION_PANEL_QML_PASS: actual panel background reveal, modal admission, explicit Wayland focus admission, Enter default Block and explicit close")
   Qt.quit()
  }
 }}
 Timer{interval:9000;running:true;onTriggered:{console.error("BROWSER_PERMISSION_PANEL_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
