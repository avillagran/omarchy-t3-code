import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Rectangle {
 id:root
 required property var bridge
 property bool initialized:false
 property bool expanded:bridge.viewState.queueExpanded !== false
 property int dropIndex:-1
 property string draggedRun:""
 readonly property var items:bridge.queuedItems || []
 readonly property bool busy:!!bridge.queueOperation.command || !!bridge.draft.busy || !!bridge.draft.attempt
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.55)
 signal editSelected()
 function move(runId,beforeRunId){if(!busy)bridge.queueAction("reorder",{runId:runId,beforeRunId:beforeRunId || null})}
 Component.onCompleted:initialized=true
 onExpandedChanged:if(initialized && bridge.viewState.queueExpanded!==expanded)bridge.rememberView({queueExpanded:expanded})
 visible:items.length>0 || !!bridge.queueOperation.error
 implicitHeight:body.implicitHeight+Style.space(8)
 radius:Style.space(10);color:Qt.rgba(fg.r,fg.g,fg.b,0.035);border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.09)
 ColumnLayout{
  id:body;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(4)} spacing:Style.space(2)
  RowLayout{
   Layout.fillWidth:true;spacing:Style.space(5)
   T3IconButton{name:"list-ordered";tooltipText:T3I18n.tr("Queued messages");onClicked:root.expanded=!root.expanded}
   Text{Layout.fillWidth:true;text:root.bridge.queue.held?T3I18n.tr("Queued · Paused"):T3I18n.tr("Queued");color:root.dim;font.pixelSize:Style.font.caption}
   Text{text:root.items.length;color:root.dim;font.pixelSize:Style.font.caption}
   Controls.Button{objectName:"t3QueueResume";visible:!!root.bridge.queue.held;enabled:!root.busy;text:T3I18n.tr("Resume");onClicked:root.bridge.queueAction("resume",{})}
   T3IconButton{objectName:"t3QueueToggle";name:root.expanded?"chevron-up":"chevron-down";tooltipText:T3I18n.tr("Expand/collapse queue");onClicked:root.expanded=!root.expanded}
  }
  Text{Layout.fillWidth:true;visible:!!root.bridge.queueOperation.error;text:T3I18n.error(root.bridge.queueOperation.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  Controls.Button{visible:!!root.bridge.queueOperation.command && !root.bridge.queueOperation.busy;enabled:!root.bridge.queueOperation.busy;text:T3I18n.tr("Retry same action");onClicked:root.bridge.queueAction(null,{})}
  ListView{
    T3Wheel {flickable:list}
   id:list;objectName:"t3QueuedMessages"
   Layout.fillWidth:true;Layout.preferredHeight:Math.min(contentHeight,Style.space(144))
   visible:root.expanded&&root.items.length>0;clip:true;reuseItems:true;cacheBuffer:0
   model:root.items;spacing:Style.space(2)
   Controls.ScrollBar.vertical:Controls.ScrollBar{}
   delegate:Rectangle{
    id:entry;required property var modelData;required property int index
    width:list.width;height:Style.space(42);radius:Style.space(5)
    color:root.bridge.draft.queueEdit && root.bridge.draft.queueEdit.runId===modelData.runId?Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.13):"transparent"
    opacity:root.draggedRun===modelData.runId?0.5:1
    Rectangle{anchors.top:parent.top;width:parent.width;height:Style.space(2);visible:root.dropIndex===index;color:Color.accent}
    RowLayout{
     anchors.fill:parent;anchors.margins:Style.space(3);spacing:Style.space(4)
     Item{
      Layout.preferredWidth:Style.space(23);Layout.fillHeight:true
      activeFocusOnTab:root.bridge.queue.canReorder&&!root.busy&&!modelData.pending
      Accessible.role:Accessible.Button;Accessible.name:T3I18n.tr("Reorder message")
      Keys.onUpPressed:if(index>0)root.move(modelData.runId,root.items[index-1].runId)
      Keys.onDownPressed:if(index<root.items.length-1)root.move(modelData.runId,index+2<root.items.length?root.items[index+2].runId:null)
      T3Icon{anchors.centerIn:parent;name:"grip-vertical";tint:root.dim;opacity:root.bridge.queue.canReorder?1:0.3}
      MouseArea{
       objectName:"queueGrip_"+modelData.runId
       anchors.fill:parent;enabled:root.bridge.queue.canReorder&&!root.busy
       cursorShape:pressed?Qt.ClosedHandCursor:Qt.OpenHandCursor
       property real startY:0
       onPressed:function(mouse){startY=mapToGlobal(mouse.x,mouse.y).y;root.draggedRun=modelData.runId}
       onPositionChanged:function(mouse){if(!pressed)return;var y=mapToItem(list,mouse.x,mouse.y).y+list.contentY;root.dropIndex=Math.max(0,Math.min(root.items.length,Math.floor((y+Style.space(21))/Style.space(44))))}
       onReleased:function(mouse){var target=root.dropIndex;var id=root.draggedRun;root.draggedRun="";root.dropIndex=-1;if(Math.abs(mapToGlobal(mouse.x,mouse.y).y-startY)<5)return;if(target<0||target===index||target===index+1)return;root.move(id,target<root.items.length?root.items[target].runId:null)}
       onCanceled:{root.draggedRun="";root.dropIndex=-1}
      }
     }
     WorkIndicator{visible:!!modelData.pending;active:visible && root.visible;implicitWidth:Style.space(12)}
     T3Icon{visible:modelData.attachments.length>0;name:"paperclip";tint:root.dim;implicitWidth:Style.space(13)}
     Text{
      Layout.fillWidth:true;text:modelData.text || modelData.attachments.map(function(a){return a.name}).join(", ") || T3I18n.tr("Queued message · content unavailable");color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText
      Controls.ToolTip.visible:previewHover.hovered;Controls.ToolTip.text:modelData.text+"\n"+modelData.attachments.map(function(a){return a.name}).join(", ")
      HoverHandler{id:previewHover}
     }
     T3IconButton{objectName:"queueEdit_"+modelData.runId;name:"square-pen";tooltipText:T3I18n.tr("Edit in composer");enabled:!root.busy&&!modelData.pending&&modelData.available!==false&&!root.bridge.draft.queueEdit;onClicked:{root.bridge.beginQueueEdit(modelData);root.editSelected()}}
     T3IconButton{objectName:"queueSteer_"+modelData.runId;name:"corner-up-right";tooltipText:T3I18n.tr("Steer the active run");enabled:!root.busy&&!modelData.pending&&!!root.bridge.queue.canSteer;onClicked:root.bridge.queueAction("steer",{runId:modelData.runId,targetRunId:root.bridge.queue.activeRunId})}
     T3IconButton{objectName:"queueRemove_"+modelData.runId;name:"x";tooltipText:T3I18n.tr("Remove from queue");enabled:!root.busy&&!modelData.pending&&!(root.bridge.draft.queueEdit && root.bridge.draft.queueEdit.runId===modelData.runId);onClicked:root.bridge.queueAction("cancel",{runId:modelData.runId})}
    }
   }
  }
 }
}
