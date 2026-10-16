import QtQuick
import Quickshell
import "UnderTest" as Test
import "UnderTest/SidebarModel.js" as Sidebar
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
      root.check(!quota.hasWindow(quota.visibleProviders[0].shortWindow),"no phantom 5h window")
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
      var family=[{id:"parent",title:"Parent"},{id:"child",title:"Child",parentThreadId:"parent",relationshipToParent:"subagent",working:true},{id:"fork",title:"Fork",parentThreadId:"parent",relationshipToParent:"fork"}]
      root.check(Sidebar.rows(family,"all","",false,"").length===2,"subagents stay outside the official sidebar roster")
      root.check(Sidebar.rows(family,"all","",false,"")[1].id==="fork","forks remain visible")
      root.check(Sidebar.count(family,"working","")===0,"filter counts follow visible roster")
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
      root.check(panel.snoozedRows.length===1 && panel.activeRows.length===4,"snoozed shelf is separate from active and settled")
      Qt.callLater(function(){ console.log("T3_SIDEBAR_QML_PASS: shelf, filters, palette, projects and activity");Qt.quit() })
    }
  }
}
