import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test

ShellRoot {
  id: root
  property int phase: 0
  property var fixture: null
  function check(value,message){if(!value){console.error("NATIVE_SIDEBAR_QML_FAIL "+message);Qt.exit(1)}}
  function thread(id,title,settled){
    var stamp="2026-11-08T00:00:00.000Z"
    return {id:id,key:"fixture/"+id,environmentId:"fixture",environmentName:"Private",title:title,
      project:"Project",projectId:"project",projectKey:"fixture/project",settled:settled===true,
      searchShell:{id:id,environmentId:"fixture",title:title,branch:null,createdAt:stamp,
        updatedAt:stamp,latestUserAuthoredMessageAt:stamp,opens:null}}
  }
  function match(id,hits){return {threadId:id,environmentId:"fixture",source:"assistant",snippet:"needle in the actual message",hits:hits,messageId:"entry-"+id}}
  Test.T3Panel{id:panel;bridge:Test.T3Backend}
  TestCase{id:keys;when:false}
  Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
  Timer {
    interval:80;repeat:true;running:true
    onTriggered:{
      var backend=Test.T3Backend,results=panel.sidebarSearchResults
      if(root.phase===0){
        if(!backend.viewStateLoaded)return
        backend.rememberView({windowMode:"floating",windowWidth:1250,windowHeight:850,interfaceLayout:"projects",animationsEnabled:false})
        root.fixture={threads:[root.thread("one","Needle title",false),root.thread("two","Other task",true),root.thread("three","Late task",false)],
          projects:[{id:"project",key:"fixture/project",environmentId:"fixture",title:"Project",workspaceRoot:"/tmp/private-project"}],
          environments:[{id:"fixture",name:"Private",state:"ready"}]}
        backend.state=root.fixture;panel.open();backend.search("needle");root.phase=1
      }else if(root.phase===1){
        if(panel.sidebarRankBusy||results.length!==1||!backend.sidebarSearch.requestId)return
        check(results[0].key==="fixture/one","native title result")
        check(panel.sidebarView.objectName==="t3ThreadSearchResults","project mode switches to one flat list")
        backend.action({op:"thread-search",requestId:backend.sidebarSearch.requestId,
          result:{query:"needle",matches:[root.match("two",1)],pending:1}})
        root.phase=2
      }else if(root.phase===2){
        if(panel.sidebarRankBusy||results.length!==2)return
        check(results.map(function(entry){return entry.key}).join(",")==="fixture/one,fixture/two","title before message-only settled row")
        check(backend.sidebarSearch.busy,"fast partial answer remains pending")
        panel.sidebarSearchHighlightKey="fixture/two"
        backend.action({op:"thread-search",requestId:backend.sidebarSearch.requestId,
          result:{query:"needle",matches:[root.match("three",100),root.match("two",1)],pending:0}})
        root.phase=3
      }else if(root.phase===3){
        if(panel.sidebarRankBusy||results.length!==3)return
        check(results.map(function(entry){return entry.key}).join(",")==="fixture/one,fixture/two,fixture/three","late message keeps shown order")
        check(panel.sidebarSearchActiveIndex===1&&results[1].item.find.start.entryId==="entry-two","highlight follows its key and native find target")
        panel.searchInput.forceActiveFocus();keys.keyClick(Qt.Key_Return)
        root.phase=4
      }else if(root.phase===4){
        check(backend.selectedKey==="fixture/two"&&backend.query==="","Enter selects and clears search")
        check(panel.findOpen&&panel.findQuery==="needle","message-only selection opens native find")
        panel.findOpen=false;backend.search("ab");root.phase=5
      }else if(root.phase===5){
        if(!backend.sidebarSearch.planReady)return
        check(!backend.sidebarSearch.plan.canSearch&&!backend.sidebarSearch.requestId,"two-letter Latin query skips message RPC")
        backend.search("日本");root.phase=6
      }else if(root.phase===6){
        if(!backend.sidebarSearch.planReady||!backend.sidebarSearch.requestId)return
        check(backend.sidebarSearch.plan.searchMessages,"two CJK characters use native message admission")
        backend.search("  "+"a".repeat(220)+"  ");root.phase=7
      }else if(root.phase===7){
        if(!backend.sidebarSearch.planReady)return
        check(backend.sidebarSearch.query.length===200,"pasted query truncates to native UTF16 limit")
        backend.search("");panel.close()
        console.log("NATIVE_SIDEBAR_QML_PASS: original V8 rank, flat project view, partial and late results, keyed Enter, matched-message find, native CJK admission and query limit")
        Qt.quit()
      }
    }
  }
}
