import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/SidebarModel.js" as Sidebar
ShellRoot {
  id: root
  function check(ok,msg) { if (!ok) { console.error("T3_SIDEBAR_QML_FAIL: " + msg); Qt.exit(1) } }
  Test.T3Panel { id: panel; bridge: Test.T3Backend }
  Test.QuotaStrip { id: quota; width: 300; providers: [] }
  Timer {
    interval: 700; running: true
    onTriggered: {
      root.check(!panel.sidebarCollapsed,"sidebar starts open")
      panel.toggleSidebar(); root.check(panel.sidebarCollapsed,"sidebar collapses")
      panel.toggleSidebar(); root.check(!panel.sidebarCollapsed,"sidebar reopens")
      quota.providers=[{provider:"codex",label:"Codex",weekly:{availablePercent:0,windowSeconds:604800},shortWindow:null,onPace:true},{provider:"empty",label:"Empty",weekly:null,shortWindow:null}]
      root.check(quota.visibleProviders.length===1,"providers without data are hidden, zero remains valid")
      root.check(quota.windows(quota.visibleProviders[0]).length===1,"no phantom 5h window")
      quota.providers=[]; root.check(!quota.visible,"empty quota section is hidden")
      var projects=[{id:"p1",key:"fixture/p1",title:"One"},{id:"p2",key:"fixture/p2",title:"Two"},{id:"p3",key:"fixture/p3",title:"Three"}]
      var threads=[{id:"a",key:"fixture/a",projectKey:"fixture/p1",title:"Active",status:"running",working:true},
        {id:"b",key:"fixture/b",projectKey:"fixture/p2",title:"Done",status:"completed",pending:true,reason:"review"},
        {id:"c",key:"fixture/c",projectKey:"fixture/p2",title:"Error",status:"failed"},
        {id:"d",key:"fixture/d",projectKey:"fixture/p3",title:"Parked",status:"completed",settled:true},
        {id:"e",key:"fixture/e",projectKey:"fixture/p1",title:"Approval",status:"running",working:true,pending:true,reason:"request"}]
      Test.T3Backend.state={projects:projects,threads:threads,environments:[],working:2,pending:2}
      root.check(Sidebar.state(threads[4])==="waiting","attention takes priority")
      root.check(Sidebar.count(threads,"done","")===2,"done and settled remain distinct")
      root.check(Sidebar.rows(threads,"all","",false,"").length===5,"settled collapsed")
      root.check(Sidebar.rows(threads,"all","",true,"").length===6,"settled expanded")
      root.check(Sidebar.rows(threads,"settled","",false,"").length===2,"settled filter expands shelf")
      root.check(Sidebar.rows(threads,"all","",false,"parked")[1].id==="d","search reveals settled match")
      root.check(Sidebar.rows(threads,"failed","fixture/p1",false,"").length===0,"project and state intersection")
      root.check(Sidebar.rows(threads,"all","",false,"error")[0].id==="c","search and section filtering")
      // Smart search: fields, word starts, initials, prefixes, phrases, exclusions and typos.
      var lib=[{key:"e/a",id:"a",title:"Fix dictation long clips",project:"omarchy-t3-code"},
        {key:"e/b",id:"b",title:"Sidebar search refinements",project:"omabridge"},
        {key:"e/c",id:"c",title:"Release notes",project:"omarchy-t3-code"},
        {key:"e/d",id:"d",title:"Audit the omabridge transport",project:"omarchy-t3-code",branch:"feat/live-wire"},
        {key:"e/p",id:"p",title:"Pinned chores",project:"misc",pinned:true}]
      var ctx={groups:{"e/c":"Launch"},hosts:{}}
      function found(q){var r=Sidebar.rank(lib,q,ctx);return Object.keys(r).sort(function(x,y){return r[y]-r[x]||x.localeCompare(y)}).map(function(k){return k.slice(2)}).join()}
      root.check(found("sid")==="b","a word start finds the title: "+found("sid"))
      root.check(found("otc")==="a,c,d","initials find the project: "+found("otc"))
      root.check(found("p:omabridge")==="b","p: looks only at the project: "+found("p:omabridge"))
      root.check(found("omabridge").split(",").sort().join()==="b,d","plain words look everywhere: "+found("omabridge"))
      root.check(found("g:launch")==="c"&&found("launch")==="c","groups are searchable: "+found("launch"))
      root.check(found("b:live")==="d","b: looks at the branch: "+found("b:live"))
      root.check(found("re").indexOf("c")===0,"a title that starts with the word ranks first: "+found("re"))
      root.check(found("dictaiton")==="a","typos are forgiven when nothing matches exactly: "+found("dictaiton"))
      root.check(found("dic")==="a"&&found("dicta -long")==="","a leading minus leaves a thread out: "+found("dicta -long"))
      root.check(found("\"long clips\"")==="a"&&found("\"clips long\"")==="","quoted words stay together")
      root.check(found("is:pin")==="p","is: filters by state: "+found("is:pin"))
      root.check(found("ACCIÓN")===""&&found("réléase")==="c","accents and case never matter")
      root.check(Sidebar.freeText('p:x "a b" -c d')==='"a b" d',"only free words go to the server: "+Sidebar.freeText('p:x "a b" -c d'))
      var family=[{id:"parent",title:"Parent"},{id:"child",title:"Child",parentThreadId:"parent",relationshipToParent:"subagent",working:true},{id:"fork",title:"Fork",parentThreadId:"parent",relationshipToParent:"fork"}]
      root.check(Sidebar.rows(family,"all","",false,"").length===2,"subagents stay outside the official sidebar roster")
      root.check(Sidebar.rows(family,"all","",false,"")[1].id==="fork","forks remain visible")
      root.check(Sidebar.count(family,"working","")===0,"filter counts follow visible roster")
      var tree=[{id:"parent",key:"e/parent",environmentId:"e",title:"Parent"},{id:"child",key:"e/child",environmentId:"e",title:"Child",parentThreadId:"parent",relationshipToParent:"subagent",working:true,updatedAt:2},{id:"older",key:"e/older",environmentId:"e",title:"Older",parentThreadId:"parent",relationshipToParent:"subagent",status:"completed",updatedAt:1},{id:"grand",key:"e/grand",environmentId:"e",title:"Grand",parentThreadId:"older",relationshipToParent:"subagent",status:"completed"},{id:"orphan",key:"e/orphan",environmentId:"e",title:"Orphan",parentThreadId:"gone",relationshipToParent:"subagent"}]
      var nested=Sidebar.nest(Sidebar.rows(tree,"all","",false,""),tree,{},"")
      root.check(nested.length===3&&nested[0].subthreads.count===2&&nested[0].subthreads.working===1&&nested[0].subthreadsExpanded,"a working child opens its parent")
      root.check(nested[1].id==="child"&&nested[1].depth===1&&nested[2].id==="older"&&nested[2].subthreads.count===1&&!nested[2].subthreadsExpanded,"children follow the parent, newest first, grandchildren folded")
      root.check(Sidebar.nest(Sidebar.rows(tree,"all","",false,""),tree,{"e/parent":false},"").length===1,"a collapsed parent hides its children")
      var deep=Sidebar.nest(Sidebar.rows(tree,"all","",false,""),tree,{},"e/grand")
      root.check(deep.length===4&&deep[3].id==="grand"&&deep[3].depth===2,"the selected grandchild stays visible")
      root.check(!nested.some(function(r){return r.id==="orphan"}),"children without a listed parent stay out of the roster")
      root.check(Sidebar.siblings(tree,tree[1]).length===2&&Sidebar.siblings(tree,tree[0]).length===0,"siblings share a parent")
      Test.T3Palette.load('blue="#123456"\ngreen="#459900"\nred="#ff3344"\nmagenta="#7f4488"')
      root.check(String(Test.T3Palette.working)==="#123456","working palette role")
      root.check(String(Test.T3Palette.done)==="#459900","done palette role")
      var colors=projects.map(function(p){return Test.T3Palette.projectColor(p.key,projects)})
      root.check(colors[0]!==colors[1] && colors[1]!==colors[2] && colors[0]!==colors[2],"project collision avoidance")
      var old=String(Test.T3Palette.working)
      Test.T3Palette.load('blue="#336699"\ngreen="#77bb22"')
      root.check(String(Test.T3Palette.working)!==String(old),"live theme update")
      panel.statusFilter="failed"
      root.check(panel.sidebarRows.length===1 && panel.sidebarRows[0].id==="c","panel state filter binding")
      Test.T3Backend.activity=[{type:"command_execution",title:""},{type:"dynamic_tool",title:"Search"}]
      root.check(Test.T3Backend.activityLabel(false).indexOf("Ejecutando comando")>=0,"real activity labels")
      root.check(Test.T3Backend.activityLabel(false).indexOf("Llamando herramienta · Search")>=0,"native dynamic tool labels")
      panel.statusFilter="all"
      Test.T3Backend.state={projects:projects,threads:threads.concat([{id:"sleep",key:"fixture/sleep",environmentId:"fixture",title:"Snoozed",snoozed:true}]),environments:[],working:2,pending:2}
      root.check(panel.snoozedRows.length===1 && panel.activeRows.length===3,"snoozed shelf is separate from active and settled")
      root.check(panel.inputRows.length===1 && panel.inputRows[0].id==="e" && !panel.activeRows.some(function(t){return t.id==="e"}),"threads waiting for input have their own group")
      Qt.callLater(function(){ console.log("T3_SIDEBAR_QML_PASS: shelf, filters, palette, projects and activity");Qt.quit() })
    }
  }
}
