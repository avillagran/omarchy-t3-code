import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property string settingsRequest:"";property string priorNoticeRequest:"";property bool captured:false
 function check(ok,msg){if(!ok){console.error("LICENSES_PREFERENCES_QML_FAIL "+msg);Qt.exit(1);throw new Error(msg)}}
 TestCase{id:keys;parent:win.contentItem;when:false}
 FloatingWindow{id:win;visible:true;implicitWidth:1200;implicitHeight:900;Test.T3Preferences{id:prefs;anchors.fill:parent;bridge:Test.T3Backend}}
 Timer{interval:180;repeat:true;running:true;onTriggered:{var b=Test.T3Backend,page=keys.findChild(prefs,"t3Licenses")
  if(phase===0){if(!b.viewStateLoaded)return;settingsRequest=b.settingsResult.requestId;prefs.query="Licencias de código abierto";phase=1}
  else if(phase===1){var link=keys.findChild(prefs,"preferencesOpenLicenses");check(link&&link.visible,"General diagnostics link participates in localized settings search");link.clicked();check(prefs.page==="licenses"&&!prefs.query,"native General drill-down navigation");check(b.settingsResult.requestId===settingsRequest,"application licenses never request environment settings");phase=2}
  else if(phase===2){if(!page||b.licenseView.busy)return;check(!b.licenseView.error&&page.entries.length>200,"actual compiled app manifest through IPC without any native account");check(!keys.findChild(prefs,"preferencesEnvironment").visible,"application manifest has no misleading environment selector");check(!keys.findChild(prefs,"preferencesTab_licenses"),"native drill-down does not invent a top-level tab");page.toggle(page.entries[0]);phase=3}
  else if(phase===3){if(b.licenseNotice.busy)return;check(!b.licenseNotice.error&&b.licenseNotice.noticeText.indexOf("Apache License")>=0,"actual exact lazy notice from compiled binary");page.toggle(page.entries[1]);priorNoticeRequest=b.licenseNotice.requestId;b.action({op:"licenses-notice",requestId:priorNoticeRequest,result:{manifestId:"wrong",entryKey:page.openEntryKey,noticeText:"WRONG"}});check(b.licenseNotice.error==="license_manifest_changed"&&!b.licenseNotice.noticeText,"mismatched manifest cannot install notice");b.requestLicenseNotice(page.openEntryKey);phase=4}
  else if(phase===4){if(b.licenseNotice.busy)return;check(!b.licenseNotice.error&&!!b.licenseNotice.noticeText,"explicit fresh retry succeeds");var before=b.licenseNotice.noticeText;b.action({op:"licenses-notice",requestId:priorNoticeRequest,result:{manifestId:b.licenseNotice.manifestId,entryKey:page.openEntryKey,noticeText:"STALE"}});check(b.licenseNotice.noticeText===before,"late stale receipt ignored");prefs.grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-licenses-preferences.png")});phase=5}
  else if(phase===5){if(!captured)return;prefs.page="general";phase=6}
  else if(phase===6){check(!keys.findChild(prefs,"t3Licenses")&&!b.licenseNotice.entryKey,"leaving page disposes lazy notice");console.log("LICENSES_PREFERENCES_QML_PASS: localized General link, native drill-down, no environment RPC, actual compiled application manifest and lazy Apache notice, mismatched/stale receipts, explicit retry and page disposal");Qt.quit()}
 }}
 Timer{interval:10000;running:true;onTriggered:{console.error("LICENSES_PREFERENCES_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
