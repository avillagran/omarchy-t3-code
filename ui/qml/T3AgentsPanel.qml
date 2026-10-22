import QtQuick 6.11
import QtQuick.Layouts 6.11
import QtQuick.Controls as Controls
import qs.Commons
import "../js/Subagents.js" as Subagents
// The open thread's agents (T3 Thread Details "Lineage"): its parent and the
// other subagents of that parent, the subagents it started, running first,
// and workflows with their members. Every row opens that agent's thread.
Item {
 id:root
 objectName:"t3AgentsPanel"
 required property var bridge
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 property real now:Date.now()
 property bool previousExpanded:false
 // Projection subagents, plus sub-threads the projection no longer lists.
 readonly property var all:{
  var subs=bridge.threadSubagents || [],known={}
  subs.forEach(function(s){if(s.childThreadId)known[s.childThreadId]=true})
  return subs.concat((bridge.childThreads || []).filter(function(t){return !known[t.id]}).map(Subagents.fromThread))
 }
 readonly property var running:all.filter(Subagents.live)
 readonly property var previous:all.filter(function(s){return !Subagents.live(s)})
 readonly property int previousFailed:previous.filter(function(s){return s.status==="failed"}).length
 readonly property var siblings:(bridge.siblingThreads || []).map(Subagents.fromThread)
 readonly property bool anyLive:running.length>0||siblings.some(Subagents.live)
 Timer{interval:1000;repeat:true;running:root.visible&&root.anyLive;onTriggered:root.now=Date.now()}
 function stop(s){if(s.workflow)bridge.stopWorkflow(s);else bridge.stopSubagent(s)}
 function members(s){return (s.workflowAgents || []).map(function(m){return {id:s.id+":agent:"+m.index,title:m.label || T3I18n.tr("Agent {n}",{n:(m.index||0)+1}),status:m.state==="queued"?"pending":m.state,childThreadId:m.childThreadId || null,startedAt:m.startedAt || 0,completedAt:m.startedAt&&m.durationMs?m.startedAt+m.durationMs:0,model:m.model,providerInstanceId:s.providerInstanceId,progress:m.phaseTitle || null,fromThread:true}})}
 component SectionTitle: Text { Layout.fillWidth:true;Layout.topMargin:Style.space(10);Layout.leftMargin:Style.space(6);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.6);font.pixelSize:Style.font.caption;font.weight:Font.Medium;textFormat:Text.PlainText;elide:Text.ElideRight }
 component AgentList: ColumnLayout {
  id:list
  property var items:[]
  property var panel:null
  Layout.fillWidth:true;spacing:Style.space(1)
  Repeater {
   model:list.panel?list.items:[]
   ColumnLayout {
    required property var modelData
    Layout.fillWidth:true;spacing:0
    T3SubagentRow{Layout.fillWidth:true;bridge:list.panel.bridge;subagent:modelData;now:list.panel.now;current:!!modelData.childThreadId&&modelData.childThreadId===list.panel.bridge.selectedThread;icon:modelData.workflow?"workflow":"";onOpened:list.panel.bridge.openSubagent(modelData);onStopRequested:list.panel.stop(modelData)}
    Repeater {
     model:list.panel.members(modelData)
     T3SubagentRow{required property var modelData;Layout.fillWidth:true;Layout.leftMargin:Style.space(22);bridge:list.panel.bridge;subagent:modelData;now:list.panel.now;current:!!modelData.childThreadId&&modelData.childThreadId===list.panel.bridge.selectedThread;onOpened:list.panel.bridge.openSubagent(modelData)}
    }
   }
  }
 }
 Flickable {
  anchors.fill:parent;anchors.margins:Style.space(8);contentHeight:body.implicitHeight;clip:true;boundsBehavior:Flickable.StopAtBounds
  Controls.ScrollBar.vertical:Controls.ScrollBar{policy:Controls.ScrollBar.AsNeeded}
  ColumnLayout {
   id:body;width:parent.width;spacing:Style.space(2)
   Text{objectName:"t3AgentsHeading";Layout.fillWidth:true;Layout.leftMargin:Style.space(6);text:T3I18n.tr("Agents")+(root.running.length?" · "+T3I18n.tr("{n} running",{n:root.running.length}):"");color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.Medium;textFormat:Text.PlainText}
   // Where this thread sits: its parent, and its place among the parent's subagents.
   SectionTitle{visible:!!root.bridge.parentThread;text:T3I18n.tr("Parent")}
   T3SubagentRow {
    objectName:"t3AgentsParent";visible:!!root.bridge.parentThread;Layout.fillWidth:true;bridge:root.bridge;icon:"corner-left-up";relation:T3I18n.tr("Parent agent")
    subagent:root.bridge.parentThread?({title:root.bridge.parentThread.title,status:"completed",childThreadId:root.bridge.parentThread.id}):({})
    onOpened:root.bridge.openParentThread()
   }
   RowLayout {
    visible:root.siblings.length>1;Layout.fillWidth:true;Layout.topMargin:Style.space(10);Layout.leftMargin:Style.space(6);spacing:Style.space(4)
    Text{Layout.fillWidth:true;text:T3I18n.tr("Subagent {index} of {count}",{index:root.bridge.siblingIndex+1,count:root.siblings.length});color:root.dim;font.pixelSize:Style.font.caption;font.weight:Font.Medium;textFormat:Text.PlainText}
    T3IconButton{objectName:"t3AgentsPrevious";name:"chevron-left";implicitWidth:Style.space(22);implicitHeight:implicitWidth;tooltipText:T3I18n.tr("Previous subagent");onClicked:root.bridge.openSibling(-1)}
    T3IconButton{objectName:"t3AgentsNext";name:"chevron-right";implicitWidth:Style.space(22);implicitHeight:implicitWidth;tooltipText:T3I18n.tr("Next subagent");onClicked:root.bridge.openSibling(1)}
   }
   AgentList{panel:root;visible:root.siblings.length>1;items:root.siblings}
   SectionTitle{visible:root.running.length>0;text:T3I18n.tr("Running")}
   AgentList{panel:root;objectName:"t3AgentsRunning";items:root.running}
   Rectangle {
    objectName:"t3AgentsPreviousToggle";visible:root.previous.length>0;Layout.fillWidth:true;Layout.topMargin:Style.space(8);implicitHeight:Style.space(28);radius:Style.space(6)
    color:prevHover.hovered?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05):"transparent"
    Accessible.role:Accessible.Button;Accessible.name:prevText.text
    HoverHandler{id:prevHover;cursorShape:Qt.PointingHandCursor}
    TapHandler{onTapped:root.previousExpanded=!root.previousExpanded}
    RowLayout {
     anchors{fill:parent;leftMargin:Style.space(6);rightMargin:Style.space(6)}spacing:Style.space(6)
     T3Icon{name:root.previousExpanded?"chevron-down":"chevron-right";implicitWidth:Style.space(13);tint:root.dim}
     Text{id:prevText;Layout.fillWidth:true;text:T3I18n.tr("Previous agents ({count})",{count:root.previous.length});color:root.dim;font.pixelSize:Style.font.caption;font.weight:Font.Medium;textFormat:Text.PlainText}
     Text{visible:root.previousFailed>0;text:T3I18n.tr("{n} failed",{n:root.previousFailed});color:T3Palette.failed;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
    }
   }
   AgentList{panel:root;objectName:"t3AgentsPreviousList";items:root.previousExpanded?root.previous:[]}
   Text{
    objectName:"t3AgentsEmpty";visible:!root.all.length&&!root.bridge.parentThread;Layout.fillWidth:true;Layout.margins:Style.space(8);wrapMode:Text.Wrap
    text:T3I18n.tr("Subagents and workflows this chat starts appear here. Open one to follow its own thread.");color:root.dim;font.pixelSize:Style.font.caption
   }
  }
 }
}
