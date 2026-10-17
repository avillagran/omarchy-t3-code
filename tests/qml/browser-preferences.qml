import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/NativeBrowserPreferences.js" as Original
ShellRoot{
 id:root;property int phase:0
 function check(v,m){if(!v){console.error("BROWSER_PREFERENCES_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 TestCase{id:keys;parent:win.contentItem;when:false}
 FloatingWindow{id:win;visible:true;implicitWidth:1000;implicitHeight:900;Rectangle{anchors.fill:parent;color:"#101214";Test.T3BrowserDefaults{id:page;anchors.left:parent.left;anchors.right:parent.right;bridge:Test.T3Backend}}}
 Timer{interval:160;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,d=b.clientSettings.data,n=Original.nativeBrowser;if(b.clientSettings.busy)return;
  if(phase===0){if(!d.revision||!d.browserDefaultViewport)return;check(page.writable&&d.browserDefaultViewport._tag==="fill"&&d.browserAutoShowFloatingPreview,"native client defaults through actual worker");check(n.PREVIEW_VIEWPORT_PRESETS.length===17&&n.PREVIEW_ZOOM_LEVELS.length===17,"exact pinned native catalogs");var profiles=n.resolveBrowserProfiles([{id:"default",name:"Spoof",kind:"persistent"},{id:"work",name:"Work",kind:"incognito"},{id:"work",name:"Duplicate",kind:"persistent"}]);check(profiles.length===3&&profiles[0].name==="Default"&&profiles[2].kind==="persistent","original built-ins/duplicate/persistence normalization");page.selectViewport("ipad-air");check(b.clientSettings.busy&&!page.writable,"save captures canonical file revision and serializes edits");phase=1}
  else if(phase===1){check(!b.clientSettings.error&&d.browserDefaultViewport.presetId==="ipad-air"&&d.browserDefaultViewport.width===820,"real native preset persisted");page.rotate();phase=2}
  else if(phase===2){check(d.browserDefaultViewport.width===1180&&d.browserDefaultViewport.height===820&&d.browserDefaultViewport.presetId==="ipad-air","original rotation preserves preset identity");var id=b.clientSettings.requestId;page.commit("width","239");check(b.clientSettings.requestId===id,"minimum rejected before file mutation");var input=keys.findChild(page,"browserViewport_width");input.text="2560";check(b.clientSettings.requestId===id,"typing does not save intermediate dimensions");input.editingFinished();phase=3}
  else if(phase===3){check(d.browserDefaultViewport._tag==="freeform"&&d.browserDefaultViewport.width===2560,"dimension committed on blur as native freeform");var id=b.clientSettings.requestId;page.commit("height","3840");check(b.clientSettings.requestId===id,"maximum pixel area rejected");keys.findChild(page,"preferenceField_browserDefaultZoomFactor").changed(1.25);phase=4}
  else if(phase===4){check(d.browserDefaultZoomFactor===1.25&&!b.clientSettings.error,"native zoom setting saved by shared field");keys.findChild(page,"preferenceField_browserRecordingShowKeyPresses").changed(true);phase=5}
  else if(phase===5){check(d.browserRecordingShowKeyPresses&&d.browserDefaultZoomFactor===1.25&&d.browserDefaultViewport.width===2560,"incremental changes preserve other native defaults");b.requestClientSettings();phase=6}
  else if(phase===6){check(d.browserRecordingShowKeyPresses&&d.browserDefaultZoomFactor===1.25&&d.browserDefaultViewport.width===2560,"native file readback remains authoritative");page.selectViewport("fill");phase=7}
  else if(phase===7){check(d.browserDefaultViewport._tag==="fill"&&!b.clientSettings.error,"native viewport reset persisted");console.log("BROWSER_PREFERENCES_QML_PASS: original helpers, actual isolated native client file, preset/rotation/dimension commit, bounds, zoom/recording and readback/reset");Qt.quit()}
 }}
 Timer{interval:9500;running:true;onTriggered:{console.error("BROWSER_PREFERENCES_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
