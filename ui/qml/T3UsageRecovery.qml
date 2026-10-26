import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
ColumnLayout {
 id:root
 required property var bridge
 readonly property var lastError:(bridge.transcript||[]).filter(function(row){return row.role==="error"}).slice(-1)[0]||null
 readonly property var failure:lastError?lastError.error||null:null
 readonly property string resetAt:failure&&failure.resetAt||""
 readonly property var recovery:(bridge.selectedInfo||{}).limitRecovery||({})
 readonly property bool matching:!!failure&&recovery.runId===failure.runId&&recovery.resetAt===resetAt
 readonly property bool scheduled:matching&&recovery.autoResume===true
 property double now:Date.now()
 readonly property bool passed:!resetAt||Date.parse(resetAt)<=now
 readonly property bool snoozed:matching&&recovery.snooze===true&&!passed&&Date.parse((bridge.selectedInfo||{}).snoozedUntil)===Date.parse(resetAt)
 readonly property var operation:bridge.nightlyState?bridge.nightlyState("limit-recovery",bridge.selectedEnvironment,bridge.selectedThread):({})
 visible:!!failure&&failure.class==="usage_limit"&&(bridge.selectedInfo||{}).status==="failed"&&(!(bridge.selectedInfo||{}).latestRunId||(bridge.selectedInfo||{}).latestRunId===failure.runId)
 spacing:Style.space(5)
 onVisibleChanged:now=Date.now()
 Timer{interval:1000;running:root.visible;repeat:true;onTriggered:root.now=Date.now()}
 Text{Layout.fillWidth:true;text:T3I18n.tr("Usage limit reached")+(root.resetAt?" · "+(root.bridge.formatTimestamp?root.bridge.formatTimestamp(root.resetAt):root.resetAt):"");color:T3Palette.warning;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
 Flow {
  Layout.fillWidth:true;spacing:Style.space(6)
  T3PickerButton{objectName:"usageResumeAtReset";primary:false;text:T3I18n.tr(root.scheduled?"Cancel auto-resume":"Resume at reset");enabled:!!root.resetAt&&!root.operation.busy&&(root.bridge.catalog||{}).canWrite!==false;onClicked:root.bridge.requestNightly("limit-recovery",{runId:root.failure.runId,resetAt:root.resetAt,autoResume:!root.scheduled})}
  T3PickerButton{objectName:"usageSnoozeUntilReset";primary:false;text:T3I18n.tr(root.snoozed?"Wake now":"Snooze until reset");enabled:!!root.resetAt&&!root.operation.busy&&!root.passed&&(root.bridge.catalog||{}).canWrite!==false;onClicked:root.bridge.requestNightly("limit-recovery",{runId:root.failure.runId,resetAt:root.resetAt,snooze:!root.snoozed})}
 }
 Text{Layout.fillWidth:true;visible:!!root.operation.error;text:root.operation.error||"";color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}
}
