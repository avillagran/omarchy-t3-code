import QtQuick 6.11
import qs.Commons
Item{
 id:root
 required property var counts
 property bool stale:false
 readonly property real pillWidth:Style.space(Math.max(counts.working,counts.done)>99?23:Math.max(counts.working,counts.done)>9?18:14)
 implicitWidth:Style.space(25)+pillWidth+(counts.waiting>0?Style.space(3+ (counts.waiting>99?23:counts.waiting>9?18:14)):0)
 implicitHeight:Style.bar.statusSlot
 Item{
  x:0;anchors.verticalCenter:parent.verticalCenter;width:Style.space(24);height:width
  WorkIndicator{anchors.centerIn:parent;implicitWidth:Style.space(24);visible:root.counts.working>0;active:visible;tint:T3Palette.working}
  T3Icon{anchors.centerIn:parent;name:"t3-logo";implicitWidth:Style.space(18);tint:root.stale?Color.muted:Color.accent}
 }
 Repeater{
  model:[{key:"working",count:root.counts.working,tint:T3Palette.working,top:true},{key:"done",count:root.counts.done,tint:T3Palette.done,top:false}]
  Rectangle{
   required property var modelData
   objectName:"t3Badge_"+modelData.key
   x:Style.space(25);y:modelData.top?Math.max(0,(root.height-Style.space(23))/2):root.height-Math.max(0,(root.height-Style.space(23))/2)-height
   width:root.pillWidth;height:Style.space(10);radius:Style.space(2.5);color:modelData.tint
   Text{anchors.centerIn:parent;text:modelData.count>99?"99+":modelData.count;color:Color.popups.background;font.pixelSize:Style.space(8);font.bold:true}
  }
 }
 Rectangle{
  objectName:"t3Badge_waiting";visible:root.counts.waiting>0
  x:Style.space(28)+root.pillWidth;anchors.verticalCenter:parent.verticalCenter
  width:Style.space(root.counts.waiting>99?23:root.counts.waiting>9?18:14);height:Style.space(10);radius:Style.space(2.5);color:T3Palette.waiting
  Text{anchors.centerIn:parent;text:root.counts.waiting>99?"99+":root.counts.waiting;color:Color.popups.background;font.pixelSize:Style.space(8);font.bold:true}
 }
}
