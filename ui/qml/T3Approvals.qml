import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Rectangle {
 id:root
 required property var bridge
 property alias optionsMenu:more
 readonly property var approval:bridge.activeApproval || null
 readonly property var state:bridge.approvalState || ({})
 readonly property var operation:approval?((bridge.approvalOperations || {})[bridge.selectedKey+"/"+approval.requestId] || {error:state.responseError || "",answered:state.responseStatus==="accepted"}):({})
 readonly property bool canRespond:!!approval && approval.responseCapability==="live" && !!bridge.catalog.canWrite && !bridge.draft.loading && !bridge.draft.loadError && !operation.busy && !operation.answered
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 readonly property string label:T3I18n.tr(approval?({command:"Command approval","file-read":"File read approval","file-change":"File change approval",permission:"App permission approval","mcp-elicitation":"App access approval"}[approval.requestKind] || "Approval required"):"Approval required")
 visible:!!approval
 implicitHeight:body.implicitHeight+Style.space(20)
 color:Qt.rgba(fg.r,fg.g,fg.b,0.035);radius:Style.space(10);border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.12)
 onApprovalChanged:more.close()
 ColumnLayout {
  id:body;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(10)} spacing:Style.space(8)
  RowLayout {
   Layout.fillWidth:true
   T3Icon{name:"circle-alert";tint:Color.accent}
   Text{text:root.label;color:Color.accent;font.pixelSize:Style.font.caption;font.bold:true}
   Text{Layout.fillWidth:true;text:root.approval?root.approval.appName || "":"";color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
   Text{visible:(root.bridge.approvals || []).length>1;text:"1/"+(root.bridge.approvals || []).length;color:root.dim;font.pixelSize:Style.font.caption}
  }
  Controls.ScrollView {
   id:detail;Layout.fillWidth:true;Layout.preferredHeight:Math.min(detailText.implicitHeight,Style.space(100));contentWidth:availableWidth;clip:true
   T3Wheel{flickable:detail.contentItem}
   TextEdit {
    id:detailText;objectName:"approvalDetail";width:detail.availableWidth;readOnly:true;selectByMouse:true
    text:!root.approval?"":root.approval.responseCapability!=="live"?T3I18n.tr("Provider process is gone — interrupt or restart the run to respond."):root.approval.detail || root.label
    color:root.fg;font.pixelSize:Style.font.body;font.family:root.approval&&root.approval.requestKind!=="mcp-elicitation"?"monospace":""
    textFormat:TextEdit.PlainText;wrapMode:TextEdit.Wrap
   }
  }
  Flow {
   Layout.fillWidth:true;spacing:Style.space(6)
   Repeater {
    model:root.state.attempt?[]:((root.approval || {}).options || []).filter(function(o){return o.decision==="accept"||o.decision==="decline"})
    delegate:Controls.Button {
     required property var modelData
     objectName:"approval-"+modelData.decision
     text:(modelData.warning?"⚠ ":"")+T3I18n.tr(modelData.label);enabled:root.canRespond
     onClicked:root.bridge.respondApproval(modelData.decision)
     contentItem:Text{text:parent.text;color:root.fg;font.pixelSize:Style.font.body;horizontalAlignment:Text.AlignHCenter;verticalAlignment:Text.AlignVCenter}
     background:Rectangle{radius:Style.space(6);color:parent.down?Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.16):Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.06);border.width:1;border.color:parent.modelData.decision==="accept"?Color.accent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.15)}
     Controls.ToolTip.visible:hovered&&!!modelData.warning
     Controls.ToolTip.text:modelData.warning || ""
    }
   }
   T3IconButton {
    objectName:"approvalMore";name:"sliders-horizontal";tooltipText:T3I18n.tr("More approval options")
    visible:!root.state.attempt&&((root.approval || {}).options || []).some(function(o){return o.decision!=="accept"&&o.decision!=="decline"})
    enabled:root.canRespond;onClicked:more.open()
    Controls.Menu {
     id:more;y:-height
     Repeater {
      model:((root.approval || {}).options || []).filter(function(o){return o.decision!=="accept"&&o.decision!=="decline"})
      delegate:Controls.MenuItem{required property var modelData;objectName:"approval-"+modelData.decision;text:(modelData.warning?"⚠ ":"")+T3I18n.tr(modelData.label);enabled:root.canRespond&&!root.state.attempt;onTriggered:root.bridge.respondApproval(modelData.decision);Controls.ToolTip.visible:hovered&&!!modelData.warning;Controls.ToolTip.text:modelData.warning || ""}
     }
    }
   }
   Controls.Button{objectName:"approvalRetry";visible:!!root.state.attempt&&!root.operation.answered;text:T3I18n.tr("Retry same action");enabled:root.canRespond;onClicked:root.bridge.respondApproval(root.state.attempt.decision)}
  }
  Text {
   Layout.fillWidth:true;visible:!!text
   text:root.operation.error?T3I18n.error(root.operation.error):root.operation.busy?T3I18n.tr("Sending…"):root.operation.answered?T3I18n.tr("Answered"):""
   color:root.operation.error?Color.urgent:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap
  }
 }
}
