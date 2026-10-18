import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var fixture:null
 property var lastThread:null
 function check(ok,msg){if(!ok){console.error("SETTLE_NAVIGATION_QML_FAIL: "+msg);Qt.exit(1);throw new Error(msg)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend
  function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}
  function onSettledLastThread(thread){root.lastThread=thread}
 }
 function row(id,extra){return Object.assign({id:id,key:"fixture/"+id,environmentId:"fixture",projectId:"p",projectKey:"fixture/p",title:id,project:"Project",updatedAt:100,modelSelection:{instanceId:"codex",model:"m"}},extra || {})}
 function setup(rows,selected){var b=Test.T3Backend;b.managementRequest=null;b.managementBusy=false;b.settleNavigation=null;root.lastThread=null;fixture={threads:rows,projects:[{id:"p",key:"fixture/p",environmentId:"fixture",title:"Project"}],environments:[]};b.state=fixture;b.selectThread("fixture",selected);b.patchDraft("fixture/"+selected,{text:"Preserved draft",loading:false});b.settleNavigationOrder=rows}
 function beginSettle(id){var b=Test.T3Backend;b.manage({action:"settle",environment:"fixture",thread:id});return b.managementRequest.requestId}
 function finish(id,error){var b=Test.T3Backend;b.action({op:"manage",environment:"fixture",requestId:id,error:error || "",result:{action:"settle",threadId:b.managementRequest.thread}})}
 Timer{interval:350;running:true;repeat:true;onTriggered:{var b=Test.T3Backend;if(!b.viewStateLoaded)return;
 if(root.phase===0){
  // Closed panels use the same captured state order; visible panels supply sidebar order.
  setup([row("a"),row("parked",{settled:true}),row("sleep",{snoozed:true}),row("deleted",{deletedAt:1}),row("b")],"a");
  var id=root.beginSettle("a");b.documentOpen={requestId:"synthetic-document",busy:false,error:""};b.inlineDocuments={synthetic:{requestId:"synthetic-inline"}};
  b.state=Object.assign({},fixture,{threads:fixture.threads.map(function(t){return t.id==="a"?Object.assign({},t,{settled:true}):t})});finish(id);
  check(b.selectedThread==="b","success skips settled, snoozed and deleted candidates");check(!b.documentOpen.requestId&&!Object.keys(b.inlineDocuments).length,"leaving closes document viewers");check(b.drafts["fixture/a"].text==="Preserved draft","source draft retained");
  setup([row("a"),row("b"),row("c")],"a");id=root.beginSettle("a");b.selectThread("fixture","c");finish(id);check(b.selectedThread==="c","user navigation wins");
  setup([row("a"),row("b")],"a");id=root.beginSettle("a");b.selectThread("fixture","b");b.selectThread("fixture","a");finish(id);check(b.selectedThread==="a","away-and-back navigation also wins");
  setup([row("a"),row("b")],"a");id=root.beginSettle("b");finish(id);check(b.selectedThread==="a","settling in background leaves selection");
  setup([row("a"),row("b")],"a");id=root.beginSettle("a");finish(id,"rpc_rejected");check(b.selectedThread==="a"&&!b.managementRequest,"failed settle leaves selection and clears plan");
  setup([row("a"),row("b")],"b");id=root.beginSettle("b");finish(id);check(b.selectedThread==="a","navigation wraps");
  setup([row("a"),row("b")],"a");id=root.beginSettle("a");b.state=Object.assign({},fixture,{threads:[row("a"),row("b",{settled:true})]});finish(id);check(!b.selectedKey&&lastThread.projectKey==="fixture/p","revalidate next candidate; last falls back to same project without creating a thread");
  setup([row("a"),row("b")],"a");id=root.beginSettle("a");finish(id,"rpc_outcome_unknown");check(b.managementRequest&&b.selectedThread==="a","unknown outcome retains retry");b.selectThread("fixture","b");b.selectThread("fixture","a");b.manage({});check(b.managementRequest.requestId===id,"retry keeps request identity");finish(id);check(b.selectedThread==="a","late retry does not steal selection");
  // The panel supplies its rendered ordering and opens a project-scoped dialog on last thread.
  setup([row("a",{activeAnchorAt:300,inboxAt:300}),row("b",{activeAnchorAt:100,inboxAt:100}),row("c",{activeAnchorAt:200,inboxAt:200})],"a");panel.open();root.phase=1;return
 }
 if(root.phase===1){var id=root.beginSettle("a");check(b.settleNavigation.candidates[0]==="fixture/c","captures displayed sidebar order");finish(id);check(b.selectedThread==="c","displayed next thread selected");
  setup([row("a")],"a");id=root.beginSettle("a");finish(id);root.phase=2;return
 }
 if(root.phase===2){stop();var dialog=checks.findChild(panel,"t3NewChatDialog");check(dialog&&dialog.visible&&!b.managementRequest,"last thread opens native new chat dialog without dispatch");dialog.contentItem.grabToImage(function(result){result.saveToFile("/tmp/ot3-settle-navigation.png");dialog.close();panel.close();console.log("SETTLE_NAVIGATION_QML_PASS: success, wrap, filters, viewer closure, draft retention, failure, background, navigation races, retry and last-project dialog");Qt.callLater(Qt.quit)})
 }}}
 TestCase{id:checks;when:false}
}
