import QtQuick 6.11
import QtQuick.Controls as Controls
import qs.Commons
Controls.Switch {
 id:root
 padding:0;spacing:Style.space(8)
 implicitWidth:indicator.implicitWidth+(text.length?label.implicitWidth+spacing:0)
 implicitHeight:Style.space(28)
 hoverEnabled:true
 indicator:Rectangle {
  implicitWidth:Style.space(32);implicitHeight:Style.space(18)
  x:root.text.length?root.width-width:0;y:(root.height-height)/2
  radius:height/2
  color:root.checked?Color.accent:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.15)
  opacity:root.enabled?1:0.4
  border.width:root.activeFocus?1:0;border.color:Color.popups.text
  Rectangle{width:Style.space(12);height:width;radius:width/2;y:(parent.height-height)/2;x:root.checked?parent.width-width-Style.space(3):Style.space(3);color:root.checked?Color.popups.background:Color.popups.text;Behavior on x{NumberAnimation{duration:120;easing.type:Easing.OutCubic}}}
 }
 contentItem:Text{id:label;visible:root.text.length>0;text:root.text;color:Color.popups.text;font.pixelSize:Style.font.caption;verticalAlignment:Text.AlignVCenter;rightPadding:root.indicator.width+root.spacing;textFormat:Text.PlainText}
 HoverHandler{cursorShape:root.enabled?Qt.PointingHandCursor:Qt.ArrowCursor}
}
