import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("PROJECT_SIDEBAR_QML_FAIL: "+msg);Qt.exit(1)}}
 function thread(id,env,project,state,stamp){return {id:id,key:env+"/"+id,environmentId:env,environmentName:env==="local"?"M2":"NODE",project:project,projectKey:env+"/"+project,projectId:project,title:id,sidebarStatus:state,status:state==="working"?"running":"completed",working:state==="working",pending:state==="approval",repliedAt:stamp}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,tree=panel.projectThreads
  if(phase===0){
   if(!b.viewStateLoaded)return
   fixture={threads:[thread("build","local","Alpha","working",80),thread("answer","fixture","Alpha","approval",100),thread("done","fixture","Alpha","done",70),thread("other","fixture","Beta","done",90),thread("older","local","Alpha","working",40)],projects:[{id:"Alpha",key:"local/Alpha",environmentId:"local",title:"Alpha",repositoryIdentity:{canonicalKey:"alpha"}},{id:"Alpha",key:"fixture/Alpha",environmentId:"fixture",title:"Alpha",repositoryIdentity:{canonicalKey:"alpha"}},{id:"Beta",key:"fixture/Beta",environmentId:"fixture",title:"Beta",repositoryIdentity:{canonicalKey:"beta"}}],environments:[{id:"local",name:"M2",state:"ready"},{id:"fixture",name:"NODE",state:"ready"}]}
   b.state=fixture;b.catalogs={local:{canWrite:true,capabilities:{},models:[]},fixture:{canWrite:true,capabilities:{},models:[]}}
   panel.open();b.selectedEnvironment="local";b.selectedThread="build";b.selectedKey="local/build";b.patchDraft(b.selectedKey,{loading:false,text:"Safe unsent draft"})
   keys.findChild(panel,"sidebarLayoutProjects").triggered();phase=1
  }else if(phase===1){
   if(!tree)return
   check(panel.projectLayout&&b.viewState.interfaceLayout==="projects","third mode activated and persisted by quick menu")
   var projects=tree.entries.filter(function(e){return e.kind==="project"})
   check(projects.length===2&&projects[0].title==="Alpha"&&projects[0].count===4,"repository grouped across hosts, busiest project first")
   var alpha=tree.entries.filter(function(e){return e.projectKey===projects[0].projectKey})
   check(alpha.map(function(e){return e.kind==="thread"?e.thread.id:e.kind==="project"?e.title:e.state}).join()==="Alpha,waiting,answer,working,build,older,done,done","project contains states and their ordered threads: "+alpha.map(function(e){return e.kind==="thread"?e.thread.id:e.state}).join())
   check(!keys.findChild(panel,"threadList").visible,"global state shelves hidden in project mode")
   var index=tree.entries.findIndex(function(e){return e.key==="local/build"});tree.positionViewAtIndex(index,ListView.Center);phase=2
  }else if(phase===2){
   var index=tree.entries.findIndex(function(e){return e.key==="local/build"}),item=tree.itemAtIndex(index)
   check(item&&item.thread&&item.thread.id==="build","project tree reuses actual thread delegate")
   check(keys.findChild(item,"threadFocusHost").visible,"thread host remains visible")
   var section=tree.entries.find(function(e){return e.kind==="state"&&e.state==="working"});tree.toggle(section.key);phase=3
  }else if(phase===3){
   check(!tree.visibleThreads.some(function(t){return t.id==="build"||t.id==="older"}),"collapsing a state hides its own threads")
   check(tree.visibleThreads.some(function(t){return t.id==="answer"}),"other states remain open")
   var alpha=tree.entries.find(function(e){return e.kind==="project"&&e.title==="Alpha"});tree.toggle(alpha.key);phase=4
  }else if(phase===4){
   check(tree.visibleThreads.length===1&&tree.visibleThreads[0].id==="other","project collapse hides all its states")
   tree.reveal("local/build");phase=5
  }else if(phase===5){
   check(tree.visibleThreads.some(function(t){return t.id==="build"}),"reveal opens project and state ancestors")
   panel.hostFilter="fixture";phase=6
  }else if(phase===6){
   check(tree.visibleThreads.length===3&&!tree.visibleThreads.some(function(t){return t.environmentId==="local"}),"host filters apply inside project hierarchy")
   b.query="other";phase=7
  }else if(phase===7){
   check(tree.entries.filter(function(e){return e.kind==="project"}).length===1&&tree.visibleThreads[0].id==="other","search removes empty projects and states")
   b.query="";panel.hostFilter="";tree.reveal("fixture/done");phase=8
  }else if(phase===8){
   tree.currentIndex=tree.entries.findIndex(function(e){return e.key==="fixture/done"});tree.activate();check(b.selectedThread==="done"&&b.selectedEnvironment==="fixture","tree opens real thread")
   b.selectedEnvironment="local";b.selectedThread="build";b.selectedKey="local/build"
   keys.findChild(panel,"sidebarLayoutClassic").triggered();phase=9
  }else if(phase===9){
   check(!panel.projectLayout&&!panel.projectThreads&&keys.findChild(panel,"threadList").visible,"classic restores previous sidebar and releases tree")
   check(b.draft.text==="Safe unsent draft","switching layouts preserves draft")
   console.log("PROJECT_SIDEBAR_QML_PASS: third mode, projects across hosts, nested ordered states, real thread delegates, project/state collapse, reveal, filters, search, navigation and reversible default");Qt.quit()
  }
 }}
}
