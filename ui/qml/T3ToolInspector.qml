import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// A panel body: parent owns the popup/inline placement and native request bridge.
Item {
 id:root
 objectName:"t3ToolInspector"
 required property var bridge
 property string environment:""
 property string threadId:""
 property bool opened:false
 property bool busy:false
 property string error:""
 property var detail:({})
 property var reference:({})
 property var pendingScope:({})
 property string requestId:""
 readonly property string instanceId:String(Date.now())+"-"+Math.random().toString(36).slice(2)
 property int generation:0
 property string mode:"details"
 readonly property color fg:Color.popups.text
 readonly property color muted:Color.muted
 readonly property var scopeKeys:["environment","threadId","sourceThreadId","sourceItemId","type","runId","providerTurnId","revision"]
 readonly property var displayedSections:mode==="raw"?[{key:"raw",label:"Raw item",text:detail.raw || "",truncated:!!detail.rawTruncated}]:mode==="metadata"?[{key:"metadata",label:"Metadata",text:detail.metadata || "",truncated:!!detail.metadataTruncated}]:(detail.sections || [])
 signal requested(string requestId,var scope)
 signal closed()
 implicitWidth:Style.space(640)
 implicitHeight:Style.space(440)
 visible:opened

 function sameScope(a,b){
  if(!a || !b)return false
  for(var i=0;i<scopeKeys.length;i++){var k=scopeKeys[i];if(a[k]!==b[k])return false}
  return true
 }
 function validReference(r){
  return !!r&&typeof r.sourceThreadId==="string"&&r.sourceThreadId.length>0&&typeof r.sourceItemId==="string"&&r.sourceItemId.length>0&&typeof r.type==="string"&&r.type.length>0&&typeof r.revision==="string"&&r.revision.length>0&&(r.runId===null||typeof r.runId==="string")&&(r.providerTurnId===null||typeof r.providerTurnId==="string")
 }
 function openItem(r){
  dismiss(false)
  opened=true
  if(!environment||!threadId||!validReference(r)){error="invalid_tool_reference";return false}
  reference=Object.assign({},r)
  mode="details"
  return reload()
 }
 function reload(){
  if(!opened||busy||!validReference(reference))return false
  generation++
  requestId="tool-inspector/"+instanceId+"/"+generation
  pendingScope={environment:environment,threadId:threadId,sourceThreadId:reference.sourceThreadId,sourceItemId:reference.sourceItemId,type:reference.type,runId:reference.runId,providerTurnId:reference.providerTurnId,revision:reference.revision}
  detail={};error="";busy=true
  requested(requestId,Object.assign({},pendingScope))
  return true
 }
 // Parent calls this with the correlation token captured before dispatch, on success OR failure.
 function accept(id,response,failure){
  if(!opened||!busy||id!==requestId||pendingScope.environment!==environment||pendingScope.threadId!==threadId)return false
  if(!failure&&(!response||!sameScope(response.scope,pendingScope)))return false
  busy=false;requestId=""
  if(failure){error=String(failure);detail={};return true}
  detail=response;error=""
  return true
 }
 function dismiss(notify){
  generation++;requestId="";pendingScope={};reference={};busy=false;detail={};error="";opened=false;mode="details"
  if(notify)closed()
 }
 function errorText(code){
  switch(code){
  case "invalid_tool_reference":return T3I18n.tr("This activity has no inspectable item reference.")
  case "tool_scope_mismatch":return T3I18n.tr("The item no longer matches this activity.")
  case "malformed_tool_item":return T3I18n.tr("The server returned an invalid item.")
  default:return T3I18n.tr("Couldn't load output")+": "+T3I18n.error(code)
  }
 }
 onEnvironmentChanged:if(opened)dismiss(true)
 onThreadIdChanged:if(opened)dismiss(true)
 Keys.onEscapePressed:function(event){dismiss(true);event.accepted=true}

 ColumnLayout {
  anchors.fill:parent
  spacing:Style.space(8)
  RowLayout {
   Layout.fillWidth:true
   T3Icon{name:root.reference.type==="reasoning"?"sparkles":"terminal";tint:root.detail.failed?Color.urgent:root.muted}
   Text{objectName:"t3ToolInspectorTitle";Layout.fillWidth:true;text:root.detail.title || T3I18n.tr("Activity details");color:root.fg;font.pixelSize:Style.font.body;textFormat:Text.PlainText;elide:Text.ElideRight}
   T3IconButton{objectName:"t3ToolInspectorReload";name:"refresh-cw";tooltipText:T3I18n.tr("Retry");enabled:!root.busy&&root.validReference(root.reference);onClicked:root.reload()}
   T3IconButton{objectName:"t3ToolInspectorClose";name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.dismiss(true)}
  }
  RowLayout {
   Layout.fillWidth:true
   visible:!root.busy&&root.detail.available===true
   Repeater {
    model:[{key:"details",label:"Details"},{key:"metadata",label:"Metadata"},{key:"raw",label:"Raw item"}]
    delegate:T3PickerButton{required property var modelData;objectName:"t3ToolInspectorTab_"+modelData.key;text:T3I18n.tr(modelData.label);primary:root.mode===modelData.key;onClicked:root.mode=modelData.key}
   }
   Item{Layout.fillWidth:true}
  }
  Text{objectName:"t3ToolInspectorStatus";Layout.fillWidth:true;visible:!!root.detail.status;text:T3I18n.tr(root.detail.status || "")+(root.detail.exitCode!==null&&root.detail.exitCode!==undefined?" · "+T3I18n.tr("Exit code")+": "+root.detail.exitCode:"");color:root.detail.failed?Color.urgent:root.muted;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  RowLayout{visible:root.busy;Layout.fillWidth:true;WorkIndicator{active:root.busy}Text{text:T3I18n.tr("Loading output…");color:root.muted;font.pixelSize:Style.font.caption}}
  Text{objectName:"t3ToolInspectorError";Layout.fillWidth:true;visible:!!root.error;text:root.errorText(root.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  Text{Layout.fillWidth:true;visible:!root.busy&&!root.error&&root.detail.available===false;text:T3I18n.tr("Output is no longer available.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  Text{Layout.fillWidth:true;visible:root.mode==="details"&&root.detail.emptyOutput===true;text:T3I18n.tr("No output.");color:root.muted;font.pixelSize:Style.font.caption}
  Text{Layout.fillWidth:true;visible:root.detail.outputOmitted===true;text:T3I18n.tr("Output was withheld by the server. Retry to refresh.");color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
  Controls.ScrollView {
   id:bodyScroll
   objectName:"t3ToolInspectorScroll"
   Layout.fillWidth:true;Layout.fillHeight:true
   clip:true
   visible:!root.busy&&!root.error&&root.detail.available===true
   contentWidth:availableWidth
   T3Wheel{flickable:bodyScroll.contentItem;policy:root.bridge.scrollPolicy || ({mouseNative:false,touchpadNative:false})}
   Column {
    width:bodyScroll.availableWidth
    spacing:Style.space(8)
    Repeater {
     model:root.displayedSections
     delegate:Column {
      id:section
      required property var modelData
      width:parent.width
      spacing:Style.space(6)
      RowLayout {
       width:parent.width
       Text{Layout.fillWidth:true;text:T3I18n.tr(section.modelData.label);color:root.muted;font.pixelSize:Style.font.caption}
       T3CopyButton{objectName:"t3ToolInspectorCopy_"+section.modelData.key;bridge:root.bridge;copyText:String(section.modelData.text || "");tooltipText:T3I18n.tr(copyState==="copied"?"Copied!":copyState==="error"?"Copy failed":"Copy")}
      }
      Controls.ScrollView {
       id:sectionScroll
       width:parent.width
       height:Math.min(Style.space(320),Math.max(Style.space(36),body.implicitHeight))
       contentWidth:availableWidth
       clip:true
       T3Wheel{flickable:sectionScroll.contentItem;policy:root.bridge.scrollPolicy || ({mouseNative:false,touchpadNative:false})}
       Controls.TextArea {
        id:body
        objectName:"t3ToolInspectorText_"+section.modelData.key
        width:sectionScroll.availableWidth
        text:String(section.modelData.text || "")
        textFormat:TextEdit.PlainText
        readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap
        color:section.modelData.key==="input"?root.fg:root.muted
        selectionColor:Color.accent
        font.family:"monospace";font.pixelSize:Style.font.caption
        padding:Style.space(8)
        background:Rectangle{color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.025);radius:Style.space(6);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.08)}
       }
      }
      Text{width:parent.width;visible:!!section.modelData.truncated;text:T3I18n.tr("Display limit reached. Copy includes only the displayed text.");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
     }
    }
   }
  }
 }
}
