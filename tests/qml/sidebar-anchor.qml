import QtQuick
import QtTest
import Quickshell
import qs.Commons
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null;property string anchorId:"";property real anchorY:0
 function check(ok,msg){if(!ok){console.error("SIDEBAR_ANCHOR_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 function rows(n,extra){var r=[];for(var i=0;i<n;i++)r.push({id:"t"+i,key:"fixture/t"+i,environmentId:"fixture",title:"Thread "+i,project:"Project",updatedAt:1e12-i*60000,activeAnchorAt:1e12-i*60000});return extra?[extra].concat(r):r}
 function firstVisible(list){var i=list.indexAt(1,list.contentY+1);var item=list.itemAtIndex(i);return {id:panel.activeRows[i].id,y:item.y-list.contentY}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:350;running:true;repeat:true;onTriggered:{var b=Test.T3Backend,list=keys.findChild(panel,"threadList")
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({sidebarCompact:false,settledExpanded:false,threadOrder:"t3"});b.state={threads:rows(30),projects:[],environments:[]};fixture=b.state;panel.open();phase=1
  }else if(phase===1){list.contentY=list.originY+Style.space(400);phase=2
  }else if(phase===2){var a=firstVisible(list);anchorId=a.id;anchorY=a.y;check(anchorId!=="t0","list scrolled away from the top");
   // A thread above the viewport moves into the inbox (e.g. finished elsewhere).
   b.state={threads:rows(30,{id:"new",key:"fixture/new",environmentId:"fixture",title:"New",project:"Project",updatedAt:1e12+60000,activeAnchorAt:1e12+60000}),projects:[],environments:[]};fixture=b.state;phase=3
  }else if(phase===3){var a=firstVisible(list);check(panel.activeRows[0].id==="new","new row sorts first");check(a.id===anchorId&&Math.abs(a.y-anchorY)<1,"visible rows stay put "+JSON.stringify({before:[anchorId,anchorY],after:a}));
   list.contentY=list.originY;phase=4
  }else if(phase===4){b.state={threads:rows(30,{id:"top",key:"fixture/top",environmentId:"fixture",title:"Top",project:"Project",updatedAt:1e12+120000,activeAnchorAt:1e12+120000}),projects:[],environments:[]};fixture=b.state;phase=5
  }else if(phase===5){check(firstVisible(list).id==="top","at the top, new rows still show "+JSON.stringify({first:firstVisible(list),cy:list.contentY,oy:list.originY,anchor:list.anchor,ids:panel.activeRows.slice(0,3).map(function(r){return r.id})}));console.log("SIDEBAR_ANCHOR_QML_PASS: inserts above the viewport keep visible rows in place");panel.close();Qt.callLater(Qt.quit)}
 }}
}
