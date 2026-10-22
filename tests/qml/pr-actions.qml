import QtQuick
import QtQuick.Window 6.11
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 function check(ok,msg){if(!ok){console.error("PR_ACTIONS_QML_FAIL "+msg);Qt.exit(1)}}
 QtObject{
  id:fixture
  signal pullRequestWritten(var data)
  property string selectedEnvironment:"fixture"
  property string selectedThread:"t1"
  property var selectedInfo:({pullRequestBadge:{links:[]}})
  property var state:({environments:[{id:"fixture",name:"Local",state:"ready"}]})
  property var prOperations:({})
  property var writes:[]
  property var links:[]
  function resetPullRequests(){prOperations=({})}
  function requestPullRequests(action,values){var all=Object.assign({},prOperations);var data=action==="list"?{entries:[{projectId:"p",host:"github.com",repository:"o/r",number:7,title:"Add parser",state:"open",updatedAt:"2026-10-01T00:00:00Z"}]}:action==="detail"?{projectId:"p",repository:"o/r",number:7,title:"Add parser",body:"",url:"https://github.com/o/r/pull/7",state:"open",isDraft:false,mergeability:"mergeable",headBranch:"feat",baseBranch:"main",additions:1,deletions:0,mergeCapabilities:{merge:true,squash:true,rebase:false},viewerPermissions:{actions:["merge","close","draft"],comment:true,verdicts:["approve","request-changes"],requestReviewers:true,labels:true}}:{comments:[]};all[action]={busy:false,error:"",data:data};prOperations=all}
  function pullRequestWrite(environment,values){writes=writes.concat([values])}
  function threadPullRequest(link,reference){links=links.concat([{link:link,reference:reference}])}
 }
 TestCase{id:keys;when:false}
 Window{id:win;visible:true;width:1100;height:700;color:"black"
  Test.T3PullRequests{id:view;anchors.fill:parent;bridge:fixture}
 }
 Timer{interval:250;running:true;repeat:true;onTriggered:{
  if(phase===0){view.openEntry(view.entries[0]);phase=1
  }else if(phase===1){check(view.allowed.join()==="merge,close,draft","allowed actions follow permissions and state: "+view.allowed.join());check(view.mergeMethods.join()==="squash,merge","merge methods follow capabilities")
   var merge=keys.findChild(view,"pullRequestAction_merge");merge.clicked();check(fixture.writes.length===0&&view.confirming==="merge","merge asks to confirm");merge.clicked();check(fixture.writes.length===1&&fixture.writes[0].action==="merge"&&fixture.writes[0].mergeMethod==="squash","confirmed merge runs with the chosen method")
   keys.findChild(view,"pullRequestLink").clicked();check(fixture.links[0].link==="link"&&fixture.links[0].reference.host==="github.com"&&fixture.links[0].reference.number===7,"link to thread uses the PR key");view.loadTab(2);phase=2
  }else if(phase===2){var reply=keys.findChild(view,"pullRequestReply");check(reply.visible,"reply box on conversation tab");check(!keys.findChild(view,"pullRequestComment").enabled,"comment needs text");keys.findChild(view,"pullRequestReplyText").text="Looks good";keys.findChild(view,"pullRequestApprove").clicked();check(fixture.writes[1].kind==="review"&&fixture.writes[1].verdict==="approve"&&fixture.writes[1].body==="Looks good","approve submits a review")
   view.loadTab(0);keys.findChild(view,"pullRequestReviewers").clicked();check(fixture.writes[2].kind==="reviewer-candidates","reviewers load candidates");fixture.pullRequestWritten({result:{kind:"reviewer-candidates",result:{candidates:[{id:"u1",kind:"user",login:"ana",name:null,isRequested:false}]}}});check(keys.findChild(view,"pullRequestPeople").candidates.length===1,"candidates shown")
   console.log("PR_ACTIONS_QML_PASS: permitted actions, merge confirmation and method, link to thread, review and comment");Qt.quit()}
 }}
}
