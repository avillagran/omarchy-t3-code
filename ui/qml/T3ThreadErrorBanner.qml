import QtQuick 6.11
import QtQuick.Layouts 6.11
import qs.Commons
// The open thread's last error (T3 ThreadErrorBanner / UsageLimitRecoveryBanner):
// what stopped it, dismissible per thread and error text for this session.
Rectangle {
 id:root
 objectName:"t3ThreadErrorBanner"
 required property var bridge
 property var dismissed:({})
 readonly property var info:bridge.selectedInfo || ({})
 readonly property string message:{
  // Providers often nest the reason as JSON: {"error":{"message":"…"}}.
  var raw=String(info.runtimeError || "");try{var v=JSON.parse(raw);var m=v&&v.error&&v.error.message||v&&v.message;if(m)return String(m)}catch(e){}
  return raw
 }
 readonly property bool limit:info.runtimeErrorClass==="usage_limit"
 readonly property string dismissKey:(info.key || "")+"\n"+message
 readonly property bool shown:!!message&&!dismissed[dismissKey]&&!info.working
 property bool expanded:false
 readonly property color tone:limit?T3Palette.warning:Color.urgent
 readonly property string resetText:info.usageLimitResetAt>0?new Date(info.usageLimitResetAt).toLocaleString(Qt.locale(),Locale.ShortFormat):""
 visible:shown
 implicitHeight:content.implicitHeight+Style.space(14)
 radius:Style.space(8);color:Qt.rgba(tone.r,tone.g,tone.b,0.1);border.width:1;border.color:Qt.rgba(tone.r,tone.g,tone.b,0.35)
 onDismissKeyChanged:expanded=false
 RowLayout {
  id:content;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(7);leftMargin:Style.space(10)}spacing:Style.space(8)
  T3Icon{Layout.alignment:Qt.AlignTop;Layout.topMargin:Style.space(2);name:"circle-alert";tint:root.tone;implicitWidth:Style.space(14)}
  ColumnLayout {
   Layout.fillWidth:true;spacing:Style.space(2)
   Text{Layout.fillWidth:true;text:T3I18n.tr(root.limit?"Usage limit reached":"The thread stopped with an error");color:root.tone;font.pixelSize:Style.font.caption;font.weight:Font.Medium;textFormat:Text.PlainText}
   Text{
    objectName:"t3ThreadErrorText";Layout.fillWidth:true;text:root.limit?(root.resetText?T3I18n.tr("Resets {time}",{time:root.resetText}):T3I18n.tr("Reset time unavailable; retry manually")):root.message
    color:Color.popups.text;font.pixelSize:Style.font.body;wrapMode:Text.Wrap;maximumLineCount:root.expanded?100:2;elide:Text.ElideRight;textFormat:Text.PlainText
    MouseArea{anchors.fill:parent;enabled:parent.truncated||root.expanded;cursorShape:enabled?Qt.PointingHandCursor:Qt.ArrowCursor;onClicked:root.expanded=!root.expanded}
   }
  }
  T3CopyButton{Layout.alignment:Qt.AlignTop;visible:!root.limit;bridge:root.bridge;copyText:root.message;implicitWidth:Style.space(22);implicitHeight:implicitWidth}
  T3IconButton{objectName:"t3ThreadErrorDismiss";Layout.alignment:Qt.AlignTop;name:"x";implicitWidth:Style.space(22);implicitHeight:implicitWidth;tooltipText:T3I18n.tr("Dismiss");onClicked:{var all=Object.assign({},root.dismissed);all[root.dismissKey]=true;root.dismissed=all}}
 }
}
