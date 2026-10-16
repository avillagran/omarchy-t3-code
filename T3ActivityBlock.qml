import QtQuick
import QtQuick.Layouts
import qs.Commons
Column {
 id:root
 objectName:"t3ActivityBlock"
 required property var bridge
 required property var activity
 property bool active:false
 property bool expanded:false
 width:parent.width
 spacing:Style.space(6)
 RowLayout {
  width:parent.width
  T3IconButton{objectName:"t3ActivityToggle";visible:(root.activity.entries || []).length>1;name:root.expanded?"chevron-down":"chevron-right";implicitWidth:Style.space(20);implicitHeight:implicitWidth;tooltipText:root.bridge.activityText(root.activity);onClicked:root.expanded=!root.expanded}
  WorkIndicator{visible:!!root.activity.active;active:visible&&root.active}
  T3ActivityText{Layout.fillWidth:true;active:root.active&&!!root.activity.active;tint:Color.popups.text;text:root.bridge.activityText(root.activity)}
 }
 ListView {
    id:entryList
    T3Wheel {flickable:entryList}
  objectName:"t3ActivityEntries"
  width:root.width;height:Math.min(Style.space(220),count*Style.space(28));clip:true;reuseItems:false
  model:root.expanded?(root.activity.entries || []):[]
  delegate:RowLayout {
   required property var modelData
   width:ListView.view.width;height:Math.max(Style.space(28),entryText.implicitHeight)
   T3Icon{name:modelData.active?"terminal":"check";implicitWidth:Style.space(12);tint:modelData.status==="failed"?Color.urgent:Color.muted}
   Text{id:entryText;Layout.fillWidth:true;text:root.bridge.activityText(modelData);color:Color.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  }
 }
}
