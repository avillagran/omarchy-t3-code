import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
ColumnLayout {
 id:root
 property string title:""
 property string icon:"settings"
 property Component body
 spacing:Style.space(9)
 RowLayout{Layout.fillWidth:true;Layout.leftMargin:Style.space(12);Layout.rightMargin:Style.space(12);spacing:Style.space(8)
  T3Icon{name:root.icon;tint:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.55);implicitWidth:Style.space(14)}
  Text{text:T3I18n.tr(root.title);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.65);font.pixelSize:Style.font.caption;Layout.fillWidth:true;textFormat:Text.PlainText}
 }
 Rectangle{objectName:"preferenceSectionCard";Layout.fillWidth:true;implicitHeight:sectionLoader.height;radius:Style.space(11);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.025);border.width:1;border.color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.085)
  Loader{id:sectionLoader;anchors.left:parent.left;anchors.right:parent.right;active:root.visible;sourceComponent:root.body}
 }
}
