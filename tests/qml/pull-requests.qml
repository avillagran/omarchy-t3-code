import QtQuick
import QtQuick.Window 6.11
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property bool done:false
 function check(ok,msg){if(!ok){console.error("T3_PULL_REQUESTS_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 QtObject{
  id:fixture
  property string selectedEnvironment:"fixture"
  property var selectedInfo:({pullRequestBadge:{links:[]}})
  signal pullRequestWritten(var data)
  property var state:({environments:[{id:"fixture",name:"Local",state:"ready"}]})
  property var prOperations:({})
  property var calls:[]
  function resetPullRequests(){prOperations=({})}
  function requestPullRequests(action,values){calls=calls.concat([{action:action,values:values}]);var all=Object.assign({},prOperations);var data=action==="list"?{entries:[{projectId:"p",host:"github.com",repository:"test/project",number:42,title:"Native pull request",url:"https://github.com/test/project/pull/42",state:"open",author:{login:"fixture"},headBranch:"feature",baseBranch:"main",updatedAt:"2026-10-16T00:00:00Z",checksState:"passing"}],errors:[],nextCursors:{}}:action==="detail"?{title:"Native pull request",body:"**Description**\n\nMatches the original view.",headBranch:"feature",baseBranch:"main",additions:12,deletions:3}:action==="checks"?{checks:[{name:"Rust tests",status:"success",description:"All passed"}]}:action==="activity"?{comments:[{author:{login:"reviewer"},body:"Looks good"}]}:{patch:"diff --git a/app.rs b/app.rs\n--- a/app.rs\n+++ b/app.rs\n@@ -1 +1 @@\n-old\n+new\n",nextCursor:null};all[action]={busy:false,data:data,error:""};prOperations=all}
 }
 Timer{interval:400;running:true;onTriggered:checks.runChecks()}
 Timer{interval:150;running:root.done;onTriggered:Qt.quit()}
 Window{
  id:win;visible:true;width:1000;height:700;color:Color.popups.background
  Rectangle{anchors.fill:parent;color:Color.popups.background}
  Test.T3PullRequests{id:view;anchors.fill:parent;bridge:fixture}
  TestCase{id:checks;when:false
   function runChecks(){win.requestActivate();wait(100);root.check(view.entries.length===1,"native list renders")
    var list=findChild(view,"pullRequestList");list.forceLayout();var row=list.itemAtIndex(0);mouseClick(row,25,row.height/2);wait(100)
    root.check(view.selected.number===42&&view.detail.body.indexOf("Description")>=0,"row opens native detail")
    view.loadTab(1);root.check(view.tabResult.data.checks[0].status==="success","checks data")
    view.loadTab(2);root.check(view.tabResult.data.comments[0].author.login==="reviewer","conversation data")
    view.loadTab(3);wait(80);root.check(view.patch.indexOf("+new")>=0,"native HTTP patch data")
    view.grabToImage(function(r){r.saveToFile("/tmp/t3-pull-requests.png")});wait(150)
    root.check(fixture.calls.every(function(c){return ["list","detail","checks","activity","diff"].indexOf(c.action)>=0}),"view emits only PR reads")
    win.visible=false;root.done=true;console.log("T3_PULL_REQUESTS_QML_PASS: list, detail, checks, conversation, diff and native read-only actions")
   }
  }
 }
}
