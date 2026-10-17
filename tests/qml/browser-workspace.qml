import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var actions:[];property var controls:[];property bool captured:false
 function check(v,m){if(!v){console.error("BROWSER_WORKSPACE_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 function tab(id,idle){return {threadId:"thread",tabId:id,navStatus:idle?{_tag:"Idle"}:{_tag:"Success",url:"https://example.test/"+id,title:"Tooltip preview"},profileId:"work",runtime:"desktop",canGoBack:!idle,canGoForward:!idle,viewport:{_tag:"fill"}}}
 QtObject {
  id:b
  property string selectedEnvironment:"env"
  property string selectedThread:"thread"
  property var clientSettings:({data:{browserDefaultProfileId:"work",browserProfiles:[{id:"work",name:"Work",kind:"persistent"}],browserDefaultViewport:{_tag:"fill"}}})
  property var previewWatch:({connected:true,canOperate:true,busy:false,error:""})
  property var previewState:({listLoaded:true,serverEpoch:"epoch",activeTabId:"a",snapshot:root.tab("a",true),sessions:{a:root.tab("a",true),b:root.tab("b",false)},recentlySeenUrls:[]})
  property var previewOperations:({})
  property var browserSurface:({interactive:true,busy:false,error:"",url:"",zoomFactor:1})
  property string browserMountId:""
  property string workspaceOwner:""
  property int serial:0
  function mountBrowserWorkspace(){workspaceOwner="workspace-"+(++serial);return workspaceOwner}
  function unmountBrowserWorkspace(id){if(id===workspaceOwner)workspaceOwner=""}
  function mountBrowserPage(w,h){browserMountId="page-"+(++serial);return browserMountId}
  function unmountBrowserPage(id){if(id===browserMountId)browserMountId=""}
  function resizeBrowserPage(w,h,owner){}
  function browserPageInput(id,event,owner){}
  function previewAction(action,input){root.actions=root.actions.concat([{action:action,input:input}]);return true}
  function browserControl(action){root.controls=root.controls.concat([action])}
  function activatePreviewTab(id){previewState=Object.assign({},previewState,{activeTabId:id,snapshot:previewState.sessions[id]})}
  function openReference(url){root.controls=root.controls.concat([url])}
  function acknowledgePreviewOperation(){previewOperations={}}
 }
 FloatingWindow{id:win;visible:true;implicitWidth:620;implicitHeight:660;Rectangle{anchors.fill:parent;color:"#101010";Test.T3BrowserWorkspace{id:workspace;anchors.fill:parent;bridge:b}}}
 TestCase{id:keys;parent:win.contentItem;when:false}
 Timer{interval:160;repeat:true;running:true;onTriggered:{
  if(phase===0){workspace.grabToImage(function(image){root.captured=image.saveToFile("/tmp/ot3-browser-workspace.png")});phase=1;return;}
  if(phase===1){if(!captured)return;check(workspace.tabs.length===2&&workspace.defaultProfile==="work"&&workspace.profileName==="Work","native profile/tab model");check(!b.browserMountId,"idle tab displays native empty state without a blank guest");check(keys.findChild(workspace,"t3BrowserTabs")&&keys.findChild(workspace,"t3BrowserNewTab").enabled,"native tab row and actions render");check(keys.findChild(workspace,"t3BrowserTabRow").height===keys.findChild(workspace,"t3BrowserChromeRow").height&&keys.findChild(workspace,"t3BrowserTabRow").height<80,"native chrome rows stay fixed while page body expands");workspace.focusUrl();keys.wait(1);var input=keys.findChild(workspace,"t3BrowserAddress");input.selectAll();"omarchy tooltip design".split("").forEach(function(c){keys.keyClick(c)});var changed=Object.assign({},b.previewState.snapshot,{navStatus:{_tag:"Success",url:"https://changed.test/",title:"Changed"}});b.previewState=Object.assign({},b.previewState,{snapshot:changed});check(input.text==="omarchy tooltip design","background navigation cannot replace address draft: text="+input.text+" draft="+workspace.addressDraft+" focus="+input.activeFocus);keys.keyClick(Qt.Key_Escape);check(input.text==="https://changed.test/","Escape restores native current URL");workspace.focusUrl();keys.wait(1);input.selectAll();"omarchy tooltip design".split("").forEach(function(c){keys.keyClick(c)});keys.keyClick(Qt.Key_Return);check(actions[0].action==="navigate"&&actions[0].input.address==="omarchy tooltip design"&&actions[0].input.tabId==="a","Enter submits raw address to original backend resolver");keys.mouseClick(keys.findChild(workspace,"t3BrowserNewTab"),10,10,Qt.LeftButton);check(actions[1].action==="open"&&actions[1].input.profileId==="work"&&actions[1].input.viewport._tag==="fill","new tab uses native defaults");keys.mouseClick(keys.findChild(workspace,"t3BrowserTab_b"),20,10,Qt.LeftButton);phase=2;}
  else if(phase===2){check(b.previewState.activeTabId==="b"&&!!b.browserMountId,"tab selection mounts the page item");keys.findChild(workspace,"t3BrowserBack").clicked();keys.findChild(workspace,"t3BrowserForward").clicked();check(controls[0]==="back"&&controls[1]==="forward","native history controls routed");keys.findChild(workspace,"t3BrowserRefresh").clicked();check(actions[2].action==="refresh"&&actions[2].input.tabId==="b","refresh targets selected tab");keys.findChild(workspace,"t3BrowserCloseTab_b").clicked();check(actions[3].action==="close"&&actions[3].input.tabId==="b","close targets its own tab");var menu=keys.findChild(workspace,"t3BrowserMoreMenu");check(menu&&menu.count>=5,"native more menu exists");menu.itemAt(0).triggered();check(controls[2]==="hard-reload","hard reload is a scoped native control");b.previewOperations={};b.previewOperations[JSON.stringify(["env","thread"])]=({state:"uncertain",error:"preview_command_uncertain"});b.previewOperations=Object.assign({},b.previewOperations);check(!workspace.writable&&!keys.findChild(workspace,"t3BrowserNewTab").enabled,"unknown command outcome blocks duplicate navigation");check(captured,"native workspace screenshot saved");b.previewOperations={};b.previewState=Object.assign({},b.previewState,{activeTabId:"a",snapshot:root.tab("a",true),recentlySeenUrls:["https://example.test/recent","http://localhost:5173/preview"]});phase=3;}
  else if(phase===3){var recent=keys.findChild(workspace,"t3BrowserRecent_0");check(recent&&recent.width>0&&recent.width<=workspace.width&&keys.findChild(workspace,"t3BrowserRecentUrls").visible,"recent URLs use bounded native cards");keys.mouseClick(recent,20,10,Qt.LeftButton);check(actions[4].action==="navigate"&&actions[4].input.address==="https://example.test/recent","recent URL opens in the selected idle tab");keys.findChild(workspace,"t3BrowserProfilesMenu").itemAt(1).triggered();check(actions[5].action==="open"&&actions[5].input.profileId==="incognito","explicit native Incognito profile choice");console.log("BROWSER_WORKSPACE_QML_PASS: native controls/layout, idle state, profile defaults, address draft/Enter/Escape, tab selection/history/refresh/close, menu and uncertain-operation admission");Qt.quit();}
 }}
 Timer{interval:6500;running:true;onTriggered:{console.error("BROWSER_WORKSPACE_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
