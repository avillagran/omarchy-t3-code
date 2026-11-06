import QtQuick
import QtQuick.Window
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/TerminalLinkRouting.js" as Routing
ShellRoot {
 id:root;property int phase:0;property int serial:0;property var calls:[];property var external:[];property var activations:[];property var viewport:null;property var pending:null;property int before:0
 readonly property var pane:paneLoader.item
 function check(ok,message){if(!ok){console.error("TERMINAL_LINK_QML_FAIL "+message);Qt.exit(1)}}
 function response(command,kind){return {op:command.op,requestId:command.requestId,environment:command.environment,thread:command.thread,result:{environment:command.environment,thread:command.thread,editors:[{id:"zed"}],terminalLink:{target:command.target,path:command.target.replace(/:[0-9]+(?::[0-9]+)?$/,""),kind:kind}}}}
 TestCase{id:keys;when:false}
 QtObject {
  id:fake
  property string selectedKey:"fixture/thread";property string selectedEnvironment:"fixture";property string selectedThread:"thread"
  property var selectedInfo:({projectId:"project",worktreePath:"/worktree"});property var state:({environments:[{id:"fixture",state:"ready"}],projects:[]})
  property var viewState:({preferredEditor:"zed"});property var catalog:({canWrite:true});property bool canOpenTerminal:true;property bool previewWorkerReady:true;property bool panelVisible:true
  property var terminalViews:({});property var terminalPathRequests:({});property var terminalPathMetadata:({});property var terminalUrlQueue:[];property var terminalLinkFailures:({})
  property var clientSettings:({busy:false,error:"",data:{browserLinkTarget:"system",browserDefaultProfileId:"default",browserProfiles:[],browserDefaultViewport:{_tag:"fill"}}})
  property bool previewVisible:false;property var previewWatch:({});property var previewState:null;property var previewOperations:({})
  signal terminalViewEvent(string request,var data)
  function uuid(){return "qml-"+(++root.serial)}
  function attachTerminal(id,columns,rows,restart){var request=uuid(),views=Object.assign({},terminalViews);views[request]={key:selectedKey,terminalId:id,status:"running",label:"Terminal",cwd:"/worktree",error:""};terminalViews=views;return request}
  function detachTerminal(request){var views=Object.assign({},terminalViews);delete views[request];terminalViews=views}
  function terminalInput(request,op,values){}
  function activateTerminal(id){}
  function copyText(text,primary){}
  function send(value){root.calls=root.calls.concat([value])}
  function rememberView(value){viewState=Object.assign({},viewState,value)}
  function openTerminalSystemUrl(url){root.external=root.external.concat([url]);return true}
  function requestClientSettings(){clientSettings=Object.assign({},clientSettings,{busy:true});send({op:"client-settings"})}
  function terminalBrowserRequested(){previewVisible=true;previewWatch={environment:selectedEnvironment,thread:selectedThread,busy:true};send({op:"show-browser"})}
  function previewAction(action,input){var id=uuid(),operations=Object.assign({},previewOperations);operations[JSON.stringify([selectedEnvironment,selectedThread])]={requestId:id,state:"pending"};previewOperations=operations;send({op:"preview-action",action:action,input:input,requestId:id});return true}
  function openTerminalLink(uri,forceBrowser,terminalId){root.activations=root.activations.concat([{uri:uri,forceBrowser:forceBrowser,terminalId:terminalId}]);return Routing.open(fake,uri,forceBrowser,terminalId)}
  function hoverTerminalLink(uri,terminalId){Routing.hover(fake,uri,terminalId)}
  function canActivateTerminalLink(uri,terminalId){return Routing.allowed(fake,uri,terminalId)}
  function clearTerminalLinkFailure(key,terminalId){Routing.clearFailure(fake,key,terminalId)}
  onSelectedKeyChanged:Routing.reset(fake)
 }
 Window {
  width:740;height:420;visible:true
  Loader{id:paneLoader;anchors.fill:parent;sourceComponent:Test.T3TerminalPane{bridge:fake;terminalId:"term";current:true}}
 }
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  if(phase===0){if(!pane||!pane.requestId)return;var loader=keys.findChild(pane,"t3TerminalViewportLoader");if(!loader||!loader.item)return;viewport=loader.item;viewport.setGrid(80,16);viewport.feed("https://example.test/path next");viewport.selectRange(0,26,0,30);keys.mouseClick(viewport,2,2,Qt.LeftButton,Qt.NoModifier);check(root.activations.length===1&&!root.activations[0].forceBrowser,"plain URL click reaches actual pane handler");check(fake.terminalUrlQueue.length===1,"URL waits for client preference read");fake.clientSettings=Object.assign({},fake.clientSettings,{busy:false});Routing.pump(fake);check(root.external.length===1&&root.external[0]==="https://example.test/path","system preference opens URL once");check(viewport.selectedText==="next","link click retains existing terminal selection");phase=1
  }else if(phase===1){viewport.reset();viewport.feed("https://example.test/forced");fake.clientSettings={busy:false,error:"",data:{browserLinkTarget:"app"}};before=root.calls.length;keys.mouseClick(viewport,2,2,Qt.LeftButton,Qt.ControlModifier);check(root.external.length===2&&root.activations[1].forceBrowser,"Ctrl forces the system browser");check(root.calls.length===before,"forced browser does not start preview or settings reads");phase=2
  }else if(phase===2){viewport.reset();viewport.feed("https://example.test/app");fake.clientSettings={busy:false,error:"",data:{browserLinkTarget:"app",browserDefaultProfileId:"work",browserProfiles:[{id:"work",name:"Work",kind:"persistent"}],browserDefaultViewport:{_tag:"fill"}}};keys.mouseClick(viewport,2,2,Qt.LeftButton,Qt.NoModifier);fake.clientSettings=Object.assign({},fake.clientSettings,{busy:false});Routing.pump(fake);check(root.calls[root.calls.length-1].op==="show-browser","app preference requests the browser side tab");fake.previewWatch={environment:"fixture",thread:"thread",busy:false,connected:true,canOperate:true};fake.previewState={listLoaded:true};Routing.pump(fake);pending=root.calls[root.calls.length-1];check(pending.op==="preview-action"&&pending.input.address==="https://example.test/app"&&pending.input.profileId==="work","app routing keeps URL and selected profile");fake.previewOperations=({"[\"fixture\",\"thread\"]":{requestId:pending.requestId,state:"complete"}});Routing.previewReceipt(fake,{requestId:pending.requestId,environment:"fixture",thread:"thread",result:{state:"complete",snapshot:{tabId:"tab"}}});check(root.external.length===2&&fake.terminalUrlQueue.length===0,"successful preview never falls back externally");phase=3
  }else if(phase===3){viewport.reset();viewport.feed("src/main.ts:12:7");before=root.calls.length;check(!fake.canActivateTerminalLink("src/main.ts:12:7","term"),"path admission waits for filesystem metadata");keys.mouseMove(viewport,12,2);keys.mouseMove(viewport,2,2);phase=4
  }else if(phase===4){if(root.calls.length===before)return;pending=root.calls[root.calls.length-1];check(pending.op==="thread-details-read"&&pending.target==="/worktree/src/main.ts:12:7","hover resolves path in the selected worktree");Routing.pathReceipt(fake,response(pending,"file"));check(fake.canActivateTerminalLink("src/main.ts:12:7","term"),"metadata admits actual path link");keys.mouseClick(viewport,2,2,Qt.LeftButton,Qt.NoModifier);pending=root.calls[root.calls.length-1];check(pending.op==="thread-details-read","click performs its guarded validation");Routing.pathReceipt(fake,response(pending,"file"));pending=root.calls[root.calls.length-1];check(pending.op==="thread-details-open-editor"&&pending.target==="/worktree/src/main.ts:12:7"&&pending.editor==="zed","actual pane dispatches preferred editor with line/column");Routing.pathReceipt(fake,Object.assign({},pending,{result:{opened:true,target:pending.target}}));phase=5
  }else if(phase===5){viewport.reset();viewport.feed("\\\\server\\share");before=root.calls.length;keys.mouseClick(viewport,2,2,Qt.LeftButton,Qt.NoModifier);check(root.calls.length===before,"network path reaches neither metadata nor editor dispatch");fake.catalog={canWrite:false};viewport.reset();viewport.feed("src/other.ts");keys.mouseClick(viewport,2,2,Qt.LeftButton,Qt.NoModifier);check(root.calls.length===before,"read-only path remains inert");phase=6
  }else if(phase===6){fake.catalog={canWrite:true};viewport.reset();viewport.feed("src/late.ts");keys.mouseClick(viewport,2,2,Qt.LeftButton,Qt.NoModifier);pending=root.calls[root.calls.length-1];before=root.calls.filter(function(c){return c.op==="thread-details-open-editor"}).length;fake.selectedThread="other";fake.selectedKey="fixture/other";Routing.pathReceipt(fake,response(pending,"file"));check(root.calls.filter(function(c){return c.op==="thread-details-open-editor"}).length===before,"late metadata cannot open an editor for a changed thread");paneLoader.active=false;console.log("TERMINAL_LINK_QML_PASS: actual viewport/pane URL and path activation, selection, modifiers, app profile, hover admission, UNC/read-only/stale-scope controls");Qt.callLater(Qt.quit);phase=7
  }
 }}
 Timer{interval:12000;running:true;onTriggered:{console.error("TERMINAL_LINK_QML_FAIL deadline phase="+root.phase);Qt.exit(1)}}
}
