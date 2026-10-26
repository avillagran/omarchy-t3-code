import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property var calls:[]
 function check(ok,msg){if(!ok){console.error("NIGHTLY_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 QtObject {
  id:fake
  property string selectedEnvironment:"fixture"
  property string selectedThread:"one"
  property var catalog:({canWrite:true})
  property var deviceView:({sessions:{fixture:{data:{settings:{easSupported:true,easDeviceHosts:[]},permissions:{configure:true}}}}})
  property var deviceOperations:({})
  property var selectedInfo:({status:"failed",latestRunId:"limited",snoozedUntil:"2030-01-01T00:00:00Z",limitRecovery:{runId:"limited",resetAt:"2030-01-01T00:00:00Z",autoResume:true,snooze:true}})
  property var transcript:[{role:"error",error:{runId:"limited",class:"usage_limit",resetAt:"2030-01-01T00:00:00Z"}}]
  property var videoStates:({})
  function videoKey(a){return "fixture/"+a.id}
  function requestVideo(a){root.calls=root.calls.concat([{action:"audio-load",id:a.id}])}
  function deviceAction(action,args,environments){root.calls=root.calls.concat([{action:action,args:args,environments:environments}])}
  property var guestShare:({id:"s",guestName:"Guest",messages:[{id:"g",status:"pending",text:"<b>literal message</b>",attachments:[]}]})
  function uuid(){return "00000000-0000-4000-8000-000000000001"}
  function nightlyState(action,env,thread){return action==="guest-create"?{data:{url:"https://example.test/guest",share:{id:"s"}}}:({})}
  function requestNightly(action,args){root.calls=root.calls.concat([{action:action,args:args}])}
  function copyText(text){root.calls=root.calls.concat([{action:"copy",text:text}])}
 }
 FloatingWindow{visible:true;implicitWidth:700;implicitHeight:300;Test.T3GuestShare{id:guest;anchors.fill:parent;anchors.margins:16;bridge:fake}}
 FloatingWindow{visible:true;implicitWidth:620;implicitHeight:650;Test.T3EasHosts{id:eas;anchors.fill:parent;anchors.margins:16;bridge:fake;environments:["fixture"]}}
 FloatingWindow{visible:true;implicitWidth:550;implicitHeight:120;Test.T3AudioAttachment{id:audio;width:500;bridge:fake;attachment:({type:"file",id:"sound",name:"sound.mp3",mimeType:"audio/mpeg"})}}
 FloatingWindow{visible:true;implicitWidth:620;implicitHeight:200;Test.T3UsageRecovery{id:recovery;anchors.fill:parent;anchors.margins:16;bridge:fake}}
 Timer{interval:180;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend
  if(phase===0){
   b.clientSettings={data:{}};check(b.followUpBehavior==="steer","Nightly default steers")
   b.clientSettings={data:{followUpBehavior:"queue"}};check(b.followUpBehavior==="queue","explicit queue survives")
   check(guest.visible&&guest.pending.length===1,"guest approval card renders")
   check(guest.url==="https://example.test/guest","only current share exposes its URL")
   var send=keys.findChild(guest,"guestApprove_g");keys.mouseMove(send,10,10);keys.mouseClick(send,10,10);phase++
  }else if(phase===1){
   check(calls.some(function(c){return c.action==="guest-send"&&c.args.guestMessageId==="g"}),"owner approves exact guest message")
   keys.mouseClick(keys.findChild(guest,"guestCopyLink"),10,10)
   keys.mouseClick(keys.findChild(guest,"guestCloseLink"),10,10)
   fake.guestShare={id:"different",messages:[]};check(!guest.url,"replacement share never reuses a credential link")
   check(calls.some(function(c){return c.action==="copy"}),"copy link action")
   check(calls.some(function(c){return c.action==="guest-close"}),"close link action")
   keys.mouseClick(keys.findChild(eas,"easAddHost"),12,12);phase++
  }else if(phase===2){
   check(!!eas.editing,"EAS host editor opens")
   keys.findChild(eas,"easIdleMinutes").text="40";check(!keys.findChild(eas,"easSave").enabled,"idle limit must be below session limit")
   keys.findChild(eas,"easIdleMinutes").text="10";keys.mouseClick(keys.findChild(eas,"easSave"),12,12)
   keys.mouseClick(keys.findChild(audio,"audioPlay"),10,10);phase++
  }else if(phase===3){
   var saved=calls.find(function(c){return c.action==="eas-upsert"});check(!!saved&&saved.args.host.maxSessionMinutes===30&&saved.environments[0]==="fixture","bounded EAS settings scoped to one environment")
   check(calls.some(function(c){return c.action==="audio-load"&&c.id==="sound"}),"audio loads only after Play")
   check(audio.width<=550&&eas.width<=620,"controls fit rendered surfaces")
   check(recovery.snoozed,"future reset is snoozed")
   recovery.now=Date.parse(recovery.resetAt)+1;check(!recovery.snoozed&&!keys.findChild(recovery,"usageSnoozeUntilReset").enabled,"past reset releases snooze and disables stale scheduling")
   var stamp=new Date(2026,0,1,18,45,0)
   b.clientSettings={data:{timestampFormat:"24-hour"}};check(b.formatTimestamp(stamp).indexOf("18:45")>=0,"24-hour reset time")
   b.clientSettings={data:{timestampFormat:"12-hour"}};check(b.formatTimestamp(stamp).indexOf("6:45")>=0,"12-hour reset time")
   b.threadUndoNotice={shortcut:true,expiresAt:Date.now()+5000};b.releaseThreadUndoShortcut();check(!b.threadUndoNotice.shortcut,"editing releases thread undo shortcut")
   console.log("NIGHTLY_QML_PASS: defaults, explicit preference, scoped guest links, real hover and approval clicks");Qt.quit()
  }
 }}
}
