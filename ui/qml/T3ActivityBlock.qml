import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
Column {
 id:root
 objectName:"t3ActivityBlock"
 required property var bridge
 required property var activity
 property bool active:false
 property bool expanded:false
 signal inspectRequested(var reference)
 // A failed tool says why, as in T3: its input and what it reported, read on demand.
 readonly property bool single:(activity.entries || []).length<2
 readonly property bool failed:single&&!!activity.failed&&!activity.active
 readonly property string failureKey:failed&&activity.inspector?bridge.toolFailureKey(activity.inspector):""
 readonly property var failure:failureKey?(bridge.toolFailures[failureKey] || null):null
 function requestFailure(){if(failureKey)bridge.requestToolFailure(activity.inspector)}
 onFailureKeyChanged:Qt.callLater(requestFailure)
 Component.onCompleted:Qt.callLater(requestFailure)
 width:parent.width
 spacing:Style.space(6)
 RowLayout {
  width:parent.width
  height:Math.max(Style.space(24),implicitHeight)
  T3IconButton{objectName:"t3ActivityToggle";visible:(root.activity.entries || []).length>1;name:root.expanded?"chevron-down":"chevron-right";implicitWidth:Style.space(20);implicitHeight:implicitWidth;tooltipText:root.bridge.activityText(root.activity);onClicked:root.expanded=!root.expanded}
  T3IconButton{objectName:"t3ActivityInspect";visible:!!root.activity.inspector&&(root.activity.entries || []).length<2;name:"terminal";tooltipText:T3I18n.tr("Activity details");onClicked:root.inspectRequested(root.activity.inspector)}
  WorkIndicator{visible:!!root.activity.active;active:visible&&root.active}
  T3Icon{objectName:"t3ActivityFailed";visible:root.failed;name:"circle-alert";implicitWidth:Style.space(13);tint:Color.urgent}
  T3ActivityText{Layout.fillWidth:true;active:root.active&&!!root.activity.active;tint:root.failed?Color.urgent:Color.popups.text;text:root.bridge.activityText(root.activity)}
 }
 Column {
  objectName:"t3ActivityFailure";visible:root.failed&&!!root.failure&&!root.failure.busy&&(!!root.failure.input||!!root.failure.detail)
  width:root.width-Style.space(26);x:Style.space(26);spacing:Style.space(4)
  TextEdit{visible:!!(root.failure&&root.failure.input);width:parent.width;text:root.failure?String(root.failure.input || ""):"";readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText;color:Color.muted;selectionColor:Color.accent;font.family:"monospace";font.pixelSize:Style.font.caption}
  TextEdit{objectName:"t3ActivityFailureDetail";visible:!!(root.failure&&root.failure.detail);width:parent.width;text:root.failure?String(root.failure.detail || ""):"";readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText;color:Color.popups.text;selectionColor:Color.accent;font.family:"monospace";font.pixelSize:Style.font.caption}
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
   T3IconButton{objectName:"t3ActivityEntryInspect";visible:!!modelData.inspector;name:"terminal";tooltipText:T3I18n.tr("Activity details");onClicked:root.inspectRequested(modelData.inspector)}
   T3Icon{name:modelData.active?"terminal":modelData.failed?"circle-alert":"check";implicitWidth:Style.space(12);tint:modelData.failed||modelData.status==="failed"?Color.urgent:Color.muted}
   Text{id:entryText;Layout.fillWidth:true;text:root.bridge.activityText(modelData);color:modelData.failed&&!modelData.active?Color.urgent:Color.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  }
 }
}
