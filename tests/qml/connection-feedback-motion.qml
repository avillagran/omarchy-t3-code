import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int layoutChecks:0
 property var fixture:null
 property real frozen:0
 property real barFrozen:0
 property bool captured:false
 function check(ok,msg){if(!ok){console.error("T3_CONNECTION_MOTION_QML_FAIL: "+msg);Qt.exit(1)}}
 function capture(item,name){captured=false;item.grabToImage(function(r){check(r.saveToFile("/tmp/ot3-"+name+".png"),"save capture");root.captured=true})}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Test.T3Tick{id:probe;running:true}
 Test.T3BarIndicator{id:bar;counts:({working:1});motion:Test.T3Motion.enabled}
 Test.T3Reveal{id:reveal;shown:true;Rectangle{implicitWidth:40;implicitHeight:40}}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   if(!b.viewStateLoaded)return
   b.rememberView({animationsEnabled:true,starfieldMode:"full",transitionStyle:"fold",threadMoveEffect:"arc"})
   fixture={threads:[],projects:[],environments:[{id:"local",name:"T3 Code",state:"ready"},{id:"node",name:"Node",state:"ssh_unavailable"}],working:0,pending:0}
   b.state=fixture;b.stale=true;b.diskSpace={low:false};panel.open();phase=1
  }else if(phase===1){
   var notice=keys.findChild(panel,"t3ConnectionNotice")
   check(notice&&notice.visible,"failure is visible without opening hosts")
   check(keys.findChild(notice,"connectionReason_node").text.indexOf("SSH")>=0,"reason describes SSH")
   check(keys.findChild(notice,"connectionCode_node").text==="ssh_unavailable","exact code remains available")
   b.connectionSettings=Object.assign({},b.connectionSettings,{busy:false})
   keys.findChild(notice,"connectionRetry_node").clicked()
   check(b.connectionSettingsCommand.action==="reconnect"&&b.connectionSettingsCommand.environmentId==="node","retry addresses the failed environment")
   check(notice.visible&&keys.findChild(notice,"connectionCode_node").text==="ssh_unavailable","retry does not conceal the cause")
   capture(notice,"connection-failure-fixture");phase=2
  }else if(phase===2){
   if(!captured)return
   fixture=Object.assign({},fixture,{environments:[fixture.environments[0],{id:"node",name:"Node",state:"ready"}]});b.state=fixture;b.stale=false;b.error=""
   check(!keys.findChild(panel,"t3ConnectionNotice").visible,"recovery clears notice")
   panel.openPreferencesPage("appearance");phase=3
  }else if(phase===3){
   var view=panel.preferencesView,master=keys.findChild(view,"preferenceAnimationsEnabled"),choice=keys.findChild(view,"preferenceChoice_starfieldMode")
   check(master&&master.visible&&master.checked,"master defaults on at the beginning of Appearance")
   var ordered=master.mapToItem(view,0,0).y<keys.findChild(view,"preferenceUsage").mapToItem(view,0,0).y
   if(!ordered&&++layoutChecks<10)return
   check(ordered,"master precedes other appearance options after layout")
   check(choice.enabled,"animation options enabled initially")
   master.checked=false;master.toggled();phase=4
  }else if(phase===4){
   var view=panel.preferencesView
   b.settingsResult={environment:"local",busy:false,error:"",fields:[],writable:true}
   check(!Test.T3Motion.enabled&&Test.T3Motion.style==="instant","motion and transitions disabled globally")
   check(!Test.T3Ticker.enabled&&!probe.ticket,"decorative clock has no active request")
   check(!panel.starfield.animated&&panel.rowMoveEffect==="off","sky and row travel disabled")
   check(!keys.findChild(view,"preferenceChoice_starfieldMode").enabled&&!keys.findChild(view,"preferenceFpsMeter").enabled,"animation controls disabled")
   check(b.viewState.starfieldMode==="full"&&b.viewState.transitionStyle==="fold","saved choices preserved")
   reveal.shown=false;check(reveal.progress===0&&!reveal.moving,"show/hide snaps to destination")
   frozen=Test.T3Ticker.time;barFrozen=bar.t;capture(view,"animations-off-fixture");phase=5
  }else if(phase===5){
   if(!captured)return
   check(Test.T3Ticker.time===frozen&&bar.t===barFrozen,"sky/spinner clock and bar remain still")
   var master=keys.findChild(panel.preferencesView,"preferenceAnimationsEnabled");master.checked=true;master.toggled();phase=6
  }else if(phase===6){
   check(Test.T3Motion.enabled&&Test.T3Motion.style==="fold"&&probe.ticket>0,"reenable restores motion and style")
   check(keys.findChild(panel.preferencesView,"preferenceChoice_starfieldMode").enabled,"options usable again")
   // Unknown codes must remain explicit, and simultaneous failures stay listed.
   fixture=Object.assign({},fixture,{environments:[{id:"node",name:"Node",state:"unrecognized_transport"},{id:"local",name:"T3 Code",state:"t3_not_running"}]});b.state=fixture;b.stale=true
   panel.preferencesOpen=false;phase=7
  }else if(phase===7){
   var notice=keys.findChild(panel,"t3ConnectionNotice")
   check(notice.failures.length===2,"all failures retained")
   check(keys.findChild(notice,"connectionReason_node").text.indexOf("unrecognized_transport")>=0,"unknown code is never hidden")
   console.log("T3_CONNECTION_MOTION_QML_PASS: visible reason, retry, recovery, unknown errors; master toggle, remembered options, frozen clocks and restore")
   Qt.quit()
  }
 }}
}
