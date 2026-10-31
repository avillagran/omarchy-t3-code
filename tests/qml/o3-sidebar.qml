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
   root.check(ids.join()==="new,old","latest reply first: "+ids.join())
   root.check(panel.pinnedRows.map(function(t){return t.id}).join()==="pin","pinned threads have their own group on top")
   var r=root.row("new");if(!r)return;root.check(!!r&&r.slim&&r.compactRows,"rows are compact by default")
   root.check(r.showHost&&!r.hostLook.color,"another host shows its icon, with no colour by default")
   var sub=keys.findChild(r,"threadCompactProject");root.check(r.subtitled&&sub&&sub.visible&&sub.text==="P"&&r.height>Style.space(40),"compact rows show the project under the title")
   // Sober by default: the project colour is a dot, with no wash behind the row.
   var dot=keys.findChild(r,"threadProjectDot");root.check(r.sober&&dot&&dot.visible,"sober rows mark the project with a dot")
   var wash=keys.findChild(r,"threadProjectWash");root.check(wash&&!wash.visible,"sober rows have no colour wash")
   var mark=keys.findChild(panel,"t3BrandMark");root.check(mark&&mark.sober,"the header is sober too")
   b.rememberView({hostStyles:{fixture:{color:"#5aa2e6",showName:true,icon:"laptop"}}});phase=11
  }else if(phase>=11&&phase<16){
   phase++;if(phase===16)keys.findChild(panel,"t3PanelLayout").grabToImage(function(g){g.saveToFile("/tmp/o3-sidebar-compact.png")})
  }else if(phase===16){
   b.rememberView({threadTitleOverflow:"marquee",threadRowSize:"large"});phase=2
  }else if(phase===2){
   var r2=root.row("new");if(!r2)return
   root.check(!r2.slim,"large rows on request")
   root.check(String(r2.hostLook.color)==="#5aa2e6"&&r2.hostLook.icon==="laptop","host style applies to its threads")
   var strip=keys.findChild(r2,"threadHostStrip");root.check(strip&&!strip.visible,"sober rows draw no host edge strip")
   b.rememberView({sidebarStyle:"vivid"});phase=21
  }else if(phase===21){
   var r3=root.row("new");if(!r3)return
   root.check(!r3.sober&&keys.findChild(r3,"threadHostStrip").visible,"vivid: a coloured host draws an edge strip")
   root.check(keys.findChild(r3,"threadProjectWash").visible,"vivid: rows get the project wash")
   var r2=r3
   var name=keys.findChild(r2,"threadHostName");root.check(name&&name.visible&&name.text==="NODE","host name shows when asked")
   root.check(r2.marquee,"marquee preference reaches the rows")
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(g){g.saveToFile("/tmp/o3-sidebar-large.png")})
   b.rememberView({threadOrder:"t3"});phase=3
  }else if(phase===3){
   root.check(!panel.orderByReply,"T3's own order on request")
   b.rememberView({threadOrder:"reply",threadRowSize:"compact",threadTitleOverflow:"elide",hostStyles:{},sidebarStyle:"sober"})
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
   var toggle=keys.findChild(panel,"t3GroupChipsToggle");root.check(toggle&&!toggle.visible&&chips.height<=Style.space(24)+1,"two chips fit on one line, no chevron")
   var chip=chips.itemAtIndex(1);root.check(!!chip,"Beta chip exists");keys.mouseClick(chip,chip.width/2,chip.height/2);phase=5
  }else if(phase===5){
   root.check(panel.projectFilter===panel.groupChips.find(function(c){return c.title==="Beta"}).key,"a chip click filters by its group")
   root.check(panel.activeRows.length===1&&panel.activeRows[0].id==="b1","only that group's threads show: "+panel.activeRows.map(function(t){return t.id}).join())
   var chip2=keys.findChild(panel,"t3GroupChips").itemAtIndex(panel.groupChips.findIndex(function(c){return c.key===panel.projectFilter}));keys.mouseClick(chip2,chip2.width/2,chip2.height/2);phase=6
  }else if(phase===6){
   root.check(panel.projectFilter==="","clicking the active chip clears the filter")
   // Many projects: one scrolling line folded, rows unfolded; scratch gets no chip.
   var many=JSON.parse(JSON.stringify(b.state));for(var n=0;n<8;n++){many.projects.push({key:"fixture/long"+n,environmentId:"fixture",title:"A rather long project name "+n,workspaceRoot:"/w/long"+n});many.threads.push({id:"l"+n,key:"fixture/l"+n,environmentId:"fixture",project:"Long "+n,projectKey:"fixture/long"+n,title:"T"+n,status:"completed"})}
   many.projects.push({key:"fixture/scratch",environmentId:"fixture",title:"No project",workspaceRoot:"/w/scratch",isScratch:true});many.threads.push({id:"s0",key:"fixture/s0",environmentId:"fixture",project:"No project",projectKey:"fixture/scratch",title:"S",status:"completed"})
   b.state=many;fixture=many;phase=61
  }else if(phase===61){
   var cl=keys.findChild(panel,"t3GroupChips"),tg=keys.findChild(panel,"t3GroupChipsToggle")
   root.check(!panel.groupChips.some(function(c){return c.title==="No project"}),"scratch workspaces get no chip")
   root.check(cl.overflow&&tg.visible&&!cl.expanded&&cl.height<=Style.space(24)+1,"folded chips stay on one line: "+cl.lineWidth+">"+cl.width)
   var last=cl.itemAtIndex(panel.groupChips.length-1);root.check(last&&last.y===0,"folded chips never wrap")
   panel.projectFilter=panel.groupChips[panel.groupChips.length-1].key;phase=62
  }else if(phase===62){
   var cl2=keys.findChild(panel,"t3GroupChips"),strip=cl2.children[0],lastItem=cl2.itemAtIndex(panel.groupChips.findIndex(function(c){return c.key===panel.projectFilter}))
   root.check(lastItem&&lastItem.x+lastItem.width<=strip.contentX+strip.width+1,"the active chip scrolls into view: "+lastItem.x+"+"+lastItem.width+" vs "+strip.contentX+"+"+strip.width)
   panel.projectFilter="";b.rememberView({chipsExpanded:true});phase=63
  }else if(phase===63){
   var cl3=keys.findChild(panel,"t3GroupChips");root.check(cl3.expanded&&cl3.height>Style.space(40)&&cl3.itemAtIndex(panel.groupChips.length-1).y>0,"unfolded chips wrap into rows: "+cl3.height)
   b.rememberView({chipsExpanded:false});fixture=JSON.parse(JSON.stringify(b.state));fixture.projects=fixture.projects.slice(0,3);fixture.threads=fixture.threads.slice(0,3);b.state=fixture;phase=64
  }else if(phase===64){
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(g){g.saveToFile("/tmp/o3-chips.png")})
   b.state={threads:[
    {id:"p1",key:"local/p1",environmentId:"local",project:"P",title:"Pinned one",status:"completed",pinned:true,pinOrderKey:"a",repliedAt:1},
    {id:"r1",key:"local/r1",environmentId:"local",project:"P",title:"Reply",status:"completed",repliedAt:5},
    {id:"q1",key:"local/q1",environmentId:"local",project:"P",title:"Question",status:"running",working:true,pending:true,reason:"request",repliedAt:3}],
    projects:[],environments:[{id:"local",name:"mbp",state:"ready"}]}
   fixture=b.state;phase=7
  }else if(phase===7){
   var ids2=function(rows){return rows.map(function(t){return t.id}).join()}
   root.check(ids2(panel.pinnedRows)==="p1"&&ids2(panel.activeRows)==="r1"&&ids2(panel.inputRows)==="q1","pinned, ready and waiting for input are separate groups: "+ids2(panel.pinnedRows)+"|"+ids2(panel.activeRows)+"|"+ids2(panel.inputRows))
   var input=keys.findChild(panel,"inputShelfHeader"),pinnedHead=keys.findChild(panel,"pinnedShelfHeader"),ready=keys.findChild(panel,"readyShelfHeader")
   root.check(input&&input.shown&&pinnedHead&&pinnedHead.shown&&ready&&ready.shown,"the groups have headers")
   root.check(input.mapToItem(panel,0,0).y>pinnedHead.mapToItem(panel,0,0).y,"pinned sits above, waiting for input below")
   // The pinned thread's answer is ready: it heads Ready.
   fixture=JSON.parse(JSON.stringify(b.state));fixture.threads[0].pending=true;fixture.threads[0].reason="review";b.state=fixture;phase=8
  }else if(phase===8){
   root.check(panel.activeRows.some(t=>t.id==="p1")&&!panel.pinnedRows.length&&panel.activeRows[0].id==="r1","a ready pinned thread joins Ready in reply order")
   b.selectThread("local","p1")
   // Opening it clears "done"; it stays in Ready while it has focus.
   fixture=JSON.parse(JSON.stringify(b.state));fixture.threads[0].pending=false;fixture.threads[0].reason=null;b.state=fixture;phase=9
  }else if(phase===9){
   root.check(panel.activeRows.some(t=>t.id==="p1"),"the open pinned thread stays in Ready")
   var sel=null;for(var i=0;i<panel.sidebarView.count;i++){var it=panel.sidebarView.itemAtIndex(i);if(it&&it.thread.id==="p1")sel=it}
   root.check(sel&&sel.selected&&keys.findChild(sel,"threadSelectedBar").visible,"the open thread is marked with a bar")
   root.check(!keys.findChild(sel,"threadPinIndicator"),"pinned rows carry no pin icon")
   b.selectThread("local","r1");phase=10
  }else if(phase===10){
   root.check(panel.pinnedRows.length===1&&panel.pinnedRows[0].id==="p1"&&panel.activeRows[0].id==="r1","once focus moves on it goes back to Pinned")
   // Right click on the header title opens the thread's menu, with Groups.
   var title=keys.findChild(panel,"t3HeaderThreadTitle");root.check(title&&title.visible,"header title shows")
   keys.mouseClick(title,Style.space(4),title.height/2,Qt.RightButton);phase=30
  }else if(phase===30){
   var hr=keys.findChild(panel,"t3HeaderThreadRow");root.check(hr&&hr.thread.key==="local/r1"&&hr.rowMenu&&hr.rowMenu.visible,"right click on the header title opens the thread menu")
   hr.setGroup(hr.thread,"Release");hr.rowMenu.close();phase=31
  }else if(phase===31){
   root.check(panel.threadGroups["local/r1"]==="Release"&&panel.threadGroupChips.length===1&&panel.threadGroupChips[0].count===1,"a thread joins a group from its menu")
   var gr=null;for(var k=0;k<panel.sidebarView.count;k++){var it2=panel.sidebarView.itemAtIndex(k);if(it2&&it2.thread.id==="r1")gr=it2}
   var label=gr?keys.findChild(gr,"threadCompactGroup"):null;root.check(label&&label.visible&&label.text==="Release","the group name shows after the project")
   panel.threadGroupFilter="Release";phase=32
  }else if(phase===32){
   root.check(panel.activeRows.length===1&&panel.activeRows[0].id==="r1"&&!panel.pinnedRows.length&&!panel.inputRows.length,"a group chip filters to its threads")
   keys.findChild(panel,"t3PanelLayout").grabToImage(function(g){g.saveToFile("/tmp/o3-groups.png")});phase=33
  }else if(phase===33){
   panel.threadGroupFilter="";b.rememberView({threadGroups:{}})
   // The whole project joins a group: all its threads, a thread's own group wins.
   var hr2=keys.findChild(panel,"t3HeaderThreadRow");hr2.setProjectGroup({key:"local/r1",projectKey:"local/proj"},"Infra");phase=35
  }else if(phase===35){
   root.check(b.viewState.groupProjects&&b.viewState.groupProjects["local/proj"]==="Infra","a project joins a group from the menu")
   fixture=JSON.parse(JSON.stringify(b.state));fixture.threads.forEach(function(t){t.projectKey="local/proj"});b.state=fixture
   b.rememberView({threadGroups:{"local/q1":"Solo"}});phase=36
  }else if(phase===36){
   var infra=panel.threadGroupChips.find(function(c){return c.group==="Infra"}),solo=panel.threadGroupChips.find(function(c){return c.group==="Solo"})
   root.check(infra&&infra.count===2&&solo&&solo.count===1,"project threads count in its group; a thread's own group wins: "+JSON.stringify(panel.threadGroupChips))
   panel.threadGroupFilter="Infra";phase=37
  }else if(phase===37){
   var shown=panel.pinnedRows.concat(panel.activeRows,panel.inputRows).map(function(t){return t.id}).sort().join()
   root.check(shown==="p1,r1","the group chip shows the project's threads: "+shown)
   panel.threadGroupFilter="";b.rememberView({threadGroups:{},groupProjects:{}});phase=34
  }else if(phase===34){
   console.log("O3_SIDEBAR_QML_PASS");root.done=true
  }
 }}
}
