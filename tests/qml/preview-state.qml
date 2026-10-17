import QtQuick
import Quickshell
import "UnderTest/ui/js/NativePreviewState.js" as Native
ShellRoot {
 function check(value,message){if(!value){console.error("PREVIEW_STATE_QML_FAIL "+message);Qt.exit(1);throw Error(message)}}
 function snapshot(tab,time){return {threadId:"thread",tabId:tab,updatedAt:time||"2026-10-17T00:00:00Z",navStatus:{_tag:"Success",url:"https://example.test/"+tab,title:tab},canGoBack:false,canGoForward:false}}
 Timer{interval:1;running:true;onTriggered:{
  var changes=0,store=Native.createPreviewStore(function(){changes++}),ref={environmentId:"env",threadId:"thread"},other={environmentId:"other",threadId:"thread"};
  var a=snapshot("a"),b=snapshot("b","2026-10-17T00:00:01Z");
  store.reconcilePreviewServerSessions(ref,{serverEpoch:"one",revision:2,sessions:[a,b]});
  check(store.readThreadPreviewState(ref).activeTabId==="b","latest fallback");
  store.setActivePreviewTab(ref,"a");store.updatePreviewServerSnapshot(ref,Object.assign({},b,{viewport:{_tag:"fill"}}));
  check(store.readThreadPreviewState(ref).activeTabId==="a","background mutation preserves focus");
  store.beginPreviewSessionClose(ref,"a");store.reconcilePreviewServerSessions(ref,{serverEpoch:"one",revision:3,sessions:[a,b]});
  check(!store.readThreadPreviewState(ref).sessions.a,"authoritative list respects local close suppression");
  store.applyPreviewServerEvent(ref,{type:"closed",serverEpoch:"one",revision:5,tabId:"a"});
  store.applyPreviewServerEvent(ref,{type:"opened",serverEpoch:"one",revision:4,snapshot:a});
  check(!store.readThreadPreviewState(ref).sessions.a,"older event cannot resurrect closed tab");
  store.applyPreviewServerEvent(ref,{type:"opened",serverEpoch:"other-server",revision:99,snapshot:a});
  check(!store.readThreadPreviewState(ref).sessions.a,"foreign epoch event rejected");
  var overlay={hasWebContents:true,canGoBack:true,canGoForward:false,loading:false,zoomFactor:1,pictureInPicture:false,colorScheme:"system",audioMuted:false,audible:false,controller:"human",favicon:null};
  store.applyPreviewDesktopState(ref,"b",overlay);var before=changes;store.applyPreviewDesktopState(ref,"b",Object.assign({},overlay));check(changes===before,"equal overlay does not publish");
  store.reconcilePreviewServerSessions(ref,{serverEpoch:"one",revision:6,sessions:[b]});check(store.readThreadPreviewState(ref).desktopOverlay.canGoBack,"same-epoch list preserves live tab overlay");
  store.applyPreviewServerEvent(ref,{type:"failed",serverEpoch:"one",revision:7,tabId:"b",url:"https://example.test/file",title:"File",code:-2,description:"download",createdAt:"2026-10-17T00:00:03Z",download:{id:"download-id",fileName:"file.pdf"}});
  check(store.readThreadPreviewState(ref).snapshot.navStatus.download.fileName==="file.pdf","original conditional download metadata survives Qt spread conversion");
  store.reconcilePreviewServerSessions(ref,{serverEpoch:"two",revision:0,sessions:[a]});
  var state=store.readThreadPreviewState(ref);check(state.serverRevision===0&&state.snapshot.tabId==="a"&&state.suppressedTabIds.size===0&&Object.keys(state.desktopByTabId).length===0,"restart resets raw tab overlay/suppression and accepts lower revision");
  check(!store.readThreadPreviewState(other).listLoaded,"same thread ID isolated across environments");
  var one={environmentId:"x/y",threadId:"z"},two={environmentId:"x",threadId:"y/z"};store.applyPreviewServerSnapshot(one,a);check(!store.readThreadPreviewState(two).snapshot,"unambiguous tuple key");
  var unsubscribe=store.subscribe(ref,function(){before++});before=0;store.rememberPreviewUrl(ref,"https://example.test/recent");unsubscribe();store.rememberPreviewUrl(ref,"https://example.test/after");check(before===1,"subscriptions dispose");
  for(var i=0;i<15;i++)store.rememberPreviewUrl(ref,"https://example.test/"+i);check(store.readThreadPreviewState(ref).recentlySeenUrls.length===10,"original recent URL cap");
  console.log("PREVIEW_STATE_QML_PASS: original reducers run in Qt; close/list/event races, focus, epochs, overlays, environment isolation and subscriptions");Qt.quit();
 }}
}
