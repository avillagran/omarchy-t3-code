import QtQuick 6.11
import qs.Commons
Item {
 id:root;objectName:"t3InlineHtml"
 required property var bridge
 required property var attachment
 property bool active:false
 property string sessionId:""
 property real pageHeight:620
 // Generated previews (QML) can be dismissed from the conversation.
 property bool closable:false
 // Embedded previews (QML blocks) draw their own bar.
 property bool chromeVisible:true
 signal dismissed()
 readonly property var docState:bridge.inlineDocuments[sessionId] || ({})
 height:pageHeight
 // A preview refused before it rendered is just its error line. Set, not
 // bound: `active` follows the height, so a binding would loop.
 function fit(){if(docState.error&&!docState.frameUrl)pageHeight=view.chromeHeight+Style.space(8)}
 function sync(){
  if(active&&!sessionId)sessionId=bridge.openInlineDocument(attachment)
  else if(!active&&sessionId){bridge.closeInlineDocument(sessionId);sessionId=""}
 }
 onActiveChanged:sync()
 Component.onCompleted:sync()
 Component.onDestruction:if(sessionId)bridge.closeInlineDocument(sessionId)
 Connections{target:root.bridge;function onInlineDocumentsChanged(){var d=root.bridge.inlineDocuments[root.sessionId];if(d&&d.error&&!d.frameUrl){Qt.callLater(root.fit);return}if(d&&d.contentHeight)root.pageHeight=Math.max(240,Math.min(1800,d.contentHeight+Math.round(view.height-view.surfaceHeight)))}}
 // All renderers busy (several previews on screen): try again while visible.
 Timer{interval:2000;running:root.active&&root.docState.error==="viewer_unavailable";onTriggered:{root.bridge.closeInlineDocument(root.sessionId);root.sessionId="";root.sync()}}
 T3DocumentView{
  id:view;anchors.fill:parent;bridge:root.bridge;sessionId:root.sessionId;inlineMode:true;fallbackTitle:root.attachment.name || "HTML"
  visible:!!root.sessionId;dismissible:root.closable;chromeVisible:root.chromeVisible
  onDismissed:root.dismissed()
  onChromeHeightChanged:Qt.callLater(root.fit)
  onClosed:root.bridge.openReference("",root.attachment)
 }
}
