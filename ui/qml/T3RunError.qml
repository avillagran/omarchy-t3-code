import QtQuick 6.11
import QtQuick.Layouts 6.11
import qs.Commons
// A provider failure in the conversation (T3 providerErrorPresentation and
// TimelineWorkEntryRow): its title, retry progress and the provider's words.
Rectangle {
 id:root
 required property var bridge
 property var failure:({})
 readonly property bool limit:failure.class==="usage_limit"
 readonly property var retry:failure.retry || null
 readonly property int attempts:retry&&retry.attempt?retry.attempt:0
 readonly property string heading:{
  if(!retry)return limit?T3I18n.tr("Usage limit reached"):String(failure.title || "")||T3I18n.tr("Provider error")
  if(failure.status==="running")return T3I18n.tr("Retrying provider ({n}/{max})",{n:attempts,max:retry.maxAttempts || "?"})
  if(failure.status==="completed")return T3I18n.tr("Provider recovered ({n} retries)",{n:attempts})
  if(failure.status==="failed")return T3I18n.tr(limit?"Usage limit reached after {n} retries":"Provider error after {n} retries",{n:attempts})
  return T3I18n.tr("Provider retry stopped")
 }
 // Recovered retries read as success, running ones and usage limits as a warning.
 readonly property color tone:retry&&failure.status==="completed"?T3Palette.done:(retry&&failure.status==="running")||limit?T3Palette.warning:Color.urgent
 readonly property var provider:(bridge.catalog.providers || []).find(function(p){return p.instanceId===root.failure.providerInstanceId}) || null
 readonly property string resetText:failure.resetAt?new Date(failure.resetAt).toLocaleString(Qt.locale(),Locale.ShortFormat):""
 readonly property string detail:limit?(resetText?T3I18n.tr("Retry after {time}.",{time:resetText}):""):String(failure.message || "")
 objectName:"t3RunError"
 implicitHeight:content.implicitHeight+Style.space(16)+(causesBlock.visible?causesBlock.implicitHeight+Style.space(8):0)
 radius:Style.space(8);color:Qt.rgba(tone.r,tone.g,tone.b,0.08)
 border.width:1;border.color:Qt.rgba(tone.r,tone.g,tone.b,0.35)
 ColumnLayout {
  id:content;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(8);leftMargin:Style.space(10)}spacing:Style.space(4)
  RowLayout {
   Layout.fillWidth:true;spacing:Style.space(6)
   T3Icon{name:root.retry&&root.failure.status==="completed"?"circle-check":"circle-alert";tint:root.tone;implicitWidth:Style.space(13)}
   Text{objectName:"t3RunErrorTitle";Layout.fillWidth:true;text:root.heading;color:root.tone;font.pixelSize:Style.font.caption;font.weight:Font.Medium;elide:Text.ElideRight;textFormat:Text.PlainText}
   // Which provider failed: the message alone is often generic ("Provider turn failed.").
   Text{visible:!!root.provider;text:root.provider?String(root.provider.name || root.provider.instanceId):"";color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.55);font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   Text{visible:!!root.failure.code;text:String(root.failure.code || "");color:Qt.rgba(root.tone.r,root.tone.g,root.tone.b,0.7);font.family:"monospace";font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   T3CopyButton{objectName:"t3CopyRunError";visible:!!root.failure.message;bridge:root.bridge;copyText:String(root.failure.message || "");implicitWidth:Style.space(20);implicitHeight:implicitWidth}
  }
  TextEdit {
   objectName:"t3RunErrorMessage";visible:!!root.detail;Layout.fillWidth:true;text:root.detail;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText
   color:Color.popups.text;selectionColor:Color.accent;selectedTextColor:Color.popups.background;font.pixelSize:Style.font.body
  }
  Text{visible:!!root.retry&&root.failure.status==="running"&&root.retry.retryDelayMs>0;Layout.fillWidth:true;text:T3I18n.tr("Retrying in {s}s.",{s:Math.ceil((root.retry?root.retry.retryDelayMs:0)/1000)});color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.6);font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
 }
 // What failed before the provider gave up, in the tools' own words (T3 shows
 // these as "Failed to …" rows with their input and output).
 readonly property string contextKey:failure.runId&&!retry?bridge.failureContextKey(failure.runId):""
 readonly property var context:contextKey?(bridge.failureContexts[contextKey] || null):null
 function requestContext(){if(contextKey)bridge.requestFailureContext(failure.runId)}
 onContextKeyChanged:Qt.callLater(requestContext)
 Component.onCompleted:Qt.callLater(requestContext)
 readonly property var causes:context&&context.tools?context.tools:[]
 ColumnLayout {
  id:causesBlock;objectName:"t3RunErrorCauses";visible:root.causes.length>0
  anchors{left:parent.left;right:parent.right;top:content.bottom;leftMargin:Style.space(10);rightMargin:Style.space(8);topMargin:Style.space(4)}spacing:Style.space(6)
  Text{Layout.fillWidth:true;text:T3I18n.tr(root.causes.some(function(c){return c.failed})?"What failed":"Interrupted");color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.55);font.pixelSize:Style.font.caption;font.weight:Font.Medium;textFormat:Text.PlainText}
  Repeater {
   model:root.causes
   ColumnLayout {
    required property var modelData
    Layout.fillWidth:true;spacing:Style.space(2)
    RowLayout {
     Layout.fillWidth:true;spacing:Style.space(6)
     T3Icon{name:"circle-alert";implicitWidth:Style.space(12);tint:modelData.failed?Color.urgent:T3Palette.warning}
     Text{Layout.fillWidth:true;text:modelData.label;color:Color.popups.text;font.pixelSize:Style.font.caption;font.weight:Font.Medium;elide:Text.ElideMiddle;textFormat:Text.PlainText}
     T3CopyButton{visible:!!modelData.detail;bridge:root.bridge;copyText:String(modelData.detail || "");implicitWidth:Style.space(18);implicitHeight:implicitWidth}
    }
    TextEdit{visible:!!modelData.input;Layout.fillWidth:true;Layout.leftMargin:Style.space(18);text:String(modelData.input || "");readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText;color:Color.muted;selectionColor:Color.accent;font.family:"monospace";font.pixelSize:Style.font.caption}
    TextEdit{objectName:"t3RunErrorCauseDetail";visible:!!modelData.detail;Layout.fillWidth:true;Layout.leftMargin:Style.space(18);text:String(modelData.detail || "");readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText;color:Color.popups.text;selectionColor:Color.accent;font.family:"monospace";font.pixelSize:Style.font.caption}
   }
  }
 }
}
