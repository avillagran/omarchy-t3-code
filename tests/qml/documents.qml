import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/DocumentTheme.js" as Theme
ShellRoot {
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("DOCUMENT_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:300;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;
   var theme=Theme.theme(Qt.color("#111111"),Qt.color("#bbbbbb"),Qt.color("#e99700"),{green:"#00aa00"});check(theme.appearance==="dark","appearance");check(theme.variables["--success"]==="#00aa00","semantic colors");
   var html=Theme.page({contents:'<html><head></head><body>Report</body></html>',baseUrl:'about:blank',theme:theme});check(html.indexOf('sandbox="allow-scripts allow-forms"')>=0,"sandbox");check(html.indexOf('--background:#111111')>=0,"native theme tokens");
   b.state={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",projectId:"p",worktreePath:"/work/tree",title:"One"}],projects:[{id:"p",key:"fixture/p",environmentId:"fixture",workspaceRoot:"/work/root"}],environments:[]};fixture=b.state;b.selectThread("fixture","one");b.transcript=[{id:"report",role:"assistant",text:"Report",rendered:'<a href="/home/node/report.html">Report</a>',fileReferences:[{name:"report.html",path:"/home/node/report.html"}],createdAt:1}];panel.open();phase=1
  }else if(phase===1){
   var row=panel.conversationView.itemAtIndex(0);var refs=keys.findChild(row,"t3FileReference");check(!!refs,"file reference chip");check(refs.filePath==="/home/node/report.html","original host path");keys.mouseClick(refs,refs.width/2,refs.height/2);phase=2
  }else if(phase===2){
   check(!!b.documentOpen.requestId,"click requests native document viewer");var id=b.documentOpen.requestId;b.openReference("javascript:alert(1)");check(b.documentOpen.requestId===id,"unsupported schemes ignored");check(keys.findChild(panel,"t3EmbeddedDocumentPopup").visible,"file viewer opens inside the panel");b.documentOpen=Object.assign({},b.documentOpen,{busy:false,error:"",data:{html:true,title:"Report",contents:"<h1>Report</h1>"},frameUrl:Qt.resolvedUrl("UnderTest/tests/fixtures/image.png").toString(),frameWidth:640,frameHeight:400});phase=3
  }else if(phase===3){
   var frame=keys.findChild(panel,"t3EmbeddedDocumentFrame");check(frame&&frame.status===Image.Ready,"embedded frame is visible in native UI");var view=keys.findChild(panel,"t3DocumentView");view.sourceMode=true;check(keys.findChild(view,"t3DocumentSource").text==="<h1>Report</h1>","source view preserves document text");b.documentOpen=Object.assign({},b.documentOpen,{data:{markdown:true,title:"/work/report.md",contents:"# Report\n\n**Bold**",rendered:"<h1>Report</h1><p><strong>Bold</strong></p>"}});phase=4
  }else if(phase===4){
   var view=keys.findChild(panel,"t3DocumentView"),md=keys.findChild(view,"t3DocumentMarkdown");check(!view.sourceMode&&md.visible&&md.textFormat===TextEdit.RichText,"Markdown opens in rendered preview");check(md.text.indexOf("<h1")>=0,"formatted document reaches preview");check(!!keys.findChild(view,"t3DocumentMarkdownWheel")&&!!keys.findChild(view,"t3DocumentSourceWheel"),"rendered and source documents share scroll controller");view.sourceMode=true;check(keys.findChild(view,"t3DocumentSource").text==="# Report\n\n**Bold**","Markdown source preserved");keys.findChild(panel,"t3EmbeddedDocumentPopup").close();check(!b.documentOpen.requestId,"closing modal releases its document session");panel.close();console.log("DOCUMENT_QML_PASS: themed document sandbox, filename chip, click dispatch and scheme guard");Qt.quit()
  }
 }}
}
