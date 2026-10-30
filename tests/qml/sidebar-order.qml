import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/SidebarModel.js" as Sidebar
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("SIDEBAR_ORDER_QML_FAIL "+msg);Qt.exit(1)}}
 function row(id,inbox,updated){return {id:id,key:"fixture/"+id,environmentId:"fixture",project:"Repo",title:id,status:"completed",inboxAt:inbox,updatedAt:updated}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({sidebarCompact:true});fixture={threads:[row("old",100,9000),row("recent",2000,50),Object.assign(row("running",500,500),{working:true,status:"running",lastSentAt:400}),Object.assign(row("pinned",10,10),{pinned:true,pinnedAt:10}),Object.assign(row("settled-old",10,9000),{settled:true,settledSortAt:100}),Object.assign(row("settled-recent",10,50),{settled:true,settledSortAt:2000})],projects:[],environments:[]};b.state=fixture;panel.open();phase=1
  }else if(phase===1){check(panel.activeRows.map(t=>t.id).join(",")==="recent,old"&&panel.pinnedRows.map(t=>t.id).join(",")==="pinned","recent completion outranks old metadata update, pins in their own group");check(panel.settledRows[0].id==="settled-recent","history ordered by completion/settlement");var tracker=Sidebar.observeReturns(null,fixture.threads,1);check(!Object.keys(tracker.returned).length,"initial mount does not invent returns");fixture=Object.assign({},fixture,{threads:fixture.threads.map(t=>t.id==="running"?Object.assign({},t,{working:false,status:"completed",inboxAt:2500}):t)});b.state=fixture;phase=2
  }else if(phase===2){check(panel.activeRows[0].id==="running","leaving Working surfaces immediately");check(b.inboxReturnState.returned["fixture/running"]>0,"unstamped background return is remembered");var before=JSON.stringify(b.inboxReturnState.returned);b.state=Object.assign({},fixture);check(JSON.stringify(b.inboxReturnState.returned)===before,"ordinary refresh does not reshuffle");var withQuestion=Sidebar.observeReturns({working:{"fixture/q":true},returned:{}},[Object.assign(row("q",0,0),{working:true,pending:true,reason:"request"})],4000);check(withQuestion.returned["fixture/q"]===4000,"mid-run question returns to inbox");check(!Object.keys(Sidebar.observeReturns(withQuestion,[],5000).returned).length,"deleted returns are pruned");panel.close();console.log("SIDEBAR_ORDER_QML_PASS: native completion order, pinned/settled lists, live Working return and stable refresh");Qt.quit()}
 }}
}
