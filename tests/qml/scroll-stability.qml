import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property int phase:0
 property int tick:0
 property var fixture:null
 property var anchor:null
 property var retained:null
 property real maxDrift:0
 property bool captured:false
 function check(ok,msg){if(!ok){console.error("T3_SCROLL_STABILITY_QML_FAIL: "+msg);Qt.exit(1)}}
 function update(rows){var b=Test.T3Backend;b.messagesAboutToChange();b.transcript=rows;b.messagesUpdated()}
 function drift(){var c=panel.conversationView;var i=Test.T3Backend.transcript.findIndex(function(r){return r.id===anchor.id});var item=c.itemAtIndex(i);check(!!item,"anchor remains instantiated");return Math.abs(c.contentY-item.y-anchor.offset)}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:90;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One",working:true,startedAt:Date.now()-65000}],projects:[],environments:[],working:1};fixture=b.state;b.selectThread("fixture","one");var rows=[];for(var i=0;i<65;i++)rows.push({id:"m"+i,role:"assistant",text:("Message "+i+" varied-height line\n").repeat(1+i%5)});rows[22]={id:"m22",role:"activity",activity:{active:true,command:"rg",count:2,entries:[{command:"cat",active:false},{command:"rg",active:true}]}};b.transcript=rows;b.activity=[{command:"rg",active:true}];panel.open();phase=1
  }else if(phase===1){if(panel.restoringConversation)return;check(c.atYEnd,"initial end");check(panel.status(fixture.threads[0])+" "+panel.duration(fixture.threads[0].startedAt)==="Trabajando 1m 5s"||panel.duration(fixture.threads[0].startedAt).startsWith("1m"),"elapsed duration");check(keys.findChild(panel,"t3LiveContextText").text==="Ejecutando rg","chat shows context without duration");panel.detachConversation();check(!panel.followingTail,"gesture immediately disengages follow");c.positionViewAtIndex(22,ListView.Beginning);c.movementEnded();phase=2
  }else if(phase===2){if(panel.restoringConversation)return;retained=c.itemAtIndex(22);var group=keys.findChild(retained,"t3ActivityBlock");group.expanded=true;phase=3
  }else if(phase===3){panel.captureState();anchor=b.sessionPositions[b.selectedKey];check(!anchor.atEnd,"reading position stored");tick=0;phase=4
  }else if(phase===4){if(panel.restoringConversation)return;maxDrift=Math.max(maxDrift,drift());check(c.itemAtIndex(22)===retained,"live updates retain delegates");check(keys.findChild(retained,"t3ActivityBlock").expanded,"expanded tool state survives stream");check(maxDrift<3,"pixel anchor stable during bursts "+maxDrift);var rows=JSON.parse(JSON.stringify(b.transcript));rows[64].text+="\nStreaming line "+tick;update(rows);if(++tick===12)phase=5
  }else if(phase===5){if(panel.restoringConversation)return;update(b.transcript.slice(5));phase=6
  }else if(phase===6){if(panel.restoringConversation)return;check(drift()<3,"pruning before viewport preserves pixel anchor");check(c.itemAtIndex(17)===retained,"pruning keeps surviving delegate");panel.followingTail=true;panel.settleSubmittedTail();tick=0;phase=7
  }else if(phase===7){if(panel.restoringConversation)return;check(c.atYEnd,"streaming stays at tail");var rows=JSON.parse(JSON.stringify(b.transcript));rows[rows.length-1].text+="\n"+"growing response ".repeat(15);update(rows);if(++tick===12)phase=8
  }else if(phase===8){if(panel.restoringConversation)return;check(c.atYEnd,"tail stable after burst");var text=keys.findChild(panel,"t3LiveContextText");check(text.animating,"context animates while visible");text.step=15;text.grabToImage(function(r){root.captured=r.saveToFile("/tmp/t3-activity-shimmer.png")});phase=9
  }else if(phase===9){if(!captured)return;b.activity=[];check(b.activityLabel()==="Pensando","thinking fallback without timer");panel.close();console.log("T3_SCROLL_STABILITY_QML_PASS: stable delegates, expanded tools, pixel anchors during bursts/pruning, gesture interruption, tail growth, elapsed/context and shimmer");Qt.quit()}
 }}
}
