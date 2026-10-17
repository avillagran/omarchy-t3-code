import QtQuick
import qs.Commons
Item {
 id:root;objectName:"t3InlineHtml"
 required property var bridge
 required property var attachment
 property bool active:false
 property string sessionId:""
 property real pageHeight:620
 height:pageHeight
 function sync(){
  if(active&&!sessionId)sessionId=bridge.openInlineDocument(attachment)
  else if(!active&&sessionId){bridge.closeInlineDocument(sessionId);sessionId=""}
 }
 onActiveChanged:sync()
 Component.onCompleted:sync()
 Component.onDestruction:if(sessionId)bridge.closeInlineDocument(sessionId)
 Connections{target:root.bridge;function onInlineDocumentsChanged(){var d=root.bridge.inlineDocuments[root.sessionId];if(d&&d.contentHeight)root.pageHeight=Math.max(240,Math.min(1800,d.contentHeight+Math.round(view.height-view.surfaceHeight)))}}
 T3DocumentView{
  id:view;anchors.fill:parent;bridge:root.bridge;sessionId:root.sessionId;inlineMode:true;fallbackTitle:root.attachment.name || "HTML"
  visible:!!root.sessionId
  onClosed:root.bridge.openReference("",root.attachment)
 }
}
