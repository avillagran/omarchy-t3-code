import QtQuick 6.11
import QtQuick.Layouts 6.11
import QtQuick.Controls as Controls
import qs.Commons
import "../js/Subagents.js" as Subagents
// One subagent: provider mark with a status dot, title, its latest progress or
// result, elapsed time, and a way into its thread.
Rectangle {
 id:root
 required property var bridge
 property var subagent:({})
 property real now:Date.now()
 property bool current:false
 property bool showDetail:true
 property string relation:""
 property string icon:""
 signal opened()
 signal stopRequested()
 readonly property bool live:Subagents.live(subagent)
 readonly property bool openable:!!subagent.childThreadId
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 readonly property color tone:T3Palette.stateColor(Subagents.tone(subagent.status))
 readonly property var provider:(bridge.catalog.providers || []).find(function(p){return p.instanceId===root.subagent.providerInstanceId}) || ({instanceId:subagent.providerInstanceId || ""})
 readonly property string modelName:{var m=(bridge.catalog.models || []).find(function(m){return m.instanceId===root.subagent.providerInstanceId&&m.model===root.subagent.model});return m?(m.shortName || m.displayName || m.model):String(subagent.model || "")}
 readonly property string detailText:Subagents.detail(subagent)
 readonly property string time:Subagents.elapsed(subagent,now)
 objectName:"t3SubagentRow"
 implicitHeight:content.implicitHeight+Style.space(10)
 radius:Style.space(6)
 color:current?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.12):hover.hovered&&openable?Qt.rgba(fg.r,fg.g,fg.b,0.06):"transparent"
 Accessible.role:Accessible.Button;Accessible.name:[subagent.title,T3I18n.tr(Subagents.label(subagent.status)),time].filter(Boolean).join(" · ")
 HoverHandler{id:hover;cursorShape:root.openable?Qt.PointingHandCursor:Qt.ArrowCursor}
 TapHandler{enabled:root.openable;onTapped:root.opened()}
 Controls.ToolTip.visible:hover.hovered&&(!!root.subagent.prompt||!!root.modelName);Controls.ToolTip.delay:700
 Controls.ToolTip.text:[root.modelName,root.subagent.prompt].filter(Boolean).join("\n")
 RowLayout {
  id:content;anchors{left:parent.left;right:parent.right;verticalCenter:parent.verticalCenter;leftMargin:Style.space(6);rightMargin:Style.space(4)}spacing:Style.space(8)
  Item {
   Layout.preferredWidth:Style.space(18);Layout.preferredHeight:Style.space(18);Layout.alignment:Qt.AlignTop
   T3ProviderIcon{visible:!root.icon;anchors.fill:parent;anchors.margins:Style.space(1);provider:root.provider;tint:root.dim}
   T3Icon{visible:!!root.icon;anchors.fill:parent;anchors.margins:Style.space(1);name:root.icon;tint:root.dim}
   Rectangle{objectName:"t3SubagentStatusDot";visible:!root.icon;width:Style.space(7);height:width;radius:width/2;anchors.right:parent.right;anchors.bottom:parent.bottom;anchors.margins:-Style.space(1);color:root.tone;border.width:1;border.color:Color.popups.background}
  }
  ColumnLayout {
   Layout.fillWidth:true;spacing:Style.space(1)
   RowLayout {
    Layout.fillWidth:true;spacing:Style.space(6)
    Text{Layout.fillWidth:true;objectName:"t3SubagentTitle";text:root.subagent.title || T3I18n.tr("Subagent");color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.Medium;elide:Text.ElideRight;textFormat:Text.PlainText}
    Text{visible:!!root.relation;text:root.relation;color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
    WorkIndicator{active:root.live&&root.subagent.status==="running";visible:active;implicitWidth:Style.space(10);implicitHeight:implicitWidth;tint:root.tone}
    Text{objectName:"t3SubagentStatus";visible:!root.relation;text:T3I18n.tr(Subagents.label(root.subagent.status))+(root.time?" · "+root.time:"");color:root.subagent.status==="completed"?root.dim:root.tone;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   }
   Text{objectName:"t3SubagentDetail";visible:root.showDetail&&!!root.detailText;Layout.fillWidth:true;text:root.detailText;color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;maximumLineCount:2;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  }
  T3IconButton{objectName:"t3SubagentStop";visible:root.live&&hover.hovered&&(root.subagent.workflow||!!root.subagent.runId)&&!root.subagent.fromThread;name:"square";implicitWidth:Style.space(20);implicitHeight:implicitWidth;tooltipText:T3I18n.tr(root.subagent.workflow?"Stop workflow":"Stop subagent");enabled:!root.bridge.managementBusy;onClicked:root.stopRequested()}
  T3Icon{visible:root.openable;name:"chevron-right";implicitWidth:Style.space(14);tint:root.dim;Layout.alignment:Qt.AlignVCenter}
 }
}
