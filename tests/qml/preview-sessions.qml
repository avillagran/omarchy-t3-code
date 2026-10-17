import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/PreviewSessions.js" as Sessions
ShellRoot {
 id:root
 function check(v,m){if(!v){console.error("PREVIEW_SESSIONS_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 function snapshot(tab){return {threadId:"thread",tabId:tab,updatedAt:"2026-10-17T00:00:00Z",navStatus:{_tag:"Idle"},canGoBack:false,canGoForward:false}}
 function receipt(kind,origin,result){return {environment:fake.selectedEnvironment,thread:fake.selectedThread,requestId:fake.previewWatch.requestId,originKey:origin,kind:kind,result:result,canOperate:false,serverBrowser:true}}
 QtObject {
  id:fake
  property bool panelVisible:false
  property bool previewVisible:false
  property bool previewWorkerReady:true
  property string selectedEnvironment:"env"
  property string selectedThread:"thread"
  property var previewStores:({})
  property var previewState:null
  property var previewWatch:({signature:""})
  property var commands:[]
  property int counter:0
  function uuid(){return "request-"+(++counter)}
  function send(d){commands=commands.concat([d])}
 }
 Timer{interval:50;running:true;onTriggered:{
  check(!Test.T3Backend.previewVisible&&Test.T3Backend.previewState===null,"actual backend keeps metadata observation inactive by default");
  Sessions.sync(fake,false);check(fake.commands.length===0,"hidden panel has no subscription");
  fake.panelVisible=true;fake.previewVisible=true;Sessions.sync(fake,false);check(fake.commands.length===1&&fake.commands[0].op==="preview-watch"&&fake.previewWatch.busy,"explicit visible view starts one scoped subscription");
  Sessions.sync(fake,false);check(fake.commands.length===1,"same selection does not restart stream");
  var list=receipt("list","origin-one",{serverEpoch:"server",revision:2,sessions:[snapshot("a")]});
  var premature=receipt("event","origin-one",{type:"opened",serverEpoch:"server",revision:3,snapshot:snapshot("b")});Sessions.receive(fake,premature);check(fake.previewState===null,"event cannot establish origin");
  Sessions.receive(fake,Object.assign({},list,{requestId:"stale"}));check(fake.previewState===null,"stale subscription receipt ignored");
  Sessions.receive(fake,list);check(fake.previewState.snapshot.tabId==="a"&&fake.previewWatch.connected&&!fake.previewWatch.canOperate,"read-only viewer accepts authoritative list");
  Sessions.receive(fake,premature);check(fake.previewState.snapshot.tabId==="b","native event reducer applies after list");
  Sessions.receive(fake,receipt("event","origin-other",{type:"closed",serverEpoch:"server",revision:99,tabId:"b"}));check(fake.previewState.snapshot.tabId==="b","foreign origin event ignored");
  Sessions.receive(fake,receipt("event","origin-one",{type:"closed",serverEpoch:"server",revision:4,tabId:"b"}));Sessions.receive(fake,premature);check(!fake.previewState.sessions.b,"original reducer rejects late resurrecting event");
  Sessions.receive(fake,receipt("disconnected","",null));check(!fake.previewWatch.connected&&fake.previewState.snapshot.tabId==="a","disconnect preserves readable metadata");
  Sessions.receive(fake,receipt("list","origin-two",{serverEpoch:"server",revision:0,sessions:[snapshot("replacement")]}));check(fake.previewState.snapshot.tabId==="replacement"&&fake.previewState.serverRevision===0,"changed origin has independent state even with same epoch and lower revision");
  var old=receipt("list","origin-two",{serverEpoch:"server",revision:10,sessions:[snapshot("old")]});fake.selectedThread="other";Sessions.sync(fake,false);Sessions.receive(fake,old);check(fake.previewState===null,"selection nonce rejects previous thread list");
  var nonce=fake.previewWatch.requestId;Sessions.sync(fake,true);check(fake.previewWatch.requestId!==nonce,"worker restart refreshes nonce");
  fake.previewVisible=false;Sessions.sync(fake,false);check(!fake.previewWatch.signature&&!fake.previewState&&fake.commands[fake.commands.length-1].thread==="","hiding view cancels native stream");
  check(fake.commands.every(function(c){return c.op==="preview-watch"}),"no browser launch, profile report or metadata mutation during observation");
  console.log("PREVIEW_SESSIONS_QML_PASS: native backend wiring, visibility/nonces, read-only state, original reducers, origin/thread isolation and reconnect cancellation");Qt.quit();
 }}
 Timer{interval:4000;running:true;onTriggered:{console.error("PREVIEW_SESSIONS_QML_FAIL timeout");Qt.exit(1)}}
}
