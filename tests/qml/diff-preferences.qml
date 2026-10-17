import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property bool whitespace:true
 TestCase{id:keys;when:false}
 function check(ok,message){if(!ok){console.error("DIFF_PREFERENCES_QML_FAIL "+message);Qt.exit(1)}}
 Test.T3DiffReview {
  id:review;width:900;height:620;summary:({});rendererBridge:Test.T3Backend
  diffLayout:Test.T3Backend.diffPreferences.layout;wordWrap:Test.T3Backend.diffPreferences.wordWrap;ignoreWhitespace:root.whitespace
  view:({loading:false,error:"",data:{files:[{path:"a.js",oldPath:"a.js",rows:[],splitRows:[]}],additions:0,deletions:0}})
  onWhitespaceChanged:function(value){root.whitespace=value}
 }
 Timer{interval:160;repeat:true;running:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded||b.clientSettings.busy||!b.clientSettings.data.revision)return;check(review.diffLayout==="stacked"&&review.wordWrap&&review.ignoreWhitespace,"native decoded defaults");keys.findChild(review,"t3DiffLayout").clicked();check(b.diffPreferenceUpdate.stage==="read"&&review.diffLayout==="split","user choice reads fresh native revision and updates optimistically");phase=1}
  else if(phase===1){if(b.clientSettings.busy||b.diffPreferenceUpdate.stage)return;check(!b.diffPreferenceUpdate.error&&b.clientSettings.data.diffLayout==="split","layout persisted through production local writer");var rev=b.clientSettings.data.revision;b.send({op:"client-settings",action:"update",requestId:"fixture-native-external",revision:rev,patch:{planModeEnabled:true}});keys.findChild(review,"t3DiffLayout").clicked();phase=2}
  else if(phase===2){if(b.clientSettings.busy||b.diffPreferenceUpdate.stage)return;check(!b.diffPreferenceUpdate.error&&b.clientSettings.data.diffLayout==="stacked"&&b.clientSettings.data.planModeEnabled===true,"fresh read preserves another native client change");keys.findChild(review,"t3DiffWrap").clicked();keys.findChild(review,"t3DiffWhitespace").clicked();check(!review.wordWrap&&!review.ignoreWhitespace&&!b.clientSettings.busy,"wrap and whitespace remain view-local like native T3");b.requestClientSettings();phase=3}
  else if(phase===3){if(b.clientSettings.busy)return;check(b.clientSettings.data.wordWrap===true&&b.clientSettings.data.diffIgnoreWhitespace===true,"view-local toggles do not rewrite native defaults");b.setDiffLayout("split");var id=b.clientSettings.requestId;b.action({op:"client-settings",requestId:id,error:"client_settings_changed_reload"});check(!b.clientSettings.busy&&review.preferenceError==="client_settings_changed_reload","read failure remains visible without an implicit write");phase=4}
  else if(phase===4){keys.findChild(review,"t3DiffPreferenceRetry").clicked();phase=5}
  else if(phase===5){if(b.clientSettings.busy||b.diffPreferenceUpdate.stage)return;check(!review.preferenceError&&b.clientSettings.data.diffLayout==="split"&&b.clientSettings.data.planModeEnabled===true,"explicit retry re-reads then persists desired layout");console.log("DIFF_PREFERENCES_QML_PASS: actual isolated native file read/write, decoded defaults, fresh-revision persistence, external change preservation, local wrap/whitespace and failure/retry");Qt.quit()}
 }}
 Timer{interval:9000;running:true;onTriggered:{console.error("DIFF_PREFERENCES_QML_FAIL timeout phase="+root.phase);Qt.exit(1)}}
}
