import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0
 TestCase{id:keys;when:false}
 function check(ok,message){if(!ok){console.error("NATIVE_DIFF_FLOW_QML_FAIL "+message);Qt.exit(1)}}
 QtObject {
  id:bridge;property var inlineDocuments:({});property int opened:0;property var openedPaths:[];property var closedIds:[];property var commands:[]
  function openReviewDocument(snapshot,layout,wrap,path){openedPaths=openedPaths.concat([path]);opened++;var id="review-"+opened,all=Object.assign({},inlineDocuments);all[id]={busy:false,error:"",rendererReady:true,data:{html:true,internalReview:true,title:snapshot},frameUrl:Qt.resolvedUrl("UnderTest/tests/fixtures/image.png").toString(),frameWidth:640,frameHeight:400};inlineDocuments=all;return id}
  function closeInlineDocument(id){closedIds=closedIds.concat([id]);var all=Object.assign({},inlineDocuments);delete all[id];inlineDocuments=all}
  function send(command){commands=commands.concat([command])}
 }
 Test.T3DiffReview {
  id:review;rendererBridge:bridge;width:900;height:620;summary:({});treeOpen:true
  view:({loading:false,error:"",data:{files:[{path:"a.js",oldPath:"a.js",reviewSnapshotId:"opaque-range",rows:[],splitRows:[]},{path:"b.js",oldPath:"b.js",reviewSnapshotId:"opaque-range",rows:[],splitRows:[]},{path:"image.png",oldPath:"image.png",binary:true,rows:[],splitRows:[]}],additions:1,deletions:1}})
 }
 Timer{interval:160;running:true;repeat:true;onTriggered:{
  if(phase===0){if(!review.richSessionId)return;check(review.nativeRenderer&&bridge.opened===1,"native snapshot opens one bounded review session");var doc=keys.findChild(review,"t3DocumentView");check(doc&&!doc.chromeVisible&&doc.ownsWheel&&doc.inlineMode,"shared HTML surface embeds without duplicate chrome and owns wheel");review.diffLayout="split";review.wordWrap=false;phase=1}
  else if(phase===1){check(bridge.commands.some(function(c){return c.event&&c.event.type==="review-options"&&c.event.layout==="split"&&c.event.wrap===false}),"layout/wrap update actual renderer without reopening or refetching");check(bridge.opened===1,"option updates preserve the native instance");review.moveFile(1);phase=2}
  else if(phase===2){check(bridge.opened===2&&bridge.closedIds.indexOf("review-1")>=0&&review.richSnapshotId==="opaque-range"&&review.richPath==="b.js"&&bridge.openedPaths[1]==="b.js","file navigation cancels previous renderer");review.width=490;phase=3}
  else if(phase===3){check(!review.split&&bridge.commands.some(function(c){return c.requestId==="review-2"&&c.event&&c.event.type==="review-options"&&c.event.layout==="stacked"}),"narrow layout switches native renderer to unified");var doc=keys.findChild(review,"t3DocumentView"),e={key:Qt.Key_Left,modifiers:Qt.ControlModifier,accepted:false};check(doc.keyFilter(e)&&e.accepted,"native frame retains file navigation shortcut");phase=4}
  else if(phase===4){check(review.selectedPath==="a.js","shortcut moves to original file");review.selectFile("image.png");phase=5}
  else if(phase===5){check(!review.nativeRenderer&&!review.richSessionId&&review.file.binary,"binary metadata keeps bounded native text path without a renderer");review.selectFile("a.js");phase=6}
  else if(phase===6){review.visible=false;check(!review.richSessionId&&!Object.keys(bridge.inlineDocuments).length,"closing frees all review sessions");console.log("NATIVE_DIFF_FLOW_QML_PASS: native snapshots, shared surface/chrome, layout/wrap without refetch, narrow mode, keyboard file navigation, binary metadata and lifecycle cleanup");Qt.quit()}
 }}
}
