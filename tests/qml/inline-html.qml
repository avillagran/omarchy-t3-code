import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property string first:"";property string second:""
 function check(ok,msg){if(!ok){console.error("INLINE_HTML_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.edit("Keep this draft");b.messagesAboutToChange();b.transcript=[{id:"html/one",role:"assistant",text:"",attachments:[{id:"page-one",type:"file",name:"Interactive.html",mimeType:"text/html"}],createdAt:1}];b.messagesUpdated();phase=2
  }else if(phase===2){if(panel.restoringConversation)return;var row=panel.conversationView.itemAtIndex(0),page=keys.findChild(row,"t3InlineHtml");check(page&&page.active&&!!page.sessionId,"HTML appears inline automatically");first=page.sessionId;check(!b.documentOpen.requestId&&!keys.findChild(panel,"t3EmbeddedDocumentPopup").visible,"auto-render does not open modal");b.action({op:"document-open",requestId:first,result:{html:true,title:"Interactive.html",contents:"<button>Interact</button>"}});b.receive(JSON.stringify({type:"document-frame",data:{requestId:first,url:Qt.resolvedUrl("UnderTest/tests/fixtures/image.png").toString(),revision:1,width:640,height:400}}));phase=3
  }else if(phase===3){var page=keys.findChild(panel.conversationView.itemAtIndex(0),"t3InlineHtml"),frame=keys.findChild(page,"t3EmbeddedDocumentFrame");check(frame&&frame.status===Image.Ready,"inline uses actual renderer frames");check(!frame.activeFocus,"loading does not steal keyboard focus");second=b.openInlineDocument({id:"page-two",name:"Second.html",mimeType:"text/html"});b.action({op:"document-open",requestId:second,result:{html:true,title:"Second.html"}});check(Object.keys(b.inlineDocuments).length===2,"independent inline sessions coexist");b.openReference("",{id:"page-one",name:"Interactive.html",mimeType:"text/html"});check(!!b.documentOpen.requestId&&b.inlineDocuments[first]&&b.inlineDocuments[second],"expanded modal preserves inline sessions");keys.findChild(panel,"t3EmbeddedDocumentPopup").close();check(!b.documentOpen.requestId&&!!b.inlineDocuments[first],"closing modal leaves inline page alive");b.closeInlineDocument(second);b.receive(JSON.stringify({type:"document-frame",data:{requestId:second,url:"file:///stale.png",revision:9}}));check(!b.inlineDocuments[second],"late frame cannot revive a released renderer");b.receive(JSON.stringify({type:"document-size",data:{requestId:first,height:850}}));check(page.height>=850,"native measured content height expands inline page");check(b.draft.text==="Keep this draft","rendering preserves composer");panel.close();phase=4
  }else if(phase===4){check(Object.keys(b.inlineDocuments).length===0,"hiding panel releases inline renderers");console.log("INLINE_HTML_QML_PASS: automatic inline frame, independent sessions, expansion, height, stale frame, focus and cleanup");Qt.quit()}
 }}
}
