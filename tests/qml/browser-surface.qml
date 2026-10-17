import QtQuick
import Quickshell
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/BrowserSurface.js" as Surface
ShellRoot {
 id:root
 function check(v,m){if(!v){console.error("BROWSER_SURFACE_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 QtObject{
  id:fake
  property bool panelVisible:true
  property bool previewVisible:true
  property bool previewWorkerReady:true
  property bool browserPageMounted:true
  property int browserPageWidth:640
  property int browserPageHeight:400
  property var browserCleanups:({})
  property var browserSurface:Surface.empty()
  property var previewWatch:({environment:"env",thread:"thread",originKey:"origin",connected:true,canOperate:true})
  property var previewState:({serverEpoch:"epoch",snapshot:{tabId:"a",runtime:"desktop",profileId:"work",viewport:{_tag:"fill"},navStatus:{_tag:"Loading",url:"https://example.test"}}})
  property var commands:[]
  property int counter:0
  function uuid(){return "surface-"+(++counter)}
  function send(value){commands=commands.concat([value])}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:640;implicitHeight:400}
 Component{id:viewFactory;Test.T3BrowserView{bridge:Test.T3Backend;visible:false;width:640;height:400}}
 Timer{interval:50;running:true;onTriggered:{
  check(typeof Test.T3Backend.mountBrowserPage==="function"&&!Test.T3Backend.browserPageMounted,"real singleton bridge does not mount a page by default");
  Surface.sync(fake,false);var show=fake.commands[0];check(show.op==="browser-show"&&show.tabId==="a"&&show.originKey==="origin"&&show.serverEpoch==="epoch"&&show.width===640,"scoped native show request");check(!show.url&&!show.profileId&&!show.canOperate,"QML never supplies navigation/profile/permission authority");
  Surface.sync(fake,false);check(fake.commands.length===1,"metadata refresh does not reload an existing page");
  Surface.receive(fake,{kind:"shown",requestId:show.requestId,environment:"env",thread:"thread",tabId:"other",canOperate:true});check(!fake.browserSurface.interactive,"foreign tab receipt ignored");
  Surface.receive(fake,{kind:"shown",requestId:show.requestId,environment:"env",thread:"thread",tabId:"a",canOperate:true});Surface.receive(fake,{kind:"frame",requestId:show.requestId,environment:"env",thread:"thread",tabId:"a",url:"file:///fixture.png",revision:2,width:640,height:400});check(fake.browserSurface.interactive&&!fake.browserSurface.busy&&fake.browserSurface.revision===2,"authorized shown/frame receipts");
  Surface.receive(fake,{kind:"frame",requestId:show.requestId,url:"file:///older.png",revision:1});check(fake.browserSurface.url==="file:///fixture.png","older frame rejected");var count=fake.commands.length;Surface.input(fake,"stale",{type:"press"});check(fake.commands.length===count,"stale input nonce ignored");Surface.input(fake,show.requestId,{type:"press",x:20,y:20});check(fake.commands[fake.commands.length-1].op==="browser-input","current authorized input sent");
  Surface.resize(fake,600,300);check(fake.commands[fake.commands.length-1].event.type==="resize","fill viewport follows native panel size");var bounded=Surface.size(3840,3840);check(bounded.width*bounded.height<=3840*2160,"bounded rendered viewport allocation");
  fake.previewState=Object.assign({},fake.previewState,{snapshot:Object.assign({},fake.previewState.snapshot,{viewport:{_tag:"preset",presetId:"iphone-se",width:375,height:667}})});Surface.sync(fake,false);var next=fake.browserSurface.requestId;check(next!==show.requestId,"native viewport change requests fresh scoped attachment");Surface.receive(fake,{kind:"error",requestId:show.requestId,error:"permission_denied"});check(!fake.browserSurface.error,"old surface error ignored");Surface.receive(fake,{kind:"error",requestId:next,error:"permission_denied"});check(!fake.browserSurface.interactive&&fake.browserSurface.error==="permission_denied","current native error disables page input");
  Surface.sync(fake,true);var cleanupRequest=fake.browserSurface.requestId;
  Surface.receive(fake,{kind:"shown",requestId:cleanupRequest,canOperate:true,profileId:"work"});
  Surface.receive(fake,{kind:"cleanup-started",cleanupId:"clear-one",profileId:"work",environments:["env"]});
  check(!fake.browserSurface.interactive&&!fake.browserSurface.url&&fake.browserSurface.busy,"cleanup removes stale page and blocks input");
  var beforeCleanup=fake.commands.length;Surface.sync(fake,true);Surface.input(fake,cleanupRequest,{type:"press",x:10,y:10});
  check(fake.commands.length===beforeCleanup,"reserved profile cannot reopen or receive input");
  Surface.receive(fake,{kind:"frame",requestId:cleanupRequest,url:"file:///stale.png",revision:100});
  check(!fake.browserSurface.url,"in-flight signed-in frame rejected during cleanup");
  Surface.receive(fake,{kind:"cleanup-ended",cleanupId:"stale-clear",profileId:"work",environments:["env"]});
  check(fake.commands.length===beforeCleanup,"stale cleanup completion cannot release current reservation");
  Surface.receive(fake,{kind:"cleanup-ended",cleanupId:"clear-one",profileId:"work",environments:["env"]});
  check(fake.browserSurface.requestId!==cleanupRequest&&fake.commands[fake.commands.length-1].op==="browser-show","matching cleanup completion freshly attaches native page");
  fake.previewWatch=Object.assign({},fake.previewWatch,{canOperate:false});Surface.sync(fake,false);check(!fake.browserSurface.requestId&&fake.commands[fake.commands.length-1].op==="browser-hide","revoked operation role unmounts local page as original host");
  fake.previewWatch=Object.assign({},fake.previewWatch,{canOperate:true});fake.previewState=Object.assign({},fake.previewState,{snapshot:Object.assign({},fake.previewState.snapshot,{runtime:"server"})});Surface.sync(fake,false);check(fake.browserSurface.error==="server_browser_surface_required"&&!fake.browserSurface.requestId,"server runtime never silently becomes a local guest");
  var older=viewFactory.createObject(win.contentItem);check(older!==null,"native browser view component loads");older.visible=true;var owner1=older.mountId;var replacement=viewFactory.createObject(win.contentItem);replacement.visible=true;var owner2=replacement.mountId;older.visible=false;older.destroy();check(owner1!==owner2&&Test.T3Backend.browserPageMounted&&Test.T3Backend.browserMountId===owner2,"old component disposal cannot unmount replacement surface");replacement.visible=false;replacement.destroy();check(!Test.T3Backend.browserPageMounted,"current component disposal releases surface");
  console.log("BROWSER_SURFACE_QML_PASS: native bridge ownership, authoritative attachment, scoped frames/input, viewport handling, role revocation and server-runtime boundary");Qt.quit();
 }}
 Timer{interval:4000;running:true;onTriggered:{console.error("BROWSER_SURFACE_QML_FAIL timeout");Qt.exit(1)}}
}
