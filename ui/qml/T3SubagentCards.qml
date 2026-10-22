import QtQuick 6.11
import QtQuick.Layouts 6.11
import qs.Commons
import "../js/Subagents.js" as Subagents
// Delegated work in the conversation (T3 SubagentTimelineLink / V2SubagentGroup):
// one card per subagent, grouped when a run starts several.
Rectangle {
 id:root
 required property var bridge
 property var cards:[]
 property bool active:false
 readonly property var subagents:cards.map(function(c){return Subagents.merge(c,root.bridge.threadSubagents)})
 readonly property bool group:subagents.length>1
 readonly property bool anyLive:subagents.some(Subagents.live)
 property bool userExpanded:false
 property bool userToggled:false
 readonly property bool expanded:!group||(userToggled?userExpanded:(subagents.length<=3||anyLive))
 readonly property var counts:Subagents.buckets(subagents)
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 property real now:Date.now()
 Timer{interval:1000;repeat:true;running:root.active&&root.anyLive;onTriggered:root.now=Date.now()}
 objectName:"t3SubagentCards"
 implicitHeight:column.implicitHeight+Style.space(8)
 radius:Style.space(8);color:Qt.rgba(fg.r,fg.g,fg.b,0.03);border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.1)
 function summary(){var c=counts,parts=[];if(c.working)parts.push(T3I18n.tr("{n} working",{n:c.working}));if(c.done)parts.push(T3I18n.tr("{n} done",{n:c.done}));if(c.failed)parts.push(T3I18n.tr("{n} failed",{n:c.failed}));if(c.stopped)parts.push(T3I18n.tr("{n} stopped",{n:c.stopped}));if(c.idle)parts.push(T3I18n.tr("{n} idle",{n:c.idle}));return parts.join(" · ")}
 function stop(s){if(s.workflow)bridge.stopWorkflow(s);else bridge.stopSubagent(s)}
 ColumnLayout {
  id:column;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(4)}spacing:Style.space(2)
  Rectangle {
   objectName:"t3SubagentGroupHeader";visible:root.group;Layout.fillWidth:true;implicitHeight:Style.space(28);radius:Style.space(6)
   color:headerHover.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05):"transparent"
   Accessible.role:Accessible.Button;Accessible.name:headerText.text
   HoverHandler{id:headerHover;cursorShape:Qt.PointingHandCursor}
   TapHandler{onTapped:{root.userExpanded=!root.expanded;root.userToggled=true}}
   RowLayout {
    anchors{fill:parent;leftMargin:Style.space(6);rightMargin:Style.space(6)}spacing:Style.space(8)
    T3Icon{name:"bot";implicitWidth:Style.space(15);tint:root.anyLive?T3Palette.working:root.dim}
    Text{id:headerText;text:T3I18n.tr("{n} subagents",{n:root.subagents.length});color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.Medium;textFormat:Text.PlainText}
    Text{Layout.fillWidth:true;text:root.summary();color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    T3Icon{name:root.expanded?"chevron-up":"chevron-down";implicitWidth:Style.space(14);tint:root.dim}
   }
  }
  Repeater {
   model:root.expanded?root.subagents:[]
   T3SubagentRow {
    required property var modelData
    Layout.fillWidth:true;bridge:root.bridge;subagent:modelData;now:root.now
    onOpened:root.bridge.openSubagent(modelData)
    onStopRequested:root.stop(modelData)
   }
  }
 }
}
