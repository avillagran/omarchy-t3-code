import QtQuick 6.11
import QtQuick.Window 6.11
import QtWebEngine
Window {
 id:root;visible:true;width:browserData.width || 900;height:browserData.height || 620;color:"#ffffff"
 // A cold import needs a live WebEngine adapter, without opening a user tab.
 Loader{active:browserData.initializeCookieStore===true;opacity:0;sourceComponent:Component{WebEngineView{profile:browserProfile;url:"data:text/html,%3Chtml%3E%3C/html%3E";width:1;height:1}}}
 property var tabs:({})
 property string selected:""
 property string surfaceId:""
 function tabKey(threadId,tabId){return JSON.stringify([threadId,tabId])}
 function emitState(tab){browserHost.event({type:"status",threadId:tab.threadId,tabId:tab.tabId,navStatus:tab.navigation,canGoBack:tab.web.canGoBack,canGoForward:tab.web.canGoForward,profileId:browserData.profileId,runtime:"desktop"})}
 function selectedTab(){return tabs[selected] || null}
 function command(c){
  var key=tabKey(c.threadId,c.tabId),tab=tabs[key]
  if(c.type==="deactivate"){selected="";surfaceId="";browserHost.requestFrame();return}
  if(c.type==="open"){
   if(!c.threadId||!c.tabId)return
   if(!tab){tab=tabComponent.createObject(root.contentItem,{threadId:c.threadId,tabId:c.tabId,key:key});tabs[key]=tab}
   selected=key;surfaceId=c.surfaceId || "";if(c.url)tab.navigate(c.url);tab.web.forceActiveFocus();return
  }
  if(c.type==="activate"&&tab){selected=key;surfaceId=c.surfaceId || "";tab.web.forceActiveFocus();browserHost.requestFrame();return}
  if(c.type==="close-tab"&&tab){delete tabs[key];tab.destroy();if(selected===key)selected=Object.keys(tabs)[0] || "";browserHost.event({type:"closed",threadId:c.threadId,tabId:c.tabId});return}
  if(!tab)return
  if(c.type==="navigate")tab.navigate(c.url)
  else if(c.type==="refresh"){if(c.bypassCache)tab.web.reloadAndBypassCache();else tab.web.reload()}
  else if(c.type==="stop")tab.web.stop()
  else if(c.type==="back")tab.web.goBack()
  else if(c.type==="forward")tab.web.goForward()
  else if(c.type==="zoom"){var zoom=Number(c.value);if(zoom>=0.25&&zoom<=5)tab.web.zoomFactor=zoom}
  else if(c.type==="evaluate"&&c.requestId&&typeof c.script==="string"&&c.script.length<=32768){var id=c.requestId;tab.web.runJavaScript(c.script,function(value){browserHost.event({type:"evaluation",requestId:id,value:value===undefined?null:value})})}
  browserHost.requestFrame()
 }
 Component{id:tabComponent
  Item{id:tab
   required property string threadId
   required property string tabId
   required property string key
   property alias web:web
   property var navigation:({_tag:"Idle"})
   anchors.fill:parent;visible:root.selected===key
   function navigate(url){if(typeof url!=="string")return;if(!/^(https?:|about:blank(?:$|#))/i.test(url)){var external=browserHost.externalProtocolPromptUrl(url);if(external)browserHost.event({type:"external-protocol",threadId:tab.threadId,tabId:tab.tabId,url:external});return}web.url=url}
   WebEngineView{id:web;anchors.fill:parent;profile:browserProfile;focus:tab.visible;zoomFactor:browserData.zoomFactor || 1
    settings.localContentCanAccessFileUrls:false;settings.localContentCanAccessRemoteUrls:false;settings.javascriptCanOpenWindows:true;settings.fullScreenSupportEnabled:true
    onPermissionRequested:function(p){if(p.permissionType===WebEnginePermission.PermissionType.ClipboardReadWrite||p.permissionType===WebEnginePermission.PermissionType.Notifications||p.permissionType===WebEnginePermission.PermissionType.Geolocation)p.grant();else p.deny()}
    onFullScreenRequested:function(r){r.accept()}
    onContextMenuRequested:function(r){r.accepted=true;browserHost.event({type:"context-menu",threadId:tab.threadId,tabId:tab.tabId,x:r.position.x,y:r.position.y})}
    onFileDialogRequested:function(r){r.accepted=true;r.dialogReject();browserHost.event({type:"file-dialog",threadId:tab.threadId,tabId:tab.tabId})}
    onNewWindowRequested:function(r){browserHost.event({type:"new-window",threadId:tab.threadId,tabId:tab.tabId,url:r.requestedUrl.toString()})}
    onNavigationRequested:function(r){var url=r.url.toString();if(!/^(https?:|about:blank(?:$|#))/.test(url)){r.action=WebEngineNavigationRequest.IgnoreRequest;var external=browserHost.externalProtocolPromptUrl(url);if(external)browserHost.event({type:"external-protocol",threadId:tab.threadId,tabId:tab.tabId,url:external})}}
    onLoadingChanged:function(info){var url=web.url.toString();if(info.status===WebEngineView.LoadStartedStatus)tab.navigation={_tag:"Loading",url:url,title:web.title};else if(info.status===WebEngineView.LoadSucceededStatus)tab.navigation={_tag:"Success",url:url,title:web.title};else if(info.status===WebEngineView.LoadFailedStatus)tab.navigation={_tag:"LoadFailed",url:url,title:web.title,code:info.errorCode,description:info.errorString};root.emitState(tab);browserHost.requestFrame()}
    onTitleChanged:{if(tab.navigation._tag!=="Idle")tab.navigation=Object.assign({},tab.navigation,{title:web.title});root.emitState(tab)}
    onCanGoBackChanged:root.emitState(tab)
    onCanGoForwardChanged:root.emitState(tab)
    onScrollPositionChanged:browserHost.requestFrame()
   }
  }
 }
}
