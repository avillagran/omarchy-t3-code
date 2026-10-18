import QtQuick 6.11
import qs.Commons
T3IconButton {
 id:root
 required property var bridge
 property string copyText:""
 property var attachment:null
 property string requestId:""
 property string copyState:"idle"
 property string copyError:""
 readonly property bool imageCopy:attachment!==null
 name:copyState==="busy"?"loader-circle":copyState==="copied"?"check":copyState==="error"?"circle-alert":"copy"
 tooltipText:T3I18n.tr(copyState==="copied"?"Copied!":copyState==="error"?"Copy failed":imageCopy?"Copy image":"Copy message")+(copyError?"\n"+T3I18n.error(copyError):"")
 tint:copyState==="error"?Color.urgent:copyState==="copied"?Color.accent:Color.popups.text
 enabled:copyState!=="busy"&&(imageCopy || copyText.length>0)
 onClicked:{reset.stop();copyError="";copyState="busy";requestId=imageCopy?bridge.copyImage(attachment):bridge.copyText(copyText)}
 onAttachmentChanged:{requestId="";copyState="idle";copyError="";if(reset)reset.stop()}
 Timer{id:reset;interval:1800;onTriggered:{root.copyState="idle";root.copyError=""}}
 Connections{target:root.bridge;function onCopyFinished(id,error){if(id!==root.requestId)return;root.requestId="";root.copyError=error;root.copyState=error?"error":"copied";reset.restart()}}
}
