import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property int waited:0
 function check(ok,msg){if(!ok){console.error("DIAGRAM_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:250;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1500,windowHeight:900,htmlRenders:"side"});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();b.send({op:"visible",value:true});phase=1
  }else if(phase===1){if(b.draft.loading)return;b.messagesAboutToChange();b.transcript=[{id:"a",role:"assistant",text:"diagram",codeBlocks:[{language:"mermaid",code:"graph TD\n  A[Start] --> B{Works?}\n  B -->|yes| C[Done]"}],createdAt:1}];b.messagesUpdated();phase=2
  }else if(phase===2){check(!b.codeAction(b.transcript[0],"ot3-code-diagram://3"),"unknown blocks are ignored");check(b.codeAction(b.transcript[0],"ot3-code-diagram://0"),"mermaid blocks open a diagram");phase=3
  }else if(phase===3){check(panel.renderShown&&panel.renderAttachment.generated==="mermaid","diagram opens in the renders panel");check(panel.renderList.some(function(a){return a.generated==="mermaid"}),"diagram joins the render list");phase=4
  }else if(phase===4){var side=keys.findChild(panel,"t3RenderPanel");var d=side&&b.inlineDocuments[side.sessionId];waited++;if(d&&d.error)check(false,"renderer error "+d.error);if(!d||!d.frameUrl){if(waited<80)return;check(false,"diagram renderer produced no frame "+JSON.stringify(d))}
   panel.close();console.log("DIAGRAM_QML_PASS: mermaid code blocks render through the bundled mermaid.js in the side panel");Qt.quit()}
 }}
}
