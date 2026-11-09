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
 Connections{
  target:typeof browserPrivatePipe==="undefined"?null:browserPrivatePipe
  function onCommand(line){
   let value;try{value=JSON.parse(line)}catch(_){browserPrivatePipe.close();return}
   const tab=tabs[tabKey(value.threadId,value.tabId)]
   if(tab&&tab.automationItem)tab.automationItem.accept(value)
  }
  function onEnded(){for(const tab of Object.values(root.tabs))tab.disableAutomation()}
 }
 function command(c){
  var key=tabKey(c.threadId,c.tabId),tab=tabs[key]
  if(c.type==="deactivate"){selected="";surfaceId="";browserHost.requestFrame();return}
  if(c.type==="open"){
   if(!c.threadId||!c.tabId)return
   if(!tab){tab=tabComponent.createObject(root.contentItem,{threadId:c.threadId,tabId:c.tabId,key:key});tabs[key]=tab}
   tab.serverControlled=c.serverControlled===true
   if(c.automation===true)tab.enableAutomation(c.automationInstance)
   if(c.url)tab.navigate(c.url)
   if(c.background!==true){selected=key;surfaceId=c.surfaceId || "";tab.web.forceActiveFocus()}
   return
  }
  if(c.type==="activate"&&tab){tab.serverControlled=c.serverControlled===true;if(c.automation===true)tab.enableAutomation(c.automationInstance);selected=key;surfaceId=c.surfaceId || "";tab.web.forceActiveFocus();browserHost.requestFrame();return}
  if(c.type==="close-tab"&&tab){delete tabs[key];tab.destroy();if(selected===key)selected=Object.keys(tabs)[0] || "";browserHost.event({type:"closed",threadId:c.threadId,tabId:c.tabId});return}
  if(!tab)return
  if(c.type==="automation"){tab.enableAutomation(c.automationInstance);return}
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
   property bool serverControlled:false
   readonly property bool selected:root.selected===key
   property bool automationPainting:false
   readonly property var automationItem:automationLoader.item
   property string automationInstance:""
   function disableAutomation(){automationLoader.active=false;automationPainting=false;automationInstance=""}
   function enableAutomation(instance){
    if(typeof instance!=="string"||!instance||!browserPrivatePipe||!browserPrivatePipe.open)return
    if(automationInstance===instance&&automationLoader.item)return
    automationLoader.active=false;automationPainting=false;automationInstance=instance
    if(!String(web.url))web.url="about:blank"
    automationLoader.setSource("T3BrowserAutomation.qml",{owner:tab,instance:instance});automationLoader.active=true
   }
   Loader{id:automationLoader;active:false
    onLoaded:{item.painting.connect(function(active){tab.automationPainting=active})}
   }
   anchors.fill:parent;visible:tab.selected||tab.automationPainting;opacity:tab.selected?1:0
   Component.onDestruction:browserHost.cancelPermissions(tab.threadId,tab.tabId)
   function navigate(url){if(typeof url!=="string")return;if(!/^(https?:|about:blank(?:$|#))/i.test(url)){var external=browserHost.externalProtocolPromptUrl(url);if(external)browserHost.event({type:"external-protocol",threadId:tab.threadId,tabId:tab.tabId,url:external});return}web.url=url}
   WebEngineView{id:web;anchors.fill:parent;profile:browserProfile;focus:tab.selected;zoomFactor:browserData.zoomFactor || 1
    settings.localContentCanAccessFileUrls:false;settings.localContentCanAccessRemoteUrls:false;settings.javascriptCanOpenWindows:true;settings.fullScreenSupportEnabled:true
    onPermissionRequested:function(p){browserHost.permission(p,tab.threadId,tab.tabId)}
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
