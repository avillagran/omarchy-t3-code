import QtQuick
import Quickshell
import Quickshell.Wayland
import QtQuick.Layouts
import QtTest
import qs.Commons
import "UnderTest/ui/js/NativeUsageLimits.js" as Native
import "UnderTest/ui/js/UsageLimits.js" as Usage
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 function check(v,m){if(!v){console.error("USAGE_LIMITS_QML_FAIL "+m);Qt.exit(1);throw Error(m)}}
 property real now:Date.parse("2026-10-18T12:00:00Z")
 property var sample:({})
 function provider(id,percent,fingerprint){return {instanceId:id,driver:"opencode",enabled:true,installed:true,auth:{status:"authenticated"},usageLimits:{checkedAt:"2026-10-18T12:00:00Z",credentialFingerprint:fingerprint,windows:[{id:"go_rolling",label:"Go · Session",kind:"session",windowDurationMins:300,usedPercent:percent,resetsAt:"2026-10-18T14:00:00Z"},{id:"go_monthly",label:"Go · Monthly",kind:"monthly",usedPercent:percent,resetsAt:"2026-11-01T00:00:00Z"}]}}}
 FloatingWindow {id:window;width:760;height:430;visible:true;color:"#202020"
  Rectangle{id:canvas;anchors.fill:parent;color:"#202020"
  Test.QuotaStrip{id:strip;anchors.left:parent.left;anchors.right:parent.right;anchors.top:parent.top;anchors.margins:14;showHeader:false;providers:[];collapsed:false;notices:["Fixture source unavailable"];onRefreshRequested:root.refreshed=true}
  }
 }
 property real compactHeight:0
 property bool captured:false
 property bool refreshed:false
 property int phase:0
 TestCase{id:keys;when:false}
 Timer{interval:150;repeat:true;running:true;onTriggered:{
  if(root.phase===0){
   var a=provider("one",20,"same-key"),b=provider("two",80,"other-key"),duplicate=provider("dup",20,"same-key")
   sample={environments:[{id:"a",name:"Laptop",connected:true,providers:[a,b],usageLimitSources:[]},{id:"b",name:"Server",connected:true,providers:[duplicate],usageLimitSources:[]}],localSources:[]}
   var result=Usage.present(sample,now)
   check(result.providers.length===1 && result.providers[0].accounts.length===2,"shared credential dedup across hosts")
   check(result.providers[0].windows.length===2,"monthly and session windows retained without weekly")
   check(result.providers[0].windows[0].availablePercent===50,"original pool arithmetic")
   check(result.providers[0].accounts[0].hosts.indexOf("Laptop")>=0,"account origin retained")
   var unknown=provider("future",10,"future-key");unknown.driver="future-provider";sample.environments[0].providers.push(unknown)
   result=Usage.present(sample,now);check(result.providers.length===2,"future provider is data driven")
   strip.providers=result.providers;phase=1
  }else if(phase===1){
   check(strip.visible && strip.visibleProviders.length===2,"native quota rows visible")
   check(!!keys.findChild(strip,"quotaWindow_opencode_go_monthly"),"actual monthly QML row")
   var marker=keys.findChild(strip,"quotaOptimal_opencode_go_rolling")
   check(marker && marker.visible && Math.abs(marker.expected-40)<0.001,"optimal remaining marker uses native clock (two of five hours remain)")
   var original=marker.parent.modelData
   var fill=keys.findChild(strip,"quotaFill_opencode_go_rolling")
   marker.parent.modelData=Object.assign({},original,{availablePercent:marker.expected-0.1})
   check(marker.parent.overBudget && Qt.colorEqual(fill.color,Color.urgent),"below optimum turns urgent even inside native five-point pace tolerance")
   marker.parent.modelData=Object.assign({},original,{availablePercent:marker.expected})
   check(!marker.parent.overBudget && Qt.colorEqual(fill.color,Color.accent),"at optimum retains healthy color")
   marker.parent.modelData=original
   check(Math.abs(marker.x-(marker.parent.width*0.4-marker.width/2))<1,"marker position follows available-percent axis")
   check(!keys.findChild(strip,"quotaOptimal_opencode_go_monthly").visible,"unknown monthly duration has no invented optimal marker")
   var warning=keys.findChild(strip,"quotaWarning")
   check(warning && warning.opacity===1 && warning.tooltipText==="Fixture source unavailable","failure details live only in warning tooltip")
   check(strip.height<110,"compact provider rows use less than110 logical pixels for four windows: "+strip.height)
   compactHeight=strip.height
   check(canvas.grabToImage(function(r){check(r.saveToFile("/tmp/ot3-usage-limits-compact.png"),"save capture");root.captured=true}),"request capture");phase=2
  }else if(phase===2){
   if(!root.captured)return
   var button=keys.findChild(strip,"quotaAccounts_opencode");keys.mouseClick(button,button.width/2,button.height/2)
   keys.mouseClick(keys.findChild(strip,"quotaRefresh"),10,10);check(refreshed,"refresh emits explicit request");phase=3
  }else if(phase===3){
   check(strip.height>compactHeight,"account details retained on demand")
   strip.providers=[];phase=4
  }else{
   check(strip.visible && strip.height<=24,"failed-only result uses one compact warning line")
   check(keys.findChild(strip,"quotaUnavailable").tooltipText==="Fixture source unavailable","failed-only tooltip keeps diagnostic")
   console.log("USAGE_LIMITS_QML_PASS: native grouping, compact geometry, optimal remaining markers, tooltip-only errors, expandable account detail and refresh");Qt.quit()
  }
 }}
}
