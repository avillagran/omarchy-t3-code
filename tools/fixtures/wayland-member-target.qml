import QtQuick
import QtQuick.Window
Window {
 id:root;visible:true;width:480;height:360;title:"Owned Toolkit Target";color:"#eeeeee";flags:Qt.Window|Qt.FramelessWindowHint
 readonly property string typed:field.text
 property int drops:0
 property int entered:0
 property string payload:""
 TextInput{id:field;x:40;y:30;width:380;height:50;focus:true;font.pixelSize:24;color:"#111111";selectByMouse:true}
 Rectangle{x:40;y:120;width:380;height:180;color:"#aaddaa"
  DropArea{anchors.fill:parent;onEntered:function(drag){root.entered++;drag.accepted=drag.hasText}onDropped:function(drop){if(drop.hasText){root.payload=drop.text;root.drops++;drop.acceptProposedAction()}}}
 }
}
