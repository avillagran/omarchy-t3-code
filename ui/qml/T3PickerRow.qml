import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Controls.ItemDelegate {
 id:root
 property string iconName:"folder"
 property string description:""
 property string trailingText:""
 hoverEnabled:true
 padding:Style.space(12)
 implicitHeight:Style.space(description?56:42)
 opacity:enabled?1:0.45
 background:Rectangle {
  radius:Style.space(7)
  color:root.down?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.14):root.hovered||root.highlighted?Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.065):"transparent"
  border.width:root.activeFocus?1:0;border.color:Color.accent
  Behavior on color{ColorAnimation{duration:100}}
 }
 contentItem:RowLayout {
  spacing:Style.space(12)
  T3Icon{name:root.iconName;tint:root.highlighted?Color.accent:Color.popups.text;opacity:0.7}
  ColumnLayout {
   Layout.fillWidth:true;spacing:Style.space(3)
   Text{Layout.fillWidth:true;text:root.text;color:Color.popups.text;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;visible:!!root.description;text:root.description;color:Color.popups.text;opacity:0.5;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
  }
  Text{visible:!!root.trailingText;text:root.trailingText;color:Color.popups.text;opacity:0.5;font.pixelSize:Style.font.caption}
  T3Icon{name:"chevron-right";tint:Color.popups.text;opacity:0.35;implicitWidth:Style.space(14)}
 }
 HoverHandler{cursorShape:root.enabled?Qt.PointingHandCursor:Qt.ArrowCursor}
}
