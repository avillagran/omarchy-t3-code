import QtQuick 6.11
import qs.Commons
Item {
 id:root;objectName:"t3BrowserView"
 required property var bridge
 property string mountId:""
 property bool completed:false
 readonly property bool ownsSurface:!!mountId&&bridge.browserMountId===mountId
 function sync(){if(!completed)return;if(visible&&!mountId)mountId=bridge.mountBrowserPage(width,height);else if(!visible&&mountId){bridge.unmountBrowserPage(mountId);mountId=""}}
 Component.onCompleted:{completed=true;sync()}
 Component.onDestruction:if(mountId)bridge.unmountBrowserPage(mountId)
 onVisibleChanged:sync()
 T3BrowserPage {
  id:page;anchors.fill:parent;visible:root.ownsSurface
  page:root.ownsSurface?root.bridge.browserSurface:({})
  interactive:root.ownsSurface&&root.bridge.browserSurface.interactive
  onInput:function(id,event){root.bridge.browserPageInput(id,event,root.mountId)}
  onSurfaceResized:function(width,height){if(root.ownsSurface)root.bridge.resizeBrowserPage(width,height,root.mountId)}
 }
 WorkIndicator{anchors.centerIn:parent;visible:root.ownsSurface&&root.bridge.browserSurface.busy;active:visible}
 Text{anchors.centerIn:parent;width:Math.max(0,parent.width-32);visible:root.ownsSurface&&!!root.bridge.browserSurface.error;text:T3I18n.error(root.bridge.browserSurface.error);textFormat:Text.PlainText;horizontalAlignment:Text.AlignHCenter;wrapMode:Text.Wrap;color:Color.urgent;font.pixelSize:Style.font.body}
}
