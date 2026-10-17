import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test

ShellRoot {
 id: root
 property var scope: ({environment:"remote",thread:"t",projectId:"p",cwd:"/remote/tree",requestId:"r1"})
 property var sample: ({scope:{environment:"remote",thread:"t",projectId:"p",cwd:"/remote/tree",requestId:"r1"},revision:"fixture-revision",status:{isRepo:true,refName:"main",isDefaultRef:true,hasWorkingTreeChanges:true,aheadCount:1,behindCount:0,pr:null,workingTree:{files:[{path:"file <b>plain</b>.rs",insertions:2,deletions:1}]}},scripts:[{id:"test",name:"Test",command:"npm test",runOnWorktreeCreate:false}],actions:{commit:true,push:true,pull:false,commit_push:true,create_pr:true,commit_push_pr:true}})
 property var view: ({loading:false,error:"",data:sample})
 property int requests: 0
 property var lastIntent: null
 property int phase: 0
 function check(ok, message){if(!ok){console.error("GIT_CONTROLS_QML_FAIL "+message);Qt.exit(1)}}
 TestCase{id:helpers;when:false}
 Test.T3GitControls {
  id:panel;width:650;height:740;scope:root.scope;view:root.view
  canSourceControlWrite:true;canTerminalWrite:true
  onActionRequested:function(intent){root.requests++;root.lastIntent=intent}
 }
 Timer {
  interval:180;running:true;repeat:true
  onTriggered:{
   if(phase===0){
    check(panel.ready&&panel.files.length===1&&panel.scripts.length===1,"native read presentation")
    check(helpers.findChild(panel,"t3GitBranch").text==="main","branch label")
    check(!panel.request("pull",""),"unsupported pull disabled")
    check(panel.request("push",""),"open push confirmation")
    check(panel.needsDefaultConfirmation&&!panel.confirm()&&requests===0,"default branch guard")
    panel.defaultBranchConfirmed=true;check(panel.confirm()&&requests===1,"confirmed push intent")
    check(lastIntent.operation==="push"&&lastIntent.defaultBranchConfirmed,"default acknowledgement preserved")
    check(panel.request("commit",""),"commit confirmation")
    panel.toggleFile("file <b>plain</b>.rs");panel.commitMessage="Keep message"
    check(panel.confirm()&&lastIntent.filePaths.length===1&&lastIntent.commitMessage==="Keep message","selected files and message")
    panel.request("script","test");check(panel.pending.script.command==="npm test","script command preview")
    panel.confirm();check(lastIntent.operation==="script"&&lastIntent.scriptId==="test"&&!lastIntent.command,"script identity not command injection")
    panel.operationState="unknown";check(!panel.request("script","test"),"unknown outcome blocks replay")
    panel.operationState="";panel.request("commit","")
    root.scope={environment:"remote",thread:"other",projectId:"p",cwd:"/remote/tree",requestId:"r2"};phase=1
   }else if(phase===1){
    check(!panel.ready&&panel.pending===null&&!panel.confirm(),"late cross-thread result rejected")
    root.view={loading:true,error:"",data:sample};phase=2
   }else if(phase===2){
    check(!panel.ready&&!panel.allowed("commit"),"loading state blocked")
    root.view={loading:false,error:"unauthorized",data:null};phase=3
   }else if(phase===3){
    check(!panel.ready&&!panel.allowed("script"),"error state blocked")
    var empty={scope:root.scope,revision:"empty",status:{isRepo:false,refName:null,isDefaultRef:false,hasWorkingTreeChanges:false,aheadCount:0,behindCount:0,pr:null,workingTree:{files:[]}},scripts:[],actions:{}}
    root.view={loading:false,error:"",data:empty};phase=4
   }else{
    check(panel.ready&&panel.files.length===0&&panel.scripts.length===0&&!panel.allowed("commit"),"empty non-repo state")
    panel.canTerminalWrite=false;check(!panel.allowed("script"),"terminal capability gate")
    console.log("GIT_CONTROLS_QML_PASS: scoped reads, actions, default confirmation, scripts, unknown, loading/error/empty")
    Qt.quit()
   }
  }
 }
}
