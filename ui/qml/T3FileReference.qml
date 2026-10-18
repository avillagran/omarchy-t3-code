import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Rectangle {
 id:root
 objectName:"t3FileReference"
 required property string fileName
 property string filePath:fileName
 signal clicked()
 implicitWidth:Math.min(Style.space(420),label.implicitWidth+Style.space(64));implicitHeight:Style.space(28)
 radius:Style.space(5);color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,hover.containsMouse?0.15:0.07)
 border.width:1;border.color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.3)
 activeFocusOnTab:true;Accessible.role:Accessible.Button;Accessible.name:filePath
 Keys.onReturnPressed:clicked();Keys.onSpacePressed:clicked()
 MouseArea{id:hover;anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.PointingHandCursor;onClicked:root.clicked()}
 RowLayout{
  anchors.fill:parent;anchors.margins:Style.space(5);spacing:Style.space(5)
  T3Icon{name:"file";tint:Color.accent;implicitWidth:Style.space(14);implicitHeight:implicitWidth}
  Text{id:label;Layout.fillWidth:true;text:root.fileName;textFormat:Text.PlainText;color:Color.accent;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle}
  T3IconButton{objectName:"t3PreviewFileReference";name:"eye";glyph:"\uf06e";glyphScale:0.85;implicitWidth:Style.space(22);implicitHeight:Style.space(22);tooltipText:T3I18n.tr("Preview")+": "+root.fileName;onClicked:root.clicked()}
 }
 Controls.ToolTip.visible:hover.containsMouse;Controls.ToolTip.text:filePath;Controls.ToolTip.delay:500
}
