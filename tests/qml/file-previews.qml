import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property bool captured:false
 property var image:({type:"image",id:"draft-image",name:"draft.png",mimeType:"image/png",sizeBytes:128})
 function check(ok,msg){if(!ok){console.error("FILE_PREVIEWS_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:300;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",projectId:"p",title:"Preview files"}],projects:[{id:"p",key:"fixture/p",environmentId:"fixture",workspaceRoot:"/work/tree"}],environments:[]};fixture=b.state;b.selectThread("fixture","one");b.transcript=[{id:"changes",role:"changes",changes:{additions:4,deletions:1,files:[{path:"index.html",kind:"added",additions:3},{path:"README.md",kind:"modified",additions:1},{path:"gone.txt",kind:"deleted",deletions:1}]},createdAt:1}];panel.open();phase=1}
  else if(phase===1){if(panel.restoringConversation)return;var row=panel.conversationView.itemAtIndex(0);if(!row)return;var eye=keys.findChild(row,"t3PreviewChangedFile_index.html");check(eye&&eye.glyph==="\uf06e"&&eye.glyphFont==="FiraCode Nerd Font"&&eye.glyphScale===0.85,"Nerd Font eye on file row");check(!keys.findChild(row,"t3PreviewChangedFile_gone.txt").enabled,"deleted files have no current content");keys.mouseClick(eye,eye.width/2,eye.height/2);phase=2}
  else if(phase===2){check(!!b.documentOpen.requestId&&keys.findChild(panel,"t3EmbeddedDocumentPopup").visible,"eye opens native document viewer");check(!keys.findChild(panel,"t3DiffPopup").visible,"eye does not open diff");keys.findChild(panel,"t3EmbeddedDocumentPopup").close();b.patchDraft(b.selectedKey,{loading:false,loadError:"",busy:false,text:"Keep this draft",attachments:[{uploadId:"upload-one",name:"draft.png",uploading:false,attachment:root.image},{uploadId:"upload-two",name:"pending.png",uploading:true}]});b.imageStates={"fixture/draft-image":{url:Qt.resolvedUrl("UnderTest/tests/fixtures/image.png"),loading:false}};phase=3}
  else if(phase===3){var eye=keys.findChild(panel.composerView,"t3PreviewDraftAttachment_0");check(eye&&eye.enabled,"uploaded draft attachment is previewable before send");check(!keys.findChild(panel.composerView,"t3PreviewDraftAttachment_1").enabled,"upload in progress is not previewable");keys.mouseClick(eye,eye.width/2,eye.height/2);phase=4}
  else if(phase===4){check(keys.findChild(panel,"t3ImagePopup").visible&&keys.findChild(panel,"t3FullImage").status===Image.Ready,"draft image decodes in full viewer");check(b.draft.text==="Keep this draft"&&b.draft.attachments.length===2&&!b.draft.attempt,"preview preserves unsent text and attachments");keys.findChild(panel,"t3ImagePopup").close();phase=5}
  else if(phase===5){var eye=keys.findChild(panel.composerView,"t3PreviewDraftAttachment_0");keys.mouseClick(eye,eye.width/2,eye.height/2);phase=6}
  else if(phase===6){check(keys.findChild(panel,"t3ImagePopup").visible,"same image can be reopened");keys.findChild(panel,"t3ImagePopup").close();keys.findChild(panel,"t3PanelLayout").grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-file-previews.png")});phase=7}
  else if(phase===7){if(!captured)return;panel.close();console.log("FILE_PREVIEWS_QML_PASS: file eye routes to native viewer, deleted/upload guards, reopen draft image without send/remove");Qt.callLater(Qt.quit)}
 }}
}
