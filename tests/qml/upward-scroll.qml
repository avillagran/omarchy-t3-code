import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root;property int phase:0;property int ticks:0;property int frames:0;property var fixture:null;property var positions:({});property real largest:0;property int originChanges:0;property int direction:1;property real oldOrigin:0
 function check(ok,msg){if(!ok){console.error("T3_UPWARD_SCROLL_QML_FAIL: "+msg);Qt.exit(1)}}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:16;running:phase===2||phase===4||phase===6;repeat:true;onTriggered:{
  var c=panel.conversationView,next={},deltas=[]
  c.contentItem.children.forEach(function(item){if(item.pooled===false&&item.modelData){var id=item.modelData.id,y=item.y-c.contentY;if(item.height>0&&y<c.height&&y+item.height>0){next[id]=y;if(root.positions[id]!==undefined)deltas.push(y-root.positions[id])}}})
  if(deltas.length){deltas.sort(function(a,b){return a-b});var d=deltas[Math.floor(deltas.length/2)];largest=Math.max(largest,Math.abs(d));check(d*direction>=-2&&Math.abs(d)<160,"visual directional drift "+JSON.stringify({delta:d,origin:c.originY,y:c.contentY,target:c.wheelTarget,tick:ticks,frame:frames}))}
  if(oldOrigin!==c.originY)originChanges++;oldOrigin=c.originY;positions=next;frames++
 }}
 Timer{interval:80;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one");var rows=[];for(var i=0;i<110;i++){var n=i%11===3?90:i%5===1?25:1;var text=("Message "+i+" · a variable height row with enough text to wrap in a narrower chat\n").repeat(n);var row={id:"m"+i,role:i%17===9?"user":"assistant",text:text};if(i%13===8){row.role="changes";row.changes={files:Array(15).fill({path:"src/changed.rs",kind:"modified",additions:3,deletions:1}),additions:45,deletions:15}}else if(i%13===4){row.role="activity";row.activity={command:"rg",active:false,count:3,entries:[{command:"rg"},{command:"sed"},{command:"cat"}]}}else if(row.role==="assistant"&&i%5===1)row.rendered="<p>"+text.replace(/\n/g,"<br>")+"</p>";rows.push(row)}b.transcript=rows;panel.setWindowMode("floating");panel.open();phase=1
  }else if(phase===1){if(panel.restoringConversation)return;c.forceActiveFocus();oldOrigin=c.originY;phase=2
  }else if(phase===2){keys.mouseWheel(c,c.width/2,c.height/2,0,120);if(++ticks===65){phase=3;ticks=0}
  }else if(phase===3){if(++ticks===3){direction=-1;positions={};keys.mouseWheel(c,c.width/2,c.height/2,0,-120);phase=4;ticks=0}
  }else if(phase===4){keys.mouseWheel(c,c.width/2,c.height/2,0,-120);if(++ticks===20){phase=5;ticks=0}}
  else if(phase===5){if(++ticks===3){direction=1;positions={};keys.mouseWheel(c,c.width/2,c.height/2,0,120);phase=6;ticks=0}}
  else if(phase===6){keys.mouseWheel(c,c.width/2,c.height/2,0,120);if(++ticks===20)phase=7}
  else if(phase===7){check(frames>100,"sampled rendered frames");console.log("T3_UPWARD_SCROLL_QML_PASS: frames="+frames+" origins="+originChanges+" maxVisualStep="+largest);panel.close();Qt.quit()}
 }}
}
