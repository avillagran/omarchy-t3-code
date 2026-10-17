import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property var fixture:null;property string oldOwner:""
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:panel;function onOpenedChanged(){Test.T3Backend.setPanelVisible(panel.opened)}}
 function check(v,m){if(!v){console.error("BROWSER_FLOW_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;repeat:true;running:true;onTriggered:{
  var b=Test.T3Backend;
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"Native browser route",projectId:"p"}],projects:[{id:"p",key:"fixture/p",environmentId:"fixture",title:"Project",workspaceRoot:"/fixture"}],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.resizePanel(1500,900);panel.open();phase=1;}
  else if(phase===1){if(b.draft.loading||panel.restoringConversation)return;var button=keys.findChild(panel,"t3OpenBrowser");check(button&&button.enabled,"native chat header exposes browser action");button.clicked();phase=2;}
  else if(phase===2){var workspace=keys.findChild(panel,"t3BrowserWorkspace");check(panel.browserShown&&workspace&&b.previewVisible&&b.previewWatch.environment==="fixture"&&b.previewWatch.thread==="one","actual native route mounts scoped metadata and QML workspace: shown="+panel.browserShown+" open="+panel.browserOpen+" inline="+panel.browserInline+" workspace="+workspace+" direct="+panel.browserWorkspaceItem+" preview="+b.previewVisible+" scope="+JSON.stringify(b.previewWatch));check(panel.supportsNativeShortcut("preview.toggle")&&panel.supportsNativeShortcut("preview.focusUrl"),"original preview commands participate in native shortcut routing");oldOwner=b.browserWorkspaceId;panel.resizePanel(800,900);phase=3;}
  else if(phase===3){check(!panel.browserInline&&keys.findChild(panel,"t3BrowserSheet").active,"narrow chat uses native sheet presentation");check(b.previewVisible&&b.browserWorkspaceId&&b.browserWorkspaceId!==oldOwner,"presentation replacement retains current workspace ownership");panel.preferencesOpen=true;phase=4;}
  else if(phase===4){check(!panel.browserShown&&!b.previewVisible&&!b.browserPageMounted,"preferences unmounts browser without replacing native settings UI");panel.preferencesOpen=false;phase=5;}
  else if(phase===5){var workspace=keys.findChild(panel,"t3BrowserWorkspace");check(workspace&&b.previewVisible,"return restores native browser route");keys.findChild(workspace,"t3BrowserClosePanel").clicked();phase=6;}
  else if(phase===6){check(!panel.browserOpen&&!b.previewVisible&&!b.browserPageMounted&&!!keys.findChild(panel,"conversationList"),"closing browser restores original native chat");panel.close();console.log("BROWSER_FLOW_QML_PASS: real header/bridge, responsive native workspace, scoped subscriptions, shortcut admission, preferences and close lifecycle");Qt.quit();}
 }}
 Timer{interval:10000;running:true;onTriggered:{console.error("BROWSER_FLOW_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
