import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
import "UnderTest/ui/js/TimelineModel.js" as Timeline
ShellRoot {
 id:root
 property int phase:0
 property var fixture:null
 property real before:0
 property bool captured:false
 property var marker:null
 function check(ok,msg){if(!ok){console.error("TIMELINE_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:200;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,c=panel.conversationView,m=keys.findChild(panel,"t3TimelineMinimap"),rail=keys.findChild(panel,"t3TimelineRail")
  if(phase===0){
   if(!b.viewStateLoaded)return
   check(Timeline.derive([{id:"a",role:"assistant"},{id:"u",role:"user",text:" raw\n prompt "},{id:"a1",role:"assistant",text:"draft"},{id:"w",role:"activity"},{id:"a2",role:"assistant",text:"final"},{id:"c",role:"changes"}])[0].assistantText==="final","last assistant in turn, activity/changes excluded")
   check(Timeline.indexAt(100,100,3)===2&&Timeline.indexAt(-100,100,3)===0,"pointer clamps")
   b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"},{key:"fixture/two",id:"two",environmentId:"fixture",title:"Two"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one")
   var rows=[];for(var i=0;i<30;i++){rows.push({id:"u"+i,role:"user",text:"Prompt "+i+"\n  next line"});rows.push({id:"a"+i,role:"assistant",text:("Response "+i+"\n").repeat(16)})}b.transcript=rows;panel.open();phase=1
  }else if(phase===1){
   if(panel.restoringConversation||b.draft.loading)return
   b.edit("Keep my draft");check(m.visible&&m.items.length===30,"one marker per user prompt");check(m.items[0].userText==="Prompt 0\n  next line","unmodified prompt source");check(m.items[0].assistantText.indexOf("Response 0")===0,"paired response");before=c.contentY;keys.mouseMove(rail,4,rail.height-1);phase=2
  }else if(phase===2){
   check(Math.abs(c.contentY-before)<2,"hover does not move chat");check(m.hoverIndex===29&&keys.findChild(panel,"t3TimelinePreview").visible,"hover preview");c.parent.grabToImage(function(image){root.captured=image.saveToFile("/tmp/t3-timeline-preview.png")});phase=20
  }else if(phase===20){
   if(!captured)return;keys.mouseClick(rail,4,1);phase=3
  }else if(phase===3){
   if(panel.restoringConversation)return
   var row=c.itemAtIndex(0);check(row&&Math.abs(row.y-c.contentY)<2,"click jumps to exact prompt");check(!panel.followingTail,"jump detaches live follow");check(m.currentIndex===0,"visible current turn");check(!keys.findChild(panel,"t3TimelinePrevious").enabled,"previous disabled at first");keys.mouseClick(keys.findChild(panel,"t3TimelineNext"),14,14);phase=4
  }else if(phase===4){
   if(panel.restoringConversation)return
   check(m.currentIndex===1,"next button navigates to next turn");rail.forceActiveFocus();keys.keyClick(Qt.Key_End);check(m.hoverIndex===29,"keyboard End selects last marker");keys.keyClick(Qt.Key_Return);phase=5
  }else if(phase===5){
   if(panel.restoringConversation)return
   check(m.currentIndex===29,"keyboard activation jumps last turn");keys.keyClick(Qt.Key_Home);keys.keyClick(Qt.Key_Return);phase=6
  }else if(phase===6){
   if(panel.restoringConversation)return
   check(m.currentIndex===0,"keyboard Home activates first turn");marker=keys.findChild(panel,"t3TimelineMarker0");var rows=b.transcript.slice();rows[rows.length-1]=Object.assign({},rows[rows.length-1],{text:"Live final response update"});b.transcript=rows;before=c.contentY;phase=7
  }else if(phase===7){
   if(panel.restoringConversation)return
   check(Math.abs(c.contentY-before)<3,"live update preserves manual reading");check(marker===keys.findChild(panel,"t3TimelineMarker0"),"stream updates reuse marker delegates");check(b.draft.text==="Keep my draft","navigation retains draft");keys.mouseWheel(rail,4,rail.height/2,0,-120);check(!panel.followingTail,"wheel routes to manual chat scroll");panel.pullRequestsOpen=true;phase=8
  }else if(phase===8){
   check(!m.active&&!m.visible,"PR hides minimap");panel.pullRequestsOpen=false;panel.expandComposer(false);phase=9
  }else if(phase===9){
   check(!m.active&&!m.visible,"expanded editor hides minimap");panel.expandComposer(false);b.selectThread("fixture","two");b.transcript=[{id:"only",role:"user",text:"Only one"}];phase=10
  }else if(phase===10){
   check(!m.visible&&m.hoverIndex===-1,"single turn hidden and thread preview reset");panel.close();console.log("TIMELINE_QML_PASS: native turn grouping, hover, exact click, arrows, keyboard, live updates, wheel, drafts and visibility");Qt.quit()
  }
 }}
}
