import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property bool clickReady:false
 function check(ok,msg){if(!ok){console.error("T3_SUBAGENT_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   var base={environmentId:"fixture",projectId:"project",projectKey:"fixture/project",modelSelection:{instanceId:"fixture",model:"model"},runtimeMode:"approval-required"}
   fixture={threads:[Object.assign({},base,{key:"fixture/native",id:"native",title:"Native",providerSubagent:true,parentThreadId:"parent"}),Object.assign({},base,{key:"fixture/parent",id:"parent",title:"Parent"}),Object.assign({},base,{key:"fixture/new",id:"new",title:"New"}),Object.assign({},base,{key:"fixture/mcp",id:"mcp",title:"MCP",relationshipToParent:"subagent",parentThreadId:"parent"})],projects:[],environments:[]}
   b.state=fixture;b.catalogs={fixture:{canWrite:true,models:[],providers:[],favorites:[]}};b.selectThread("fixture","native");panel.open();phase=1
  }else if(phase===1){
   if(b.draft.loading)return
   b.edit("Preserve my failed prompt");b.patchDraft(b.selectedKey,{error:"rpc_rejected"});b.submit()
   check(b.providerSubagent&&!b.draft.busy&&!b.draft.attempt,"native direct send blocked before draft/outbox mutation")
   check(!panel.composerView.visible,"native composer hidden")
   var parent=keys.findChild(panel,"t3OpenParentThread"),recover=keys.findChild(panel,"t3RecoverSubagentDraft")
   check(parent&&parent.visible&&parent.enabled&&recover.visible&&recover.enabled,"parent and recoverable draft actions visible")
   if(!clickReady){clickReady=true;return};keys.mouseClick(parent,parent.width/2,parent.height/2);phase=2
  }else if(phase===2){
   check(b.selectedThread==="parent"&&!b.providerSubagent&&panel.composerView.visible,"parent button opens writable parent in same environment: "+b.selectedThread+" / "+b.providerSubagent+" / "+panel.composerView.visible)
   b.selectThread("fixture","native");phase=3
  }else if(phase===3){
   check(b.draft.text==="Preserve my failed prompt"&&b.draft.error==="rpc_rejected","navigation preserves failed draft")
   b.managementRequest={op:"manage",action:"new-thread",environment:"fixture",projectId:"project",requestId:"fixture-creation"};b.managementBusy=true
   b.action({op:"manage",environment:"fixture",requestId:"fixture-creation",result:{action:"new-thread",threadId:"new",projectId:"project"}});phase=4
  }else if(phase===4){
   check(b.selectedThread==="new"&&!b.providerSubagent&&panel.composerView.visible,"new thread acknowledgement selects writable destination")
   if(b.draft.loading)return
   b.edit("New chat prompt");b.submit();check(b.draft.attempt&&b.draft.attempt.thread==="new","new chat sends to destination")
   b.selectThread("fixture","mcp");phase=5
  }else if(phase===5){
   if(b.draft.loading)return
   check(!b.providerSubagent&&panel.composerView.visible,"MCP delegated children stay writable")
   panel.close();console.log("T3_SUBAGENT_QML_PASS: native ownership guard, parent navigation, draft preservation and writable new/MCP chats");Qt.quit()
  }
 }}
}
