import QtQuick
import QtTest
import qs.Commons
import Quickshell
import "UnderTest/ui/qml" as Test
// O3 sidebar: compact rows by default, latest reply first, host colour/name
// from the Environments popup, marquee titles and the Sidebar preferences.
ShellRoot{
 id:root;property int phase:0;property bool done:false;property var fixture:null
 function check(ok,msg){if(!ok){console.error("O3_SIDEBAR_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 function row(id){var list=panel.sidebarView;for(var i=0;i<list.count;i++){var r=list.itemAtIndex(i);if(r&&r.thread&&r.thread.id===id)return r}return null}
 Timer{interval:200;running:!root.done;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.rememberView({hostStyles:{}})
   b.state={threads:[
    {id:"old",key:"fixture/old",environmentId:"fixture",environmentName:"NODE",project:"P",title:"Old reply but sent late, with a very long title that does not fit in the row at all",status:"completed",repliedAt:100,inboxAt:900,lastSentAt:900,activeAnchorAt:900},
    {id:"new",key:"fixture/new",environmentId:"fixture",environmentName:"NODE",project:"P",title:"New reply",status:"completed",repliedAt:500,inboxAt:500,activeAnchorAt:1},
    {id:"pin",key:"local/pin",environmentId:"local",environmentName:"mbp",project:"P",title:"Pinned",status:"completed",pinned:true,repliedAt:1}],
    projects:[],environments:[{id:"local",name:"mbp",state:"ready"},{id:"fixture",name:"NODE",state:"ready"}]}
   fixture=b.state;panel.open();phase=1
  }else if(phase===1){
   var ids=panel.activeRows.map(function(t){return t.id})
   root.check(ids.join()==="pin,new,old","pinned first, then latest reply: "+ids.join())
   var r=root.row("new");if(!r)return;root.check(!!r&&r.slim&&r.compactRows,"rows are compact by default")
   root.check(r.showHost&&!r.hostLook.color,"another host shows its icon, with no colour by default")
   var sub=keys.findChild(r,"threadCompactProject");root.check(r.subtitled&&sub&&sub.visible&&sub.text==="P"&&r.height>Style.space(40),"compact rows show the project under the title")
   b.rememberView({hostStyles:{fixture:{color:"#5aa2e6",showName:true,icon:"laptop"}}});phase=11
  }else if(phase>=11&&phase<16){
   phase++;if(phase===16)keys.findChild(panel,"t3PanelLayout").grabToImage(function(g){g.saveToFile("/tmp/o3-sidebar-compact.png")})
  }else if(phase===16){
   b.rememberView({threadTitleOverflow:"marquee",threadRowSize:"large"});phase=2
  }else if(phase===2){
   var r2=root.row("new");if(!r2)return
   root.check(!r2.slim,"large rows on request")
   root.check(String(r2.hostLook.color)==="#5aa2e6"&&r2.hostLook.icon==="laptop","host style applies to its threads")
   var strip=keys.findChild(r2,"threadHostStrip");root.check(strip&&strip.visible,"coloured host draws an edge strip")
   var name=keys.findChild(r2,"threadHostName");root.check(name&&name.visible&&name.text==="NODE","host name shows when asked")
   root.check(r2.marquee,"marquee preference reaches the rows")
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(g){g.saveToFile("/tmp/o3-sidebar-large.png")})
   b.rememberView({threadOrder:"t3"});phase=3
  }else if(phase===3){
   root.check(!panel.orderByReply,"T3's own order on request")
   b.rememberView({threadOrder:"reply",threadRowSize:"compact",threadTitleOverflow:"elide",hostStyles:{}})
   // The same repository on two hosts is one group; another repository is another.
   var repo=function(k){return {canonicalKey:k,rootPath:"/w/"+k}}
   b.state={threads:[
    {id:"a1",key:"local/a1",environmentId:"local",project:"Alpha",projectKey:"local/alpha",title:"A on mbp",status:"completed",repliedAt:30},
    {id:"a2",key:"fixture/a2",environmentId:"fixture",project:"Alpha",projectKey:"fixture/alpha",title:"A on NODE",status:"completed",repliedAt:20},
    {id:"b1",key:"fixture/b1",environmentId:"fixture",project:"Beta",projectKey:"fixture/beta",title:"B",status:"completed",repliedAt:10}],
    projects:[{key:"local/alpha",environmentId:"local",title:"Alpha",workspaceRoot:"/w/alpha",repositoryIdentity:repo("alpha")},{key:"fixture/alpha",environmentId:"fixture",title:"Alpha",workspaceRoot:"/w/alpha",repositoryIdentity:repo("alpha")},{key:"fixture/beta",environmentId:"fixture",title:"Beta",workspaceRoot:"/w/beta",repositoryIdentity:repo("beta")}],
    environments:[{id:"local",name:"mbp",state:"ready"},{id:"fixture",name:"NODE",state:"ready"}]}
   fixture=b.state;phase=4
  }else if(phase===4){
   root.check(panel.groupChips.length===2&&panel.groupChips[0].title==="Alpha"&&panel.groupChips[0].count===2,"one chip per group, busiest first: "+JSON.stringify(panel.groupChips))
   root.check(String(panel.groupTints["local/alpha"])===String(panel.groupTints["fixture/alpha"]),"grouped projects share a colour")
   var chips=keys.findChild(panel,"t3GroupChips");root.check(chips&&chips.visible,"chips show with two or more groups")
   var chip=chips.itemAtIndex(1);root.check(!!chip,"Beta chip exists");keys.mouseClick(chip,chip.width/2,chip.height/2);phase=5
  }else if(phase===5){
   root.check(panel.projectFilter===panel.groupChips.find(function(c){return c.title==="Beta"}).key,"a chip click filters by its group")
   root.check(panel.activeRows.length===1&&panel.activeRows[0].id==="b1","only that group's threads show: "+panel.activeRows.map(function(t){return t.id}).join())
   var chip2=keys.findChild(panel,"t3GroupChips").itemAtIndex(panel.groupChips.findIndex(function(c){return c.key===panel.projectFilter}));keys.mouseClick(chip2,chip2.width/2,chip2.height/2);phase=6
  }else if(phase===6){
   root.check(panel.projectFilter==="","clicking the active chip clears the filter")
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(g){g.saveToFile("/tmp/o3-chips.png")})
   console.log("O3_SIDEBAR_QML_PASS");root.done=true
  }
 }}
}
