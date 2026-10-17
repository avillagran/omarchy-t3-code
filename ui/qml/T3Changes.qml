import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Rectangle {
 id:root
 required property var summary
 property bool expanded:true
 signal openDiff(string filePath)
 width:parent.width
 implicitHeight:content.implicitHeight+Style.space(22)
 height:implicitHeight
 radius:Style.space(10);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.025)
 Column {
  id:content
  anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(11)}
  spacing:Style.space(6)
  RowLayout {
   width:parent.width
   T3IconButton{name:root.expanded?"chevron-down":"chevron-right";tooltipText:T3I18n.tr("Changed files");onClicked:root.expanded=!root.expanded}
   Text{text:(root.summary.files || []).length+" "+T3I18n.tr("Changed files");color:Color.popups.text;font.pixelSize:Style.font.caption}
   Text{text:"+"+root.summary.additions;color:T3Palette.done;font.pixelSize:Style.font.caption}
   Text{text:"−"+root.summary.deletions;color:Color.urgent;font.pixelSize:Style.font.caption}
   Item{Layout.fillWidth:true}
   T3IconButton{objectName:"t3OpenDiff";name:"external-link";tooltipText:T3I18n.tr("Open diff");onClicked:root.openDiff((root.summary.files || [])[0]?.path || "")}
  }
  ListView {
    id:filesList
    T3Wheel {flickable:filesList}
   width:content.width
   height:Math.min(Style.space(240),count*Style.space(25))
   clip:true;reuseItems:true
   model:root.expanded?(root.summary.files || []):[]
   Controls.ScrollBar.vertical:Controls.ScrollBar{}
   delegate:Item {
    id:fileRow;objectName:"t3ChangedFile_"+modelData.path
    required property var modelData
    width:ListView.view.width;height:Style.space(25)
    activeFocusOnTab:true;Accessible.role:Accessible.Button;Accessible.name:modelData.path
    Keys.onReturnPressed:root.openDiff(modelData.path)
    RowLayout {anchors.fill:parent
    T3Icon{name:"file";implicitWidth:Style.space(13);tint:modelData.kind==="added"?T3Palette.done:modelData.kind==="deleted"?Color.urgent:Color.muted}
    Text{Layout.fillWidth:true;text:modelData.path;color:Color.popups.text;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
    Text{text:"+"+(modelData.additions || 0);color:T3Palette.done;font.pixelSize:Style.font.caption}
    Text{text:"−"+(modelData.deletions || 0);color:Color.urgent;font.pixelSize:Style.font.caption}
    }
    MouseArea{anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.PointingHandCursor;onClicked:{fileRow.forceActiveFocus();root.openDiff(fileRow.modelData.path)}}
   }
  }
 }
}
