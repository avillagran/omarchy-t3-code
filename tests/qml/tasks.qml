import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0
 property var steps:[{id:"a",text:"Verify",status:"completed",durationMs:3000},{id:"b",text:"Verify",status:"running"},{id:"c",text:"Report",status:"pending"}]
 function check(ok,msg){if(!ok){console.error("T3_TASKS_QML_FAIL: "+msg);Qt.callLater(function(){Qt.exit(1)})}}
 TestCase{id:keys;when:false}
 QtObject{id:fake;property var viewState:({});property var tasks:({total:3,completed:1,current:"Verify",items:root.steps});function rememberView(v){viewState=Object.assign({},viewState,v)}}
 Test.PinnedKeyboardPanel{id:window;anchorItem:null;bar:null;contentWidth:700;contentHeight:280;padding:8
  Loader{id:loader;anchors.fill:parent;sourceComponent:Component{Test.T3Tasks{bridge:fake}}}
 }
 Timer{interval:220;running:true;repeat:true;onTriggered:{
  var card=loader.item
  if(phase===0){window.open=true;phase=1
  }else if(phase===1){check(card.visible&&!card.expanded,"Tasks defaults to compact badge");check(keys.findChild(card,"t3TasksProgress").text==="1/3","completed and total count");check(keys.findChild(card,"t3TasksCurrent").text==="Verify","current task summary");check(keys.findChild(card,"t3TaskSegments").visible,"short task lists have colored segments");keys.mouseClick(keys.findChild(card,"t3TasksToggle"),30,15);phase=2
  }else if(phase===2){var list=keys.findChild(card,"t3TaskList");check(card.expanded&&list.visible&&list.count===3,"click expands all tasks including duplicate labels");check(card.duration(root.steps[0])==="3s"&&card.duration(root.steps[1])==="ahora","durations follow T3 without local clock");check(fake.viewState.tasksExpanded===true,"expanded preference saved");loader.active=false;phase=3
  }else if(phase===3){loader.active=true;phase=4
  }else if(phase===4){check(card.expanded,"expansion survives component recreation");var toggle=keys.findChild(card,"t3TasksToggle");toggle.forceActiveFocus();keys.keyClick(Qt.Key_Return);phase=5
  }else if(phase===5){check(!card.expanded,"keyboard collapses Tasks");var many=[];for(var i=0;i<11;i++)many.push({id:"task"+i,text:"Task "+i,status:"pending"});fake.tasks={total:11,completed:0,current:"Task 0",items:many};phase=6
  }else if(phase===6){check(!keys.findChild(card,"t3TaskSegments").visible,"segments omitted above ten tasks like T3");fake.tasks=null;phase=7
  }else if(phase===7){check(!card.visible,"no active task list leaves no empty UI");window.open=false;console.log("T3_TASKS_QML_PASS: task badge, counts, repeated labels, durations, segments, native click/keyboard, persistence and empty state");Qt.quit()}
 }}
}
