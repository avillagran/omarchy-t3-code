import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/PreviewSessions.js" as Sessions
ShellRoot {
 id:root
 function check(v,m){if(!v){console.error("PREVIEW_ACTIONS_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 function snapshot(tab){return {threadId:"thread",tabId:tab,updatedAt:"2026-10-17T00:00:00Z",navStatus:{_tag:"Idle"},canGoBack:false,canGoForward:false}}
 function list(){Sessions.receive(fake,{kind:"list",requestId:fake.previewWatch.requestId,environment:"env",thread:"thread",originKey:"origin",canOperate:true,serverBrowser:true,result:{serverEpoch:"epoch",revision:1,sessions:[snapshot("a"),snapshot("b")]}})}
 function ack(input,state,value){return {op:"preview-action",requestId:input.requestId,environment:"env",thread:"thread",result:{environment:"env",thread:"thread",originKey:"origin",serverEpoch:"epoch",action:input.action,state:state,error:state==="complete"?"":"preview_command_uncertain",snapshot:value||null}}}
 QtObject {
  id:fake
  property bool panelVisible:true
  property bool previewVisible:true
  property bool previewWorkerReady:true
  property string selectedEnvironment:"env"
  property string selectedThread:"thread"
  property var previewStores:({})
  property var previewState:null
  property var previewWatch:({signature:""})
  property var previewOperations:({})
  property var previewCommands:({})
  property var commands:[]
  property int counter:0
  function uuid(){return "request-"+(++counter)}
  function send(d){commands=commands.concat([d])}
 }
 Timer{interval:50;running:true;onTriggered:{
  check(typeof Test.T3Backend.previewAction==="function","actual native bridge exposes lifecycle actions");
  Sessions.sync(fake,false);list();var key=Sessions.operationKey("env","thread");
  fake.previewWatch=Object.assign({},fake.previewWatch,{canOperate:false});check(!Sessions.command(fake,"open",{}),"read-only state cannot dispatch");fake.previewWatch=Object.assign({},fake.previewWatch,{canOperate:true});
  check(Sessions.command(fake,"close",{tabId:"a",environment:"forged",thread:"forged",serverEpoch:"forged",originKey:"forged"}),"close accepted");
  var close=fake.commands[fake.commands.length-1];check(close.environment==="env"&&close.thread==="thread"&&close.serverEpoch==="epoch"&&close.originKey==="origin","captured scope overrides caller fields");check(!fake.previewState.sessions.a,"native optimistic close suppresses tab");
  check(!Sessions.command(fake,"open",{}),"pending lifecycle command serializes controls");
  Sessions.receipt(fake,{requestId:close.requestId,environment:"other",thread:"thread",error:"permission_denied"});check(fake.previewCommands[close.requestId],"foreign receipt cannot consume command");
  Sessions.receipt(fake,{requestId:close.requestId,environment:"env",thread:"thread",error:"preview_server_restarted"});check(fake.previewState.sessions.a&&fake.previewOperations[key].state==="failed","known preflight failure restores native optimistic close");
  var store=fake.previewStores[JSON.stringify(["env","origin"])],ref={environmentId:"env",threadId:"thread"};store.setActivePreviewTab(ref,"b");fake.previewState=store.readThreadPreviewState(ref);
  check(Sessions.command(fake,"resize",{tabId:"a",viewport:{_tag:"fill"}}),"background resize dispatch");var resize=fake.commands[fake.commands.length-1];var resized=Object.assign({},snapshot("a"),{updatedAt:"2026-10-17T00:00:01Z",viewport:{_tag:"fill"}});Sessions.receipt(fake,ack(resize,"complete",resized));check(fake.previewState.activeTabId==="b"&&fake.previewState.sessions.a.viewport._tag==="fill","native background mutation does not steal focus");
  check(Sessions.command(fake,"open",{}),"open dispatch");var opened=fake.commands[fake.commands.length-1];Sessions.receipt(fake,ack(opened,"complete",snapshot("new")));check(fake.previewState.activeTabId==="new","open response uses native active-tab transition");
  check(Sessions.command(fake,"close",{tabId:"a"}),"second close dispatch");var uncertain=fake.commands[fake.commands.length-1];Sessions.receipt(fake,ack(uncertain,"uncertain",null));check(!fake.previewState.sessions.a&&fake.previewOperations[key].state==="uncertain","unknown close retains suppression until authoritative reconciliation");
  var count=fake.commands.length;check(!Sessions.command(fake,"open",{})&&fake.commands.length===count,"uncertain mutation cannot be replayed automatically");Sessions.acknowledge(fake);check(fake.commands[fake.commands.length-1].op==="preview-watch"&&fake.commands.length===count+1,"explicit acknowledgement requests state only");list();check(fake.previewState.sessions.a,"fresh list may restore an uncertain close that the server did not execute");
  check(Sessions.command(fake,"open",{}),"new explicit command after acknowledgement");var pending=fake.commands[fake.commands.length-1];Sessions.lost(fake);check(fake.previewOperations[key].state==="uncertain"&&Object.keys(fake.previewCommands).length===0,"worker loss retains unknown outcome and removes replayable command");Sessions.receipt(fake,ack(pending,"complete",snapshot("late")));check(!fake.previewState.sessions.late,"late previous worker receipt ignored");
  console.log("PREVIEW_ACTIONS_QML_PASS: native bridge, captured scope, read-only admission, close rollback, background focus, uncertain outcomes, explicit reconciliation and worker-loss no replay");Qt.quit();
 }}
 Timer{interval:4000;running:true;onTriggered:{console.error("PREVIEW_ACTIONS_QML_FAIL timeout");Qt.exit(1)}}
}
