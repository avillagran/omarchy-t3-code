import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property string session:""
 function check(ok,msg){if(!ok){console.error("MCP_APP_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1300,windowHeight:900,htmlRenders:"inline"});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;b.messagesAboutToChange();b.transcript=[{id:"app/i1",role:"assistant",text:"",createdAt:1,attachments:[{type:"file",id:"att",name:"forecast.html",mimeType:"text/html",mcpApp:{attachmentId:"att",server:"weather",tool:"forecast",resourceUri:"ui://weather/forecast",itemId:"i1",threadId:"one"}}]}];b.messagesUpdated();phase=2
  }else if(phase===2){if(panel.restoringConversation)return;var page=keys.findChild(panel.conversationView.itemAtIndex(0),"t3InlineHtml");check(page&&!!page.sessionId,"app renders inline");session=page.sessionId;check(!!b.inlineDocuments[session].app,"session knows it is an app")
   b.receive(JSON.stringify({type:"document-app-request",data:{requestId:session,request:{id:1,method:"ui/initialize",params:{}}}}));check(Object.keys(b.appCalls).length===0,"initialize is answered by the host")
   b.receive(JSON.stringify({type:"document-app-request",data:{requestId:session,request:{id:2,method:"tools/call",params:{name:"refresh",arguments:{}}}}}));var calls=Object.keys(b.appCalls);check(calls.length===1&&b.appCalls[calls[0]].rpcId===2,"tool calls go to the server")
   b.action({op:"mcp-app",requestId:calls[0],result:{content:[]}});check(Object.keys(b.appCalls).length===0,"reply clears the call")
   b.receive(JSON.stringify({type:"document-app-request",data:{requestId:"other",request:{id:3,method:"tools/call",params:{name:"x"}}}}));check(Object.keys(b.appCalls).length===0,"requests from unknown pages are ignored")
   panel.close();console.log("MCP_APP_QML_PASS: inline app, initialize, tool call round trip, unknown page");Qt.quit()}
 }}
}
