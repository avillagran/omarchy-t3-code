import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import "../js/MarkdownStyle.js" as MarkdownStyle

// Presentation only: parent supplies a thread-scoped native plan projection and
// handles every signal after checking current scope, permissions and draft state.
Item {
 id:root
 objectName:"t3PlanWorkflow"
 property var view:({loading:false,error:"",plans:[],ready:null})
 property bool compact:false
 signal retryRequested()
 signal cancelRequested()
 signal linkRequested(string link)
 implicitHeight:planColumn.implicitHeight
 property string planId:"" // set for a historical timeline card; empty selects ready plan
 property bool modeEnabled:false
 property string interactionMode:"default"
 property bool canOperate:false
 property bool busy:false
 property string draftText:""
 property bool hasAttachments:false
 signal modeRequested(string mode)
 signal followUpRequested(var plan,string draft,bool newThread)
 signal copyRequested(string markdown)
 readonly property var plan: planId ? (view.plans || []).find(function(p){return p.id===planId}) || null : view.ready || null
 readonly property bool actionable: !planId && !!view.ready && !!plan && plan.status==="active" && modeEnabled && canOperate && !busy && !hasAttachments && !view.loading && !view.error
 readonly property bool lengthy:!!plan && ((plan.markdown || "").length>900 || (plan.markdown || "").split("\n").length>20)
 property bool expanded:false
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 function requestImplementation(newThread){if(actionable)followUpRequested(plan,newThread?"":draftText,newThread)}
 function requestFeedback(){if(actionable && draftText.trim())followUpRequested(plan,draftText,false)}
 onPlanIdChanged: expanded=false
 onPlanChanged: expanded=false
 ColumnLayout {
  id:planColumn;anchors.fill:parent;spacing:Style.space(7)
  RowLayout {
   visible:!root.compact
   Layout.fillWidth:true;spacing:Style.space(6)
   Text{text:T3I18n.tr("Plan");color:Color.accent;font.pixelSize:Style.font.caption;font.bold:true;textFormat:Text.PlainText}
   Text{Layout.fillWidth:true;text:root.plan?root.plan.title:T3I18n.tr("Proposed plan");color:root.fg;font.pixelSize:Style.font.body;font.bold:true;textFormat:Text.PlainText;elide:Text.ElideRight}
   T3PickerButton {primary:false;objectName:"t3PlanCopy";visible:!!root.plan;text:T3I18n.tr("Copy plan");onClicked:root.copyRequested(root.plan.markdown);palette.button:Color.popups.background;palette.buttonText:root.fg}
  }
  RowLayout {
   Layout.fillWidth:true;visible:!root.compact && root.modeEnabled && !root.planId;spacing:Style.space(5)
   T3PickerButton {primary:false;objectName:"t3PlanDefault";text:T3I18n.tr("Default");checkable:true;checked:root.interactionMode!=="plan";enabled:root.canOperate&&!root.busy;onClicked:root.modeRequested("default");palette.button:Color.popups.background;palette.buttonText:root.fg}
   T3PickerButton {primary:false;objectName:"t3PlanMode";text:T3I18n.tr("Plan");checkable:true;checked:root.interactionMode==="plan";enabled:root.canOperate&&!root.busy;onClicked:root.modeRequested("plan");palette.button:Color.popups.background;palette.buttonText:root.fg}
  }
  Text{Layout.fillWidth:true;visible:!!root.view.error;text:T3I18n.error(root.view.error || "");color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  RowLayout{visible:!!root.view.error
   T3PickerButton{primary:false;enabled:!root.busy;text:T3I18n.tr("Retry");onClicked:root.retryRequested()}
   T3PickerButton{primary:false;visible:root.view.canCancel===true;enabled:!root.busy;text:T3I18n.tr("Cancel");onClicked:root.cancelRequested()}
  }
  WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:!!root.view.loading;active:visible}
  Text{Layout.fillWidth:true;visible:!root.plan && !root.view.loading && !root.view.error;text:T3I18n.tr("No proposed plan for this thread.");color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  Rectangle {
   Layout.fillWidth:true;Layout.fillHeight:false;Layout.preferredHeight:Math.min(Style.space(root.lengthy?(root.expanded?400:240):400),Math.max(Style.space(48),planText.contentHeight+Style.space(18)));visible:!!root.plan&&!root.compact
   radius:Style.space(8);color:Color.popups.background;border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.12)
   Controls.ScrollView{
    id:planScroll
    T3Wheel{flickable:planScroll.contentItem}
    anchors.fill:parent;anchors.margins:Style.space(9);clip:true
    Controls.TextArea{
     id:planText;objectName:"t3PlanMarkdown";readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap
     textFormat:root.plan && root.plan.rendered ? TextEdit.RichText : TextEdit.PlainText
     text:root.plan ? (root.plan.rendered ? MarkdownStyle.render(root.plan.rendered,Style.font.body,Style.spaceReal(1),String(root.fg),String(root.dim),String(Color.popups.background),String(root.dim)) : root.plan.displayMarkdown || "") : ""
     onLinkActivated:function(link){root.linkRequested(link)}
     color:root.fg;selectionColor:Color.accent;font.pixelSize:Style.font.body
     background:Rectangle{color:"transparent"}
    }
   }
  }
  T3PickerButton{primary:false;objectName:"t3PlanExpand";Layout.alignment:Qt.AlignHCenter;visible:root.lengthy&&!root.compact;text:T3I18n.tr(root.expanded?"Show less":"Show more");onClicked:root.expanded=!root.expanded;palette.button:Color.popups.background;palette.buttonText:root.fg}
  Text{Layout.fillWidth:true;visible:root.actionable;text:T3I18n.tr("Plan ready")+(root.plan?" · "+root.plan.title:"");color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  Flow{
   Layout.fillWidth:true;visible:root.actionable;spacing:Style.space(5)
   T3PickerButton{primary:false;objectName:"t3PlanFeedback";visible:false;text:T3I18n.tr("Send plan feedback");enabled:root.actionable && !!root.draftText.trim();onClicked:root.requestFeedback();palette.button:Color.popups.background;palette.buttonText:root.fg}
   T3PickerButton{primary:true;objectName:"t3PlanImplement";text:T3I18n.tr(root.draftText.trim()?"Send plan feedback":"Implement plan");enabled:root.actionable;onClicked:root.requestImplementation(false);palette.button:Color.popups.background;palette.buttonText:root.fg}
   T3PickerButton{primary:false;objectName:"t3PlanNewThread";text:T3I18n.tr("Implement in new thread");enabled:root.actionable;onClicked:root.requestImplementation(true);palette.button:Color.popups.background;palette.buttonText:root.fg}
  }
 }
}
