import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property int waited:0
 function check(ok,msg){if(!ok){console.error("QML_PREVIEW_QML_FAIL "+msg);Qt.exit(1)}}
 function wait(ok,msg){if(ok){waited=0;return true}if(++waited>80)check(false,msg);return false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 readonly property string good:"import QtQuick\nimport QtQuick.Controls\nRectangle {\n  width: 320; height: 180; color: omarchy.surface\n  Button { anchors.centerIn: parent; text: \"Hola\" }\n}"
 readonly property string shell:"import QtQuick\nimport Quickshell.Io\nProcess { command: [\"sh\", \"-c\", \"id\"]; running: true }"
 readonly property string broken:"import QtQuick\nRectangle { width: 200; height: 100; colr: \"red\" }"
 function inline(){var row=panel.conversationView.itemAtIndex(0);return row?keys.findChild(row,"t3InlineQmlPreview"):null}
 function doc(item){return item?Test.T3Backend.inlineDocuments[item.sessionId]:null}
 Timer{interval:250;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1500,windowHeight:900,htmlRenders:"side"});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();b.send({op:"visible",value:true});phase=1
  }else if(phase===1){if(b.draft.loading)return;b.messagesAboutToChange();b.transcript=[{id:"a",role:"assistant",text:"qml",rendered:"x",codeBlocks:[{language:"qml",code:root.good},{language:"qml",code:root.shell},{language:"qml",code:root.broken},{language:"js",code:"1"}],createdAt:1}];b.messagesUpdated();phase=2
  }else if(phase===2){
   check(!b.codeAction(b.transcript[0],"ot3-code-preview://3"),"only qml blocks preview");check(!b.codeAction(b.transcript[0],"ot3-code-preview://9"),"unknown blocks are ignored")
   check(b.codeAction(b.transcript[0],"ot3-code-preview://0"),"qml blocks open a preview");phase=3
  }else if(phase===3){
   check(panel.renderShown&&panel.renderAttachment.generated==="qml","side preference opens the renders panel");check(panel.renderList.some(function(a){return a.generated==="qml"}),"preview joins the render list");phase=4
  }else if(phase===4){var side=keys.findChild(panel,"t3RenderPanel"),d=side&&b.inlineDocuments[side.sessionId];if(d&&d.error)check(false,"renderer error "+d.error);if(!wait(d&&d.frameUrl,"side preview produced no frame "+JSON.stringify(d)))return
   panel.closeSide();b.rememberView({htmlRenders:"inline"});phase=5
  }else if(phase===5){if(panel.renderSide||panel.restoringConversation)return;check(b.codeAction(b.transcript[0],"ot3-code-preview://0"),"inline preview opens");phase=6
  }else if(phase===6){var item=inline(),d=doc(item);if(d&&d.error)check(false,"inline renderer error "+d.error);if(!wait(d&&d.frameUrl,"inline preview produced no frame "+JSON.stringify(d)))return
   check(item.height<520,"fixed-size components shrink the inline page ("+item.height+")");check(!panel.renderShown,"inline preference keeps the side panel closed")
   b.codeAction(b.transcript[0],"ot3-code-preview://0");phase=7
  }else if(phase===7){if(!wait(!inline(),"the action again closes the inline preview"))return;check(b.codeAction(b.transcript[0],"ot3-code-preview://1"),"blocked previews still open");phase=8
  }else if(phase===8){var item=inline(),d=doc(item);if(!wait(d&&d.error,"Quickshell imports are refused"))return
   check(d.error==="qml_import_blocked:Quickshell.Io","blocked import code "+d.error);check(!d.frameUrl,"refused previews never start the renderer")
   var err=keys.findChild(item,"t3DocumentError");check(err&&err.visible&&err.text.indexOf("Quickshell.Io")>=0&&err.text.indexOf("QtQuick.Layouts")>=0,"the refusal names the module and the allowed list: "+(err&&err.text))
   keys.findChild(item,"t3DocumentDismiss").clicked();phase=9
  }else if(phase===9){if(!wait(!inline(),"the close button dismisses the preview"))return;check(b.codeAction(b.transcript[0],"ot3-code-preview://2"),"broken previews open");phase=10
  }else if(phase===10){var item=inline(),d=doc(item);if(d&&d.error)check(false,"compile errors are shown in the frame, not as a request error: "+d.error);if(!wait(d&&d.frameUrl,"broken preview produced no frame"))return
   panel.close();console.log("QML_PREVIEW_QML_PASS: qml blocks preview in the sandboxed renderer, inline or in the side panel; forbidden imports are refused");Qt.quit()}
 }}
}
