import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Rectangle{
 id:root
 required property var bridge
 property bool initialized:false
 property bool expanded:!!bridge.viewState.tasksExpanded
 readonly property var tasks:bridge.tasks || ({items:[],total:0,completed:0,current:""})
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.55)
 function duration(item){if(typeof item.durationMs!=="number")return item.status==="running"?T3I18n.tr("now"):"";var s=Math.floor(item.durationMs/1000);return s<60?s+"s":Math.floor(s/60)+"m "+s%60+"s"}
 function tint(status){return status==="completed"?T3Palette.done:status==="running"?Color.accent:root.dim}
 Component.onCompleted:initialized=true
 onExpandedChanged:if(initialized && bridge.viewState.tasksExpanded!==expanded)bridge.rememberView({tasksExpanded:expanded})
 visible:tasks.total>0
 implicitHeight:body.implicitHeight+Style.space(8)
 radius:Style.space(10);color:Qt.rgba(fg.r,fg.g,fg.b,0.035);border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.09)
 ColumnLayout{
  id:body
  anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(4)}
  spacing:Style.space(2)
  Rectangle{
   id:header;objectName:"t3TasksToggle"
   Layout.fillWidth:true;Layout.preferredHeight:Style.space(32)
   radius:Style.space(5);color:hover.containsMouse?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04):"transparent"
   activeFocusOnTab:true;Accessible.role:Accessible.Button
   Accessible.name:T3I18n.tr("Tasks")+" · "+root.tasks.completed+"/"+root.tasks.total+" · "+root.tasks.current
   border.width:activeFocus?1:0;border.color:Color.accent
   Keys.onReturnPressed:root.expanded=!root.expanded
   Keys.onSpacePressed:root.expanded=!root.expanded
   RowLayout{
    anchors.fill:parent;anchors.margins:Style.space(6);spacing:Style.space(8)
    T3Icon{name:"list-todo";tint:root.dim}
    Text{text:T3I18n.tr("Tasks");color:root.dim;font.pixelSize:Style.font.caption}
    Text{objectName:"t3TasksCurrent";Layout.fillWidth:true;text:root.tasks.current;color:root.fg;opacity:0.8;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    Text{objectName:"t3TasksProgress";text:root.tasks.completed+"/"+root.tasks.total;color:root.tasks.completed>=root.tasks.total?T3Palette.done:root.dim;font.pixelSize:Style.font.caption}
    RowLayout{
     objectName:"t3TaskSegments";visible:root.tasks.total>1&&root.tasks.total<=10&&root.width>=Style.space(560)
     Layout.preferredWidth:Style.space(80);spacing:Style.space(2)
     Repeater{model:root.tasks.items;Rectangle{required property var modelData;Layout.fillWidth:true;implicitHeight:Style.space(3);radius:height/2;color:root.tint(modelData.status);opacity:modelData.status==="pending"?0.45:1}}
    }
    T3Icon{name:root.expanded?"chevron-up":"chevron-down";tint:root.dim}
   }
   MouseArea{id:hover;anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.PointingHandCursor;onClicked:root.expanded=!root.expanded}
   Controls.ToolTip.visible:hover.containsMouse;Controls.ToolTip.text:header.Accessible.name
  }
  T3Reveal{
   id:listReveal;Layout.fillWidth:true;shown:root.expanded
   size:Math.min(list.contentHeight,Style.space(180))
  ListView{
    T3Wheel {flickable:list}
   id:list;objectName:"t3TaskList"
   anchors.fill:parent
   clip:true;reuseItems:false;cacheBuffer:0
   model:root.tasks.items;spacing:Style.space(3)
   Controls.ScrollBar.vertical:Controls.ScrollBar{}
   delegate:Item{
    required property var modelData
    width:list.width;height:Math.max(Style.space(26),label.implicitHeight+Style.space(8))
    Accessible.role:Accessible.ListItem
    Accessible.name:(modelData.status==="completed"?T3I18n.tr("Completed"):modelData.status==="running"?T3I18n.tr("Running"):T3I18n.tr("Pending"))+": "+modelData.text
    RowLayout{
     anchors.fill:parent;anchors.leftMargin:Style.space(6);anchors.rightMargin:Style.space(6);spacing:Style.space(8)
     T3Icon{Layout.alignment:Qt.AlignTop;Layout.topMargin:Style.space(4);name:modelData.status==="completed"?"check":modelData.status==="running"?"circle-dot":"circle";tint:root.tint(modelData.status);implicitWidth:Style.space(14)}
     Text{id:label;Layout.fillWidth:true;Layout.alignment:Qt.AlignTop;text:modelData.text;color:root.fg;opacity:modelData.status==="completed"?0.55:modelData.status==="running"?0.9:0.7;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
     Text{Layout.preferredWidth:Style.space(52);Layout.alignment:Qt.AlignTop;text:root.duration(modelData);color:root.dim;opacity:0.8;font.pixelSize:Style.font.caption;horizontalAlignment:Text.AlignRight}
    }
   }
  }
  }
 }
}
