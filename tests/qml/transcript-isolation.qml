import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot {
 id:root
 property var fixture:null
 property int phase:0
 property int cycle:0
 property int oldSerial:0
 function check(ok,msg){if(!ok){console.error("TRANSCRIPT_ISOLATION_QML_FAIL: "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 function rows(tag){var a=[];for(var i=0;i<18;i++)a.push({id:tag+"-"+i,role:"assistant",text:tag+" "+i,rendered:"<p>"+(tag+" line<br>").repeat(i%4+1)+"</p>",createdAt:i});return a}
 function receive(serial,rs){Test.T3Backend.receive(JSON.stringify({type:"messages",data:{serial:serial,result:{timeline:rs}}}))}
 Timer{interval:100;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,c=panel.conversationView
  if(phase===0){if(!b.viewStateLoaded)return;fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture"},{id:"two",key:"fixture/two",environmentId:"fixture"}],projects:[],environments:[]};b.state=fixture;panel.setWindowMode("floating");panel.open();b.visiblePanels=1;b.selectThread("fixture","one");b.transcript=rows("one");phase=1
  }else if(phase===1){if(panel.restoringConversation)return;panel.detachConversation();c.positionViewAtIndex(8,ListView.Beginning);phase=2
  }else if(phase===2){oldSerial=b.requestedSerial;var tag=cycle%2?"one":"two";b.selectThread("fixture",tag);receive(oldSerial,rows("stale"));check(!b.transcript.length&&c.count===0,"late response rejected and old visual model cleared after switch");receive(b.requestedSerial,rows(tag));phase=3
  }else if(phase===3){if(panel.restoringConversation)return;var live=[];for(var i=0;i<c.count;i++){var it=c.itemAtIndex(i);if(it){live.push(it);check(it.row.id.indexOf(b.selectedThread+"-")===0,"delegate belongs to selected thread");var next=c.itemAtIndex(i+1);if(next)check(next.y>=it.y+it.height-1,"message rows do not overlap")}}
   var strays=c.contentItem.children.filter(function(it){return it.row!==undefined&&it.visible&&it.opacity>0&&live.indexOf(it)<0&&it.y+it.height>c.contentY&&it.y<c.contentY+c.height});check(!strays.length,"removed delegates stay hidden: "+strays.map(it=>it.row.id+" index="+it.index+" y="+it.y+" height="+it.height).join());
   if(++cycle<10){phase=1}else{console.log("TRANSCRIPT_ISOLATION_QML_PASS");panel.close();Qt.quit()}
  }
 }}
}
