import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property int waited:0
 function check(ok,msg){if(!ok){console.error("RENDER_PANEL_QML_FAIL "+msg);Qt.exit(1)}}
 function rows(n){var r=[{id:"m1",role:"assistant",text:"Before",createdAt:1},{id:"html/old",role:"assistant",text:"",attachments:[{id:"page-old",type:"file",name:"Old.html",mimeType:"text/html"}],createdAt:2}];if(n>1)r.push({id:"html/new",role:"assistant",text:"",attachments:[{id:"page-new",type:"file",name:"Ideas.html",mimeType:"text/html"}],createdAt:3});return r}
 function push(n){var b=Test.T3Backend;b.messagesAboutToChange();b.transcript=rows(n);b.messagesUpdated()}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({htmlRenders:"side",windowMode:"floating",windowWidth:1400,windowHeight:900});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;push(1);phase=2
  }else if(phase===2){if(panel.restoringConversation)return;check(panel.renderSide,"wide window uses the side panel, available "+panel.browserAvailableWidth);check(!panel.renderShown,"history renders do not open the panel");check(!keys.findChild(panel.conversationView.itemAtIndex(1),"t3InlineHtml"),"side mode keeps renders out of the conversation");check(!!keys.findChild(panel.conversationView.itemAtIndex(1),"t3RenderCard"),"conversation shows a render card");push(2);phase=3
  }else if(phase===3){check(panel.renderShown&&panel.renderAttachment.id==="page-new","a new render opens in the side panel");var side=keys.findChild(panel,"t3RenderPanel");check(side&&side.visible&&!!side.sessionId&&b.inlineDocuments[side.sessionId],"side panel opens a renderer session");check(keys.findChild(side,"t3RenderPanelTitle").text==="Ideas","panel shows the render title");
   keys.mouseClick(keys.findChild(panel.conversationView.itemAtIndex(1),"t3RenderCard"));phase=4
  }else if(phase===4){check(panel.renderAttachment.id==="page-old","a card shows its render");var side=keys.findChild(panel,"t3RenderPanel");check(Object.keys(b.inlineDocuments).length===1,"one renderer session at a time");keys.mouseClick(keys.findChild(side,"t3RenderPanelClose"));phase=5
  }else if(phase===5){check(!panel.renderShown,"close hides the panel");check(Object.keys(b.inlineDocuments).length===0,"closing releases the renderer");b.rememberView({htmlRenders:"inline"});phase=7
  }else if(phase===7){check(!panel.renderSide,"inline preference");check(!!keys.findChild(panel.conversationView,"t3InlineHtml")&&!keys.findChild(panel.conversationView,"t3RenderCard"),"inline mode renders in the conversation");panel.close();console.log("RENDER_PANEL_QML_PASS: side panel opens new renders, cards, single session, close and inline preference");Qt.quit()}
 }}
}
