import QtQuick
import Quickshell
import QtTest
import Quickshell.Wayland
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int cycles:0;property int phase:0;property bool emptySource:false
 readonly property string image:"data:image/svg+xml,"+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><circle cx="12" cy="12" r="10" fill="white"/></svg>')
 Loader {id:loaded;active:false;sourceComponent:Component{PanelWindow{
  id:win;visible:false;screen:Quickshell.screens[0];implicitWidth:320;implicitHeight:160;exclusionMode:ExclusionMode.Ignore
  WlrLayershell.namespace:"omarchy-t3-dpi-fixture";WlrLayershell.keyboardFocus:WlrKeyboardFocus.None
  color:"#101010"
  function sampleIcon(){return providers.itemAt(0)}
  Grid{columns:10;spacing:4;Repeater{id:providers;model:30;delegate:Test.T3ProviderIcon{width:24;height:24;provider:({driverKind:"acpRegistry",registryIcon:root.emptySource?"":root.image})}}}
 }}}
 TestCase{id:probe;when:false}
 Timer{interval:35;running:true;repeat:true;onTriggered:{
  if(phase===0){root.emptySource=false;loaded.active=true;phase=1}
  else if(phase===1){loaded.item.visible=true;phase=2}
  else if(phase===2){root.emptySource=true;phase=3}
  else if(phase===3){var image=probe.findChild(loaded.item.sampleIcon(),"t3ProviderRegistry");if(!image||!image.layer.enabled){console.error("DPI_LIFECYCLE_QML_FAIL texture layer destroyed during source reload");Qt.exit(1)}loaded.active=false;if(++cycles>=50){console.log("DPI_LIFECYCLE_QML_PASS: scaled Wayland window, stable texture layer across reloads and 50 load/map/unload cycles");Qt.quit()}phase=0}
 }}
}
