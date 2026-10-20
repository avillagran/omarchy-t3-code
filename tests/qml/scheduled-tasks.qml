import QtQuick
import Quickshell
import QtTest
import "UnderTest/ui/qml" as Test
ShellRoot{
 id:root;property int phase:0;property var fixture:null
 function check(ok,msg){if(!ok){console.error("SCHEDULED_TASKS_QML_FAIL "+msg);Qt.exit(1)}}
 Test.T3Panel{id:panel;bridge:Test.T3Backend}
 TestCase{id:keys;when:false}
 Connections{target:Test.T3Backend;function onSnapshotUpdated(){if(root.fixture)Test.T3Backend.state=root.fixture}}
 Timer{interval:220;running:true;repeat:true;onTriggered:{var b=Test.T3Backend
  if(phase===0){if(!b.viewStateLoaded)return;b.rememberView({windowMode:"floating",windowWidth:1300,windowHeight:900});fixture={threads:[{id:"one",key:"fixture/one",environmentId:"fixture",title:"One",projectId:"p",modelSelection:{instanceId:"codex",model:"m"}}],projects:[{id:"p",key:"fixture/p",title:"Alpha",environmentId:"fixture"}],environments:[]};b.state=fixture;b.selectThread("fixture","one");panel.open();phase=1
  }else if(phase===1){if(b.draft.loading)return;var p=keys.findChild(panel,"t3ScheduledTasks");p.show();check(b.scheduledTasks.lastAction==="list","opening lists tasks");b.action({op:"scheduled-tasks",requestId:b.scheduledTasks.requestId,result:{tasks:[{id:"t1",title:"Daily CI",prompt:"Check CI",enabled:true,schedule:{type:"fixed_time",timeOfDay:"09:00",weekdays:[1,2,3,4,5]},projectId:"p",lastRunStatus:"failed"}]}});phase=2
  }else if(phase===2){var p=keys.findChild(panel,"t3ScheduledTasks");check(p.tasks.length===1&&p.describe(p.tasks[0].schedule).indexOf("09:00")>=0,"lists with schedule");p.edit(null);check(p.editing.projectId==="p"&&p.editing.type==="interval"&&!!p.editing.modelSelection,"new task defaults to the open thread's project and model");phase=3
  }else if(phase===3){var p=keys.findChild(panel,"t3ScheduledTasks");keys.findChild(p,"scheduledTaskTitle").text="Hourly";keys.findChild(p,"scheduledTaskPrompt").text="Summarize";keys.findChild(p,"scheduledTaskMinutes").text="30";p.save();check(b.scheduledTasks.lastAction==="upsert","save sends upsert");b.action({op:"scheduled-tasks",requestId:b.scheduledTasks.requestId,result:{tasks:[]}});phase=4
  }else if(phase===4){var p=keys.findChild(panel,"t3ScheduledTasks");check(p.editing===null,"form closes after save");p.close();panel.close();console.log("SCHEDULED_TASKS_QML_PASS: list, schedule description, defaults, save");Qt.quit()}
 }}
}
