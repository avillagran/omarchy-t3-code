import QtQuick 6.11
import QtQuick.Layouts 6.11
import qs.Commons
// Top of a sub-thread (T3 "Subagent of …" divider): back to the parent and
// on to the parent's other subagents.
Rectangle {
 id:root
 objectName:"t3SubthreadBar"
 required property var bridge
 signal agentsRequested()
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 readonly property int count:(bridge.siblingThreads || []).length
 implicitHeight:Style.space(34)
 radius:Style.space(8);color:Qt.rgba(fg.r,fg.g,fg.b,0.04);border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.08)
 RowLayout {
  anchors{fill:parent;leftMargin:Style.space(10);rightMargin:Style.space(4)}spacing:Style.space(6)
  T3Icon{name:"bot";implicitWidth:Style.space(14);tint:root.dim}
  Text{
   objectName:"t3SubthreadParent";Layout.fillWidth:true;textFormat:Text.PlainText;elide:Text.ElideRight;font.pixelSize:Style.font.caption;color:root.dim
   text:root.bridge.parentThread?T3I18n.tr("Subagent of {title}",{title:root.bridge.parentThread.title || ""}):T3I18n.tr("Subagent")
  }
  Text{visible:root.count>1;text:(root.bridge.siblingIndex+1)+"/"+root.count;color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  T3IconButton{objectName:"t3SubthreadPrevious";visible:root.count>1;name:"chevron-left";implicitWidth:Style.space(24);implicitHeight:implicitWidth;tooltipText:T3I18n.tr("Previous subagent");onClicked:root.bridge.openSibling(-1)}
  T3IconButton{objectName:"t3SubthreadNext";visible:root.count>1;name:"chevron-right";implicitWidth:Style.space(24);implicitHeight:implicitWidth;tooltipText:T3I18n.tr("Next subagent");onClicked:root.bridge.openSibling(1)}
  T3IconButton{objectName:"t3SubthreadAgents";name:"bot";implicitWidth:Style.space(24);implicitHeight:implicitWidth;tooltipText:T3I18n.tr("Show agents");onClicked:root.agentsRequested()}
  T3IconButton{objectName:"t3SubthreadOpenParent";name:"corner-left-up";implicitWidth:Style.space(24);implicitHeight:implicitWidth;enabled:!!root.bridge.parentThread;tooltipText:T3I18n.tr("Open parent thread");onClicked:root.bridge.openParentThread()}
 }
}
