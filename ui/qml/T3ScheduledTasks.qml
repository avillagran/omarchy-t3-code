import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// T3's scheduled tasks: prompts that start a thread on an interval, at a time of day,
// or when a webhook is called.
Controls.Popup {
 id:root;objectName:"t3ScheduledTasks"
 required property var bridge
 readonly property var view:bridge.scheduledTasks || ({})
 readonly property var tasks:view.tasks || []
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 property var editing:null
 readonly property var projects:(bridge.state.projects || []).filter(function(p){return p.environmentId===(root.bridge.selectedEnvironment||"local")&&!p.isScratch})
 readonly property var dayNames:["Sun","Mon","Tue","Wed","Thu","Fri","Sat"]
 function show(){editing=null;open();bridge.scheduledTasksAction({action:"list"})}
 function describe(s){if(!s)return "";if(s.type==="interval"){var m=Math.round(s.everyMs/60000);return m%1440===0?T3I18n.tr("Every {count} days",{count:m/1440}):m%60===0?T3I18n.tr("Every {count} hours",{count:m/60}):T3I18n.tr("Every {count} minutes",{count:m})}if(s.type==="fixed_time"){var days=s.weekdays&&s.weekdays.length?s.weekdays.map(function(d){return T3I18n.tr(root.dayNames[d])}).join(", "):T3I18n.tr("Every day");return days+" · "+s.timeOfDay}return T3I18n.tr("Webhook")}
 function edit(task){var s=task?task.schedule:{type:"interval",everyMs:3600000};editing={workspaceStrategy:task&&task.workspaceStrategy?task.workspaceStrategy:{type:"root"},interactionMode:task&&task.interactionMode||"default",threadId:task&&task.threadId||null,id:task?task.id:null,title:task?task.title:"",prompt:task?task.prompt:"",projectId:task?task.projectId:(root.bridge.selectedInfo.projectId||(projects[0]||{}).id||""),type:s.type,minutes:s.type==="interval"?Math.round(s.everyMs/60000):60,timeOfDay:s.timeOfDay||"09:00",weekdays:s.weekdays||[],enabled:task?task.enabled:true,modelSelection:task?task.modelSelection:(root.bridge.selectedInfo.modelSelection||null),runtimeMode:task?task.runtimeMode:(root.bridge.currentRuntimeMode?root.bridge.currentRuntimeMode():"approval-required")}}
 function save(){var e=editing;if(!e)return;root.bridge.scheduledTasksAction({action:"upsert",id:e.id||undefined,title:title.text,prompt:prompt.text,projectId:e.projectId,enabled:e.enabled,modelSelection:e.modelSelection,runtimeMode:e.runtimeMode,interactionMode:e.interactionMode,threadId:e.threadId,workspaceStrategy:e.workspaceStrategy.type==="root"?{type:"root"}:Object.assign({},e.workspaceStrategy,e.workspaceStrategy.type==="worktree"?{baseRef:baseRef.text}:{worktreePath:worktreePath.text}),schedule:e.type==="interval"?{type:"interval",minutes:Number(minutes.text)}:e.type==="fixed_time"?{type:"fixed_time",timeOfDay:time.text,weekdays:e.weekdays}:{type:"webhook"}})}
 Connections{target:root.bridge;function onScheduledTasksChanged(){if(!root.view.busy&&!root.view.error&&root.view.lastAction==="upsert")root.editing=null}}
 modal:true;focus:true;padding:Style.space(18)
 x:Math.round((parent.width-width)/2);y:Math.round((parent.height-height)/2)
 width:Math.min(Style.space(680),parent.width-Style.space(40));height:Math.min(Style.space(600),parent.height-Style.space(40))
 background:Rectangle{radius:Style.space(12);color:Color.popups.background;border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.14)}
 ColumnLayout{
  anchors.fill:parent;spacing:Style.space(10)
  RowLayout{Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:T3I18n.tr("Scheduled tasks");color:root.fg;font.pixelSize:Style.font.body+4;font.weight:Font.DemiBold}
   T3PickerButton{objectName:"scheduledTaskNew";visible:!root.editing;implicitHeight:Style.space(28);text:T3I18n.tr("New task");enabled:root.projects.length>0;onClicked:root.edit(null)}
   T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.close()}
  }
  Text{visible:!!root.view.error;Layout.fillWidth:true;text:T3I18n.error(root.view.error||"");color:Color.urgent;wrapMode:Text.Wrap}
  // List
  ListView{
   id:list;objectName:"scheduledTaskList";visible:!root.editing;Layout.fillWidth:true;Layout.fillHeight:true;clip:true;model:root.tasks;spacing:Style.space(6)
   T3Wheel{flickable:list}
   Text{anchors.centerIn:parent;visible:!list.count;text:root.view.busy?T3I18n.tr("Loading…"):T3I18n.tr("No scheduled tasks. A task sends a prompt to a new thread on a schedule.");color:root.muted;width:parent.width*0.8;horizontalAlignment:Text.AlignHCenter;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
   delegate:Rectangle{
    required property var modelData;width:list.width;height:row.implicitHeight+Style.space(16);radius:Style.space(8);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.04)
    RowLayout{id:row;anchors.fill:parent;anchors.margins:Style.space(8);spacing:Style.space(8)
     T3Switch{checked:modelData.enabled;enabled:!root.view.busy;Accessible.name:T3I18n.tr("Enabled")+" · "+modelData.title;onToggled:root.bridge.scheduledTasksAction({action:"enable",id:modelData.id,enabled:checked})}
     ColumnLayout{Layout.fillWidth:true;spacing:Style.space(2)
      Text{Layout.fillWidth:true;text:modelData.title;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}
      Text{Layout.fillWidth:true;text:root.describe(modelData.schedule)+(modelData.lastRunStatus==="failed"?" · "+T3I18n.tr("Last run failed"):modelData.lastRunStatus==="running"?" · "+T3I18n.tr("Running"):"")+(modelData.webhook&&modelData.webhook.url?" · "+modelData.webhook.url:"");color:modelData.lastRunStatus==="failed"?Color.urgent:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
     }
     T3IconButton{name:"play";tooltipText:T3I18n.tr("Run now");enabled:!root.view.busy;onClicked:root.bridge.scheduledTasksAction({action:"run",id:modelData.id})}
     T3IconButton{name:"pen-line";tooltipText:T3I18n.tr("Edit");onClicked:root.edit(modelData)}
     T3IconButton{name:"trash-2";tooltipText:T3I18n.tr("Delete");enabled:!root.view.busy;onClicked:root.bridge.scheduledTasksAction({action:"delete",id:modelData.id})}
    }
   }
  }
  // Form
  ColumnLayout{
   objectName:"scheduledTaskForm";visible:!!root.editing;Layout.fillWidth:true;Layout.fillHeight:true;spacing:Style.space(8)
   Controls.TextField{id:title;objectName:"scheduledTaskTitle";Layout.fillWidth:true;placeholderText:T3I18n.tr("Title");text:root.editing?root.editing.title:"";color:root.fg}
   Controls.TextArea{id:prompt;objectName:"scheduledTaskPrompt";Layout.fillWidth:true;Layout.fillHeight:true;wrapMode:TextEdit.Wrap;placeholderText:T3I18n.tr("Prompt");text:root.editing?root.editing.prompt:"";color:root.fg;background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05)}}
   RowLayout{Layout.fillWidth:true;spacing:Style.space(8)
    Text{text:T3I18n.tr("Project");color:root.muted;font.pixelSize:Style.font.caption}
    T3Select{Layout.fillWidth:true;model:root.projects.map(function(p){return p.title});currentIndex:root.editing?Math.max(0,root.projects.findIndex(function(p){return p.id===root.editing.projectId})):0;onActivated:function(i){root.editing=Object.assign({},root.editing,{projectId:root.projects[i].id})}}
   }
   RowLayout {
    Layout.fillWidth:true
    T3Select{objectName:"scheduledTaskWorkspace";Layout.preferredWidth:Style.space(160);translateLabels:true;model:["Local","Worktree","Existing worktree"];currentIndex:root.editing?Math.max(0,["root","worktree","existing_worktree"].indexOf(root.editing.workspaceStrategy.type)):0;onActivated:function(i){root.editing=Object.assign({},root.editing,{workspaceStrategy:{type:["root","worktree","existing_worktree"][i],baseRef:root.bridge.selectedInfo.branch||"",worktreePath:root.bridge.selectedInfo.worktreePath||""}})}}
    T3TextField{id:baseRef;objectName:"scheduledTaskBaseRef";Layout.fillWidth:true;visible:!!root.editing&&root.editing.workspaceStrategy.type==="worktree";text:root.editing?root.editing.workspaceStrategy.baseRef||"":"";placeholderText:T3I18n.tr("Base branch")}
    T3TextField{id:worktreePath;Layout.fillWidth:true;visible:!!root.editing&&root.editing.workspaceStrategy.type==="existing_worktree";text:root.editing?root.editing.workspaceStrategy.worktreePath||"":"";placeholderText:T3I18n.tr("Worktree path")}
   }
   RowLayout{Layout.fillWidth:true;spacing:Style.space(8)
    T3Select{objectName:"scheduledTaskType";implicitWidth:Style.space(150);model:[T3I18n.tr("Interval"),T3I18n.tr("Time of day"),T3I18n.tr("Webhook")];currentIndex:root.editing?["interval","fixed_time","webhook"].indexOf(root.editing.type):0;onActivated:function(i){root.editing=Object.assign({},root.editing,{type:["interval","fixed_time","webhook"][i]})}}
    Text{visible:!!root.editing&&root.editing.type==="interval";text:T3I18n.tr("Every");color:root.muted;font.pixelSize:Style.font.caption}
    Controls.TextField{id:minutes;objectName:"scheduledTaskMinutes";visible:!!root.editing&&root.editing.type==="interval";implicitWidth:Style.space(70);validator:IntValidator{bottom:1;top:525600}text:root.editing?String(root.editing.minutes):"60";color:root.fg}
    Text{visible:!!root.editing&&root.editing.type==="interval";text:T3I18n.tr("minutes");color:root.muted;font.pixelSize:Style.font.caption}
    Controls.TextField{id:time;objectName:"scheduledTaskTime";visible:!!root.editing&&root.editing.type==="fixed_time";implicitWidth:Style.space(70);inputMask:"99:99";text:root.editing?root.editing.timeOfDay:"09:00";color:root.fg}
    Repeater{model:root.editing&&root.editing.type==="fixed_time"?7:0
     T3PickerButton{required property int index;implicitHeight:Style.space(26);leftPadding:Style.space(6);rightPadding:Style.space(6);primary:root.editing.weekdays.indexOf(index)>=0;text:T3I18n.tr(root.dayNames[index]);onClicked:{var d=root.editing.weekdays.slice(),i=d.indexOf(index);if(i>=0)d.splice(i,1);else d.push(index);d.sort();root.editing=Object.assign({},root.editing,{weekdays:d})}}}
    Item{Layout.fillWidth:true}
   }
   Text{visible:!!root.editing&&root.editing.type==="webhook";Layout.fillWidth:true;wrapMode:Text.Wrap;color:root.muted;font.pixelSize:Style.font.caption;text:T3I18n.tr("Runs on each request to the task's webhook URL, shown after saving. The prompt may use {{body.path}} placeholders.")}
   RowLayout{Layout.fillWidth:true
    Item{Layout.fillWidth:true}
    T3PickerButton{implicitHeight:Style.space(30);primary:false;text:T3I18n.tr("Cancel");onClicked:root.editing=null}
    T3PickerButton{objectName:"scheduledTaskSave";implicitHeight:Style.space(30);text:T3I18n.tr("Save");enabled:!root.view.busy&&!!title.text.trim()&&!!prompt.text.trim()&&!!root.editing&&!!root.editing.projectId&&!!root.editing.modelSelection&&(root.editing.workspaceStrategy.type!=="worktree"||!!baseRef.text.trim())&&(root.editing.workspaceStrategy.type!=="existing_worktree"||!!worktreePath.text.trim());onClicked:root.save()}
   }
  }
 }
}
