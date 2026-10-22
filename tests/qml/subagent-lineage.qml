import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("T3_SUBAGENT_LINEAGE_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Test.T3SubagentCards{id:cards;width:400;bridge:Test.T3Backend;cards:[{id:"s1",title:"Old title",status:"pending"},{id:"s2",title:"Fix",status:"completed"}]}
 Test.T3RunError{id:runError;width:400;bridge:Test.T3Backend;failure:({message:"Weekly cap reached for this account.",class:"usage_limit",code:"rate_limited"})}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   var base={environmentId:"fixture",projectId:"project",projectKey:"fixture/project",modelSelection:{instanceId:"fixture",model:"model"}}
   fixture={threads:[Object.assign({},base,{key:"fixture/parent",id:"parent",title:"Parent",updatedAt:1}),
    Object.assign({},base,{key:"fixture/c1",id:"c1",title:"Review",relationshipToParent:"subagent",parentThreadId:"parent",working:true,updatedAt:3}),
    Object.assign({},base,{key:"fixture/c2",id:"c2",title:"Fix",relationshipToParent:"subagent",parentThreadId:"parent",status:"completed",updatedAt:2})],projects:[],environments:[]}
   b.state=fixture;b.catalogs={fixture:{canWrite:true,models:[],providers:[],favorites:[]}};b.selectThread("fixture","c1");panel.open();phase=1
  }else if(phase===1){
   check(b.subthread&&b.parentThread&&b.parentThread.id==="parent","a sub-thread knows its parent")
   check(b.siblingThreads.length===2&&b.siblingIndex===0,"siblings in sidebar order")
   var bar=keys.findChild(panel,"t3SubthreadBar");check(bar&&bar.visible,"sub-thread bar shows")
   var rows=panel.activeRows;check(rows.length===3&&rows[0].id==="parent"&&rows[0].subthreads.count===2&&rows[1].depth===1,"children nest under the parent in the sidebar")
   check(panel.topCount(rows)===1,"shelf counts ignore nested children")
   b.openSibling(1);phase=2
  }else if(phase===2){
   check(b.selectedThread==="c2"&&b.siblingIndex===1,"next subagent opens the sibling")
   b.openSibling(1);check(b.selectedThread==="c1","navigation wraps around")
   b.openParentThread();phase=3
  }else if(phase===3){
   check(b.selectedThread==="parent"&&!b.subthread&&b.childThreads.length===2,"parent lists its sub-threads")
   b.threadSubagents=[{id:"s1",title:"Review",status:"running",childThreadId:"c1",startedAt:Date.now()-65000,progress:"Reading files"},{id:"s2",title:"Fix",status:"failed",childThreadId:"c2",result:"Tests failed"}]
   check(b.activeSubagents.length===1,"only live subagents reach the composer bar")
   check(cards.group&&cards.subagents[0].status==="running"&&cards.subagents[0].title==="Review","conversation cards follow live state")
   check(cards.summary().indexOf("1")>=0,"group summary counts states")
   panel.openSide("agents");phase=4
  }else if(phase===4){
   var agents=keys.findChild(panel,"t3AgentsPanel");if(!agents)return
   check(agents.running.length===1&&agents.previous.length===1&&agents.previousFailed===1,"agents panel splits running and previous agents")
   check(agents.all.length===2,"sub-threads already listed by the projection are not repeated")
   var parentRow=panel.activeRows[0];panel.toggleSubthreads(parentRow);phase=5
  }else if(phase===5){
   check(b.viewState.subthreadsExpanded["fixture/parent"]===false&&panel.activeRows.length===1,"collapsing a parent hides and remembers")
   panel.toggleSubthreads(panel.activeRows[0]);check(panel.activeRows.length===3,"expanding shows children again")
   check(runError.heading===Test.T3I18n.tr("Usage limit reached"),"usage limit failures are named")
   var shown=Test.T3I18n.error("rpc_rejected: Project p-1 was not found.");check(shown.indexOf("Project p-1 was not found.")>=0&&shown.indexOf("rpc_rejected")<0,"server reasons are shown instead of the bare code: "+shown)
   check(Test.T3I18n.error("invalid_project").indexOf("invalid_project")>=0,"plain codes still read as before")
   panel.close();console.log("T3_SUBAGENT_LINEAGE_QML_PASS: nested sub-threads, sibling and parent navigation, live cards, agents panel, error reasons");Qt.quit()
  }
 }}
}
