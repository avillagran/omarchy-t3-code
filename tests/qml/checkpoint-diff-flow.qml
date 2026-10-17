import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 property var summary:({fromTurnCount:2,toTurnCount:3,files:[{path:"a.js"},{path:"b.js"}]})
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 function check(ok,message){if(!ok){console.error("CHECKPOINT_DIFF_FLOW_QML_FAIL "+message);Qt.exit(1)}}
 function reply(token){var b=Test.T3Backend,v=b.diffView;b.action({op:"diff",environment:v.environment,thread:v.thread,requestId:v.requestId,result:{fromTurnCount:2,toTurnCount:3,files:[{path:"a.js",oldPath:"a.js",reviewSnapshotId:token,rows:[],splitRows:[]},{path:"b.js",oldPath:"b.js",reviewSnapshotId:token,rows:[],splitRows:[]},{path:"image.png",oldPath:"image.png",binary:true,rows:[],splitRows:[]}],additions:2,deletions:1}})}
 Connections {
  target:Test.T3Backend
  function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}
  function onInlineDocumentsChanged(){
   var b=Test.T3Backend,all=Object.assign({},b.inlineDocuments),changed=false
   Object.keys(all).forEach(function(id){var d=all[id];if(d.error==="review_snapshot_expired")return;if(d.rendererReady&&!d.error&&d.data&&d.data.html&&d.frameUrl)return;all[id]=Object.assign({},d,{busy:false,error:"",rendererReady:true,data:{html:true,internalReview:true,title:"Historical patch",contents:""},frameUrl:Qt.resolvedUrl("UnderTest/tests/fixtures/image.png").toString(),frameWidth:640,frameHeight:400});changed=true})
   if(changed)b.inlineDocuments=all
  }
 }
 Timer{interval:180;repeat:true;running:true;onTriggered:{var b=Test.T3Backend,popup=keys.findChild(panel,"t3DiffPopup"),review=keys.findChild(panel,"t3DiffContents")
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"Historical",projectId:"p"}],projects:[{id:"p",key:"fixture/p",environmentId:"fixture",workspaceRoot:"/unused/current-checkout"}],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1}
  else if(phase===1){if(b.draft.loading||panel.restoringConversation)return;b.edit("Historical review keeps this draft");popup.initialPath="a.js";b.requestDiff(root.summary);popup.open();reply("range-one");phase=2}
  else if(phase===2){check(review.nativeRenderer&&review.richSessionId&&review.richPath==="a.js","checkpoint popup opens same native renderer with a range snapshot");review.selectFile("b.js");phase=3}
  else if(phase===3){check(review.richPath==="b.js"&&review.richSnapshotId==="range-one"&&Object.keys(b.inlineDocuments).length===1,"same range supports another selected file and releases previous frame");keys.findChild(review,"t3DiffWhitespace").clicked();check(popup.initialPath==="b.js"&&!b.diffView.ignoreWhitespace,"whitespace refetch captures selected historical file");reply("range-two");phase=4}
  else if(phase===4){check(review.selectedPath==="b.js"&&review.richSnapshotId==="range-two","refetch keeps selected file in refreshed range");b.updateInlineDocument(review.richSessionId,{error:"review_snapshot_expired"});phase=5}
  else if(phase===5){check(popup.initialPath==="b.js","expired range reload captures current file");b.requestDiff(root.summary,false);reply("range-three");phase=6}
  else if(phase===6){check(review.richPath==="b.js"&&review.richSnapshotId==="range-three","fresh range reopens original selected historical file");review.selectFile("image.png");phase=7}
  else if(phase===7){check(review.file.binary&&!review.nativeRenderer&&!Object.keys(b.inlineDocuments).length,"binary metadata does not create a false rich renderer");var old=b.diffView;popup.close();b.action({op:"diff",environment:old.environment,thread:old.thread,requestId:old.requestId,result:old.data});check(!b.diffView.data&&!b.diffView.requestId,"closed historical range rejects late reads");check(b.draft.text==="Historical review keeps this draft"&&!b.draft.attempt,"no composer consumption or dispatch");panel.close();console.log("CHECKPOINT_DIFF_FLOW_QML_PASS: historical popup/native snapshot, file selection within one range, whitespace and expiry retain selected file, binary fallback, late replies, cleanup and draft preservation");Qt.quit()}
 }}
 Timer{interval:9500;running:true;onTriggered:{console.error("CHECKPOINT_DIFF_FLOW_QML_FAIL timeout phase="+root.phase);Qt.exit(1)}}
}
