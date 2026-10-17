import QtQuick
import QtQuick.Window
import QtWebEngine
import "../js/DocumentTheme.js" as Theme
Window {
 id:root;visible:true;width:documentData.width || 900;height:documentData.height || 620
 color:documentData.theme.variables["--background"]
 WebEngineView{
  id:web;anchors.fill:parent;focus:true;backgroundColor:root.color
  profile:WebEngineProfile{offTheRecord:true;httpCacheType:WebEngineProfile.MemoryHttpCache;httpCacheMaximumSize:8*1024*1024;persistentCookiesPolicy:WebEngineProfile.NoPersistentCookies;onDownloadRequested:function(d){d.cancel()}}
  settings.localContentCanAccessFileUrls:false;settings.localContentCanAccessRemoteUrls:false
  settings.javascriptCanAccessClipboard:false;settings.javascriptCanOpenWindows:false;settings.fullScreenSupportEnabled:false
  onPermissionRequested:function(p){p.deny()}
  onContextMenuRequested:function(r){r.accepted=true}
  onNewWindowRequested:function(r){}
  onNavigationRequested:function(r){if(r.isMainFrame&&r.url.toString()!=="about:blank"&&!r.url.toString().startsWith("data:"))r.action=WebEngineNavigationRequest.IgnoreRequest}
  onLoadingChanged:documentHost.requestFrame()
  onScrollPositionChanged:documentHost.requestFrame()
  Component.onCompleted:{forceActiveFocus();loadHtml(Theme.page(documentData))}
 }
 property int measuredHeight:0
 Timer{interval:200;running:true;repeat:true;onTriggered:web.runJavaScript("window.__t3ContentHeight || 0",function(h){if(h>0&&h!==root.measuredHeight){root.measuredHeight=h;documentHost.contentHeight(h);documentHost.requestFrame()}})}
 function verifyDocument(){web.runJavaScript("JSON.stringify({status:window.__t3DocumentStatus,sandbox:document.querySelector('iframe').getAttribute('sandbox')})",function(value){console.log("DOCUMENT_VERIFY "+value)})}
 function copySelection(){web.triggerWebAction(WebEngineView.Copy)}
 function setZoom(value){web.zoomFactor=value;documentHost.requestFrame()}
}
