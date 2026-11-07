import QtQuick
import QtQuick.Window
Window {
 id:root;visible:true;width:480;height:360;title:"Owned Toolkit Source";color:"#ddddff";flags:Qt.Window|Qt.FramelessWindowHint
 property string typed:""
 property int drops:0
 property int entered:0
 property string payload:""
 Rectangle {
  id:tile;x:40;y:120;width:130;height:80;color:"#5555bb"
  Drag.active:mouse.drag.active
  Drag.source:tile
  Drag.hotSpot.x:10;Drag.hotSpot.y:10
  Drag.mimeData:({"text/plain":"owned-wayland-drop"})
  Drag.supportedActions:Qt.CopyAction
  Drag.proposedAction:Qt.CopyAction
  Drag.dragType:Drag.Automatic
  MouseArea{id:mouse;anchors.fill:parent;drag.target:tile}
 }
}
