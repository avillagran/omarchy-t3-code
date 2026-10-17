import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/NativeLicenses.js" as Original
ShellRoot{
 id:root;property int phase:0;property bool captured:false
 function check(ok,msg){if(!ok){console.error("LICENSES_QML_FAIL "+msg);Qt.exit(1);throw new Error(msg)}}
 function entry(name,version,license,bundles){var e={kind:"package",name:name,version:version,license:license,bundles:bundles,sourceUrl:null};e.entryKey=Original.nativeLicenses.thirdPartyLicenseEntryKey(e);return e}
 QtObject{id:fixture
  property var licenseView:({busy:false,error:"",data:null,requestId:""})
  property var licenseNotice:({busy:false,error:"",entryKey:"",manifestId:"",noticeText:""})
  property int reads:0;property int notices:0;property int forceReads:0
  function requestLicenses(force){reads++;if(force)forceReads++}
  function requestLicenseNotice(key){notices++;licenseNotice={busy:true,error:"",entryKey:key,manifestId:"fixture",noticeText:""}}
  function dismissLicenseNotice(){licenseNotice={busy:false,error:"",entryKey:"",manifestId:"",noticeText:""}}
 }
 TestCase{id:keys;parent:win.contentItem;when:false}
 FloatingWindow{id:win;visible:true;implicitWidth:950;implicitHeight:760;Test.T3Licenses{id:page;anchors.fill:parent;anchors.margins:24;bridge:fixture}}
 Timer{interval:160;repeat:true;running:true;onTriggered:{
  if(phase===0){check(fixture.reads===1,"application manifest read on mount");fixture.licenseView={busy:false,error:"",data:{manifestId:"fixture",entries:[entry("Alpha library","1.2.3","MIT",["desktop"]),entry("Beta renderer",null,"Apache-2.0",["web","assets"]),entry("Example <script> & ñ /",null,"MIT",["desktop"])]}};phase=1}
  else if(phase===1){check(page.filtered.length===3,"native manifest rows");page.query="alpha MIT desktop";check(page.filtered.length===1,"original all-term name/license/bundle search");page.query="beta 1.2.3";check(!page.filtered.length,"terms must match same entry");page.query="";keys.findChild(page,"licenseRow_0").clicked();check(fixture.notices===1&&page.openEntryKey===page.entries[0].entryKey,"row uses original stable key/lazy notice");fixture.licenseNotice={busy:false,error:"",manifestId:"fixture",entryKey:page.openEntryKey,noticeText:"<script>exact plain license</script>\nCopyright retained"};phase=2}
  else if(phase===2){var text=keys.findChild(page,"licenseNotice_0");check(text&&text.visible&&text.text.indexOf("<script>")===0&&text.readOnly,"exact selectable plain-text notice");keys.findChild(page,"licenseRow_1").clicked();check(page.openEntryKey===page.entries[1].entryKey&&!keys.findChild(page,"licenseNotice_0"),"only one lazy row stays open");fixture.licenseNotice={busy:false,error:"",manifestId:"fixture",entryKey:page.openEntryKey,noticeText:"Apache License\nExact terms"};phase=3}
  else if(phase===3){check(Original.nativeLicenses.formatLicenseBundles(["web","assets","custom"])==="Web, Assets, custom","original bundle formatting");check(page.entries[2].entryKey==="%5B%22package%22%2C%22Example%20%3Cscript%3E%20%26%20%C3%B1%20%2F%22%2Cnull%5D","original Unicode identity matches Rust decoder");keys.findChild(page,"licenseRow_1").clicked();check(!page.openEntryKey&&!fixture.licenseNotice.entryKey,"close clears pending notice");fixture.licenseView={busy:false,error:"invalid_license_manifest",data:null};phase=4}
  else if(phase===4){keys.findChild(page,"licenseRetry").clicked();check(fixture.forceReads===1,"explicit retry after error");fixture.licenseView={busy:false,error:"",data:{manifestId:"fixture",entries:[entry("Alpha library","1.2.3","MIT",["desktop"]),entry("Beta renderer",null,"Apache-2.0",["web","assets"])]}};page.grabToImage(function(r){root.captured=r.saveToFile("/tmp/ot3-licenses-fixture.png")});phase=5}
  else if(phase===5){if(!captured)return;console.log("LICENSES_QML_PASS: original search/key/bundles, counts, one lazy expanded row, exact plain text, collapse and retry");Qt.quit()}
 }}
 Timer{interval:9000;running:true;onTriggered:{console.error("LICENSES_QML_FAIL timeout phase="+phase);Qt.exit(1)}}
}
