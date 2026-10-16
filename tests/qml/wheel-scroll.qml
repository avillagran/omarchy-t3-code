import QtQuick
import Quickshell
import QtTest
import "UnderTest" as Test
ShellRoot {
 id:root;property int anchorIndex:-1;property real anchorScreenY:0;property int phase:0;property int tick:0;property var fixture:null;property real beforeY:0;property real lastY:0;property int samples:0
 function check(ok,msg){if(!ok){console.error("T3_WHEEL_SCROLL_QML_FAIL: "+msg);Qt.exit(1)}}
 function refresh(){var b=Test.T3Backend;b.messagesAboutToChange();var r=JSON.parse(JSON.stringify(b.transcript));r[r.length-1].text+="\nLive line";b.transcript=r;b.messagesUpdated()}
 function opaque(){var children=panel.conversationView.contentItem.children;for(var i=0;i<children.length;i++)if(children[i].hasOwnProperty("pooled")&&!children[i].pooled)check(children[i].opacity===1,"recycled message opacity remains1")}
 TestCase{id:keys;when:false}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:16;running:root.phase===2;repeat:true;onTriggered:{var c=panel.conversationView;root.check(c.contentY<=root.lastY+2,"upward wheel stays monotonic during live updates "+c.contentY+" vs "+root.lastY);root.lastY=c.contentY;root.samples++;root.opaque()}}
 Timer{interval:100;running:true;repeat:true;onTriggered:{
  var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){if(!b.viewStateLoaded)return;b.state={threads:[{key:"fixture/one",id:"one",environmentId:"fixture",title:"One"}],projects:[],environments:[]};fixture=b.state;b.selectThread("fixture","one");var rows=[];for(var i=0;i<80;i++)rows.push({id:"m"+i,role:"assistant",text:("Message "+i+" variable line\n").repeat(1+i%6)});rows[35]={id:"m35",role:"changes",changes:{files:[{path:"src/main.rs",kind:"modified",additions:2,deletions:1}],additions:2,deletions:1}};b.transcript=rows;panel.open();phase=1
  }else if(phase===1){if(panel.restoringConversation)return;c.forceActiveFocus();beforeY=c.contentY;lastY=beforeY;keys.mouseWheel(c,c.width/2,c.height/2,0,120);refresh();tick=0;phase=2
  }else if(phase===2){check(!panel.followingTail,"actual wheel detaches follow");if(++tick<6){keys.mouseWheel(c,c.width/2,c.height/2,0,120);refresh()}else phase=3
  }else if(phase===3){check(c.contentY<beforeY-300&&samples>10,"real wheel moves smoothly across frames");c.movementEnded();check(!panel.followingTail,"layout movement signal cannot re-enable follow");opaque();c.wheelScroll(0,90);tick=0;phase=30
  }else if(phase===30){if(++tick<11)return;beforeY=c.contentY;for(var i=0;i<c.count;i++){var item=c.itemAtIndex(i);if(item&&item.y+item.height>c.contentY&&item.y<c.contentY+c.height){anchorIndex=i;anchorScreenY=item.y-c.contentY;break}}check(anchorIndex>=0,"visible anchor after coast");refresh();phase=4
  }else if(phase===4){if(panel.restoringConversation)return;var anchor=c.itemAtIndex(anchorIndex);check(!!anchor&&Math.abs(anchor.y-c.contentY-anchorScreenY)<3,"visible text stays fixed through refresh after coast");c.positionViewAtIndex(35,ListView.Beginning);panel.captureState();opaque();phase=5
  }else if(phase===5){opaque();var card=c.itemAtIndex(35);check(card&&card.opacity===1,"changed-files card stays fully opaque");keys.keyClick(Qt.Key_End);phase=6
  }else if(phase===6){if(panel.restoringConversation)return;check(c.atYEnd&&panel.followingTail,"End resumes live tail and cancels wheel animation");c.movementEnded();check(panel.followingTail,"layout movement signal cannot disable follow");refresh();phase=7
  }else if(phase===7){if(panel.restoringConversation)return;check(c.atYEnd,"live tail after wheel");panel.close();console.log("T3_WHEEL_SCROLL_QML_PASS: native wheel/monotonic frames, live refresh, pixel delta, pooled opacity/card, movement signals and End");Qt.quit()}
 }}
}
