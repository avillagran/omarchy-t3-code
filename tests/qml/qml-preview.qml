import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property int waited:0
 function check(ok,msg){if(!ok){console.error("QML_PREVIEW_QML_FAIL "+msg);Qt.exit(1)}}
 function wait(ok,msg){if(ok){waited=0;return true}if(++waited>(phase===9?40:phase>=15?12:80))check(false,msg);return false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 readonly property string good:"import QtQuick\nimport QtQuick.Controls\nRectangle {\n  width: 320; height: 180; color: omarchy.surface\n  Button { anchors.centerIn: parent; text: \"Hola\" }\n}"
 readonly property string shell:"import QtQuick\nimport Quickshell.Io\nProcess { command: [\"sh\", \"-c\", \"id\"]; running: true }"
 readonly property string broken:"import QtQuick\nRectangle { width: 200; height: 100; colr: \"red\" }"
 readonly property string looping:"import QtQuick\nItem { width: 10; height: 10; Component.onCompleted: { while (true) {} } }"
 function block(i,code){return "<!--ot3-qml:"+i+"--><table><tr><td>qml</td></tr></table><pre><code>"+code.replace(/</g,"&lt;")+"</code></pre><!--/ot3-qml:"+i+"-->"}
 readonly property var message:({id:"a",role:"assistant",text:"qml",createdAt:1,
  codeBlocks:[{language:"qml",code:good},{language:"qml",code:shell},{language:"qml",code:broken},{language:"js",code:"1"},{language:"qml",code:looping}],
  rendered:"<p>Antes del bloque</p>"+block(0,good)+"<p>Entre bloques</p>"+block(1,shell)+block(2,broken)+"<pre><code>1</code></pre>"+block(4,looping)+"<p>Después</p>"})
 function row(){return panel.conversationView.itemAtIndex(0)}
 function blocks(){var r=row(),out=[];if(!r)return out;(function walk(o){if(!o)return;if(o.objectName==="t3QmlBlock")out.push(o);var k=o.children||[];for(var i=0;i<k.length;i++)walk(k[i])})(r);return out.sort(function(a,b){return a.mapToItem(r,0,0).y-b.mapToItem(r,0,0).y})}
 // Scroll a block into view once (stop following the tail so nothing scrolls it away again).
 function reveal(i){var bl=blocks();if(!bl[i]||bl[i].parent.inView)return;panel.cancelScrollRestore();panel.followingTail=false;var v=panel.conversationView;v.contentY=Math.max(0,bl[i].mapToItem(v.contentItem,0,0).y-40)}
 function doc(block){var p=block&&block.preview;return p?Test.T3Backend.inlineDocuments[p.sessionId]:null}
 Timer{interval:250;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1500,windowHeight:900,htmlRenders:"inline"});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();b.send({op:"visible",value:true});phase=1
  }else if(phase===1){if(b.draft.loading)return;b.messagesAboutToChange();b.transcript=[Object.assign({},root.message,{streaming:true})];b.messagesUpdated();phase=2
  }else if(phase===2){if(!wait(!!row(),"the message row appears"))return
   check(blocks().length===0,"streaming messages keep their code");check(keys.findChild(row(),"chatMessageText").text.indexOf("Hola")>=0,"the code is visible while streaming")
   b.messagesAboutToChange();b.transcript=[root.message];b.messagesUpdated();phase=3
  }else if(phase===3){var bl=blocks();if(!wait(bl.length===4,"four QML blocks show in place: "+bl.length))return
   var text=keys.findChild(row(),"chatMessageText").text;check(text.indexOf("Antes del bloque")>=0&&text.indexOf("Hola")<0,"the text before the first block stays, its code does not")
   var parts=[],r=row();(function walk(o){if(!o)return;if(o.objectName==="chatMessagePart")parts.push(o.text);var k=o.children||[];for(var i=0;i<k.length;i++)walk(k[i])})(r)
   check(parts.some(function(t){return t.indexOf("Entre bloques")>=0})&&parts.some(function(t){return t.indexOf("Después")>=0}),"text between and after blocks keeps its place")
   check(bl[0].mapToItem(r,0,0).y<bl[1].mapToItem(r,0,0).y,"blocks keep their order");check(!bl[0].showCode&&bl[0].preview,"the preview is shown by default");phase=4
  }else if(phase===4){root.reveal(0);var bl=blocks(),d=doc(bl[0]);if(d&&d.error)check(false,"renderer error "+d.error);if(!wait(d&&d.frameUrl,"the first block renders: "+JSON.stringify(d)))return
   var frame=keys.findChild(bl[0],"t3EmbeddedDocumentFrame");if(!wait(frame&&frame.status===Image.Ready,"the frame decodes in the shell"))return
   check(bl[0].preview.height<400,"fixed-size components keep a short preview ("+bl[0].preview.height+")");check(!panel.renderShown,"inline renders keep the side panel closed")
   keys.findChild(bl[0],"t3QmlBlockCode").clicked();phase=5
  }else if(phase===5){var bl=blocks(),src=keys.findChild(bl[0],"t3QmlBlockSource");if(!wait(bl[0].showCode&&src&&src.visible,"the code button shows the code"))return
   check(src.text.indexOf("Button { anchors.centerIn")>=0,"the whole code is shown");check(!bl[0].preview.sessionId&&!bl[0].preview.visible,"the renderer closes while the code is shown")
   check(keys.findChild(bl[0],"t3QmlBlockCode").text.length>0,"the button can switch back");keys.findChild(bl[0],"t3QmlBlockCode").clicked();phase=6
  }else if(phase===6){root.reveal(0);var bl=blocks(),d=doc(bl[0]);if(!wait(!bl[0].showCode&&d&&d.frameUrl,"the preview comes back"))return;phase=7
  }else if(phase===7){root.reveal(1);var bl=blocks(),d=doc(bl[1]);if(!wait(d&&d.error,"Quickshell imports are refused"))return
   check(d.error==="qml_import_blocked:Quickshell.Io","blocked import code "+d.error);check(!d.frameUrl,"refused previews never start the renderer")
   var err=keys.findChild(bl[1],"t3DocumentError");check(err&&err.visible&&err.text.indexOf("Quickshell.Io")>=0&&err.text.indexOf("QtQuick.Layouts")>=0,"the refusal names the module and the allowed list: "+(err&&err.text))
   check(bl[1].preview.height<200,"a refused preview is short ("+bl[1].preview.height+")");phase=8
  }else if(phase===8){root.reveal(2);var bl=blocks(),d=doc(bl[2]);if(d&&d.error)check(false,"compile errors are drawn in the frame: "+d.error);if(!wait(d&&d.frameUrl,"the broken block renders its errors"))return;phase=9
  }else if(phase===9){root.reveal(3);var bl=blocks(),d=doc(bl[3]);if(!wait(d&&d.error,"endless loops are reported"))return
   check(d.error==="qml_preview_unresponsive","loop error code "+d.error);var err=keys.findChild(bl[3],"t3DocumentError");check(err&&err.visible&&err.text.indexOf("qml_preview")<0&&err.text.length>20,"the loop is explained: "+(err&&err.text))
   check(!b.codeAction(root.message,"ot3-code-preview://3"),"other languages have no preview");b.rememberView({htmlRenders:"side"});phase=10
  }else if(phase===10){if(!panel.renderSide)return;var bl=blocks();if(!wait(bl.length===4&&bl[0].side&&!bl[0].preview,"side renders turn blocks into cards"))return
   check(keys.findChild(bl[0],"t3QmlBlockSideCard").visible,"the card is shown");check(panel.renderList.filter(function(a){return a.generated==="qml"}).length===4,"QML blocks join the thread's renders")
   bl[0].sideRequested();phase=11
  }else if(phase===11){check(panel.renderShown&&panel.renderAttachment.generated==="qml"&&panel.renderAttachment.blockIndex===0,"the card opens the side panel");var side=keys.findChild(panel,"t3RenderPanel"),d=side&&b.inlineDocuments[side.sessionId];if(d&&d.error)check(false,"side renderer error "+d.error);if(!wait(d&&d.frameUrl,"the side preview renders"))return
   check(blocks()[0].shownInSide,"the card marks the block shown in the side panel");phase=12
  }else if(phase===12){
   // A new QML answer while the thread is open goes straight to the panel.
   b.messagesAboutToChange();b.transcript=[root.message,{id:"c",role:"assistant",text:"otro",createdAt:2,codeBlocks:[{language:"qml",code:"import QtQuick\nRectangle { width: 90; height: 90; color: omarchy.accent }"}],rendered:"<p>Nuevo</p>"+root.block(0,"Rectangle {}")}];b.messagesUpdated();phase=13
  }else if(phase===13){if(!wait(panel.renderShown&&panel.renderAttachment.messageId==="c","a new QML block opens in the side panel"))return
   b.rememberView({htmlRenders:"inline"});phase=14
  }else if(phase===14){if(!wait(!panel.renderSide&&blocks().length===4&&!blocks()[0].side,"blocks come back inline"))return;panel.close();phase=15
  }else if(phase===15){
   // Closing the panel releases every renderer, and spinners hand back the shared clock.
   if(!wait(Object.keys(b.inlineDocuments).length===0,"renderers close with the panel: "+JSON.stringify(Object.keys(b.inlineDocuments).map(function(k){var d=b.inlineDocuments[k];return {error:d.error,frame:!!d.frameUrl,title:d.data&&d.data.title}}))))return
   phase=16
  }else if(phase===16){var fast=Object.keys(Test.T3Ticker.requests).filter(function(k){return Test.T3Ticker.requests[k]<=16});if(!wait(fast.length===0,"no 60 fps ticket is left behind: "+JSON.stringify(Test.T3Ticker.requests)))return
   console.log("QML_PREVIEW_QML_PASS: QML blocks show their preview in place with the code behind a button; side renders, refusals, errors and loops covered");Qt.quit()}
 }}
}
