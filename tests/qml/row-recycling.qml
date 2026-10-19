import QtQuick
import QtTest
import Quickshell
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property int step:0;property int waited:0;property var fixture:null
 readonly property var lists:["threadList","workingThreadList","settledThreadList"]
 function check(ok,msg){if(!ok){console.error("ROW_RECYCLING_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 // Threads keep their identity while they start, finish and settle, so
 // rows keep moving between the inbox, Working and Settled.
 function threads(seed){var r=[];for(var i=0;i<16;i++){var k=(i*5+seed)%9,w=k<3,s=k===8;r.push({id:"t"+i,key:"fixture/t"+i,environmentId:"fixture",projectKey:"fixture/p",projectId:"p",project:"Project",title:"Thread "+i,working:w,startedAt:w?1e12:0,status:w?"running":"completed",settled:s,updatedAt:1e12-((i*3+seed)%16)*60000,activeAnchorAt:1e12-((i*3+seed)%16)*60000})}return r}
 function push(seed){fixture={threads:threads(seed),projects:[{key:"fixture/p",id:"p",title:"Project",environmentId:"fixture"}],environments:[]};Test.T3Backend.snapshotAboutToChange();Test.T3Backend.state=fixture}
 // Rows the view no longer places (recycled, or left over from a remove
 // transition) must not be drawn over the rows that took their slots.
 function strays(name){var view=keys.findChild(panel,name),live=[];for(var i=0;i<view.count;i++){var it=view.itemAtIndex(i);if(it)live.push(it)}
  return view.contentItem.children.filter(function(c){return c.thread!==undefined&&c.visible&&c.opacity>0&&live.indexOf(c)<0&&c!==view.currentItem}).map(function(c){return c.thread.id+(c.pooled?" (recycled)":"")})}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:phase===1?70:150;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({sidebarCompact:true,workingExpanded:true,settledExpanded:true,threadMoveEffect:"arc",threadMoveSpeed:"fast",workingShelfHeight:140,settledShelfHeight:110});push(0);panel.open();phase=1
  }else if(phase===1){if(panel.restoringSidebar&&step===0)return;push(++step%9+Math.floor(step/9))
   var list=keys.findChild(panel,"threadList");if(step%4===0)list.contentY=list.originY+(step%8?0:Math.max(0,list.contentHeight-list.height))
   if(step>=45)phase=2
  }else if(phase===2){if(++waited<10)return;check(!panel.rowMotion,"motion settles");
   lists.forEach(function(name){var s=strays(name);check(s.length===0,name+" draws rows it no longer places: "+JSON.stringify(s))})
   console.log("ROW_RECYCLING_QML_PASS: rows recycled after section moves stay hidden");panel.close();Qt.callLater(Qt.quit)
  }
 }}
}
