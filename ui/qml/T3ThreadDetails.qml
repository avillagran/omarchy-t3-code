import QtQuick 6.11
import QtQuick.Layouts 6.11
import QtQuick.Controls as Controls
import qs.Commons
import "../js/Subagents.js" as Subagents
// The open thread's details, after T3's ThreadDetailsPanel: where it runs,
// opening it in an editor, project scripts, version control, automations
// bound to it and its lineage. Each row hands off to the panel's existing
// tools (Git & scripts, working review, Agents tab, scheduled tasks).
Rectangle {
 id:root
 objectName:"t3ThreadDetails"
 required property var bridge
 signal closed()
 signal gitRequested()
 signal changesRequested()
 signal agentsRequested()
 signal automationsRequested()
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.62)
 readonly property color faint:Qt.rgba(fg.r,fg.g,fg.b,0.08)
 readonly property var info:bridge.selectedInfo || ({})
 readonly property var catalog:bridge.catalogs[bridge.selectedEnvironment] || ({})
 readonly property var details:bridge.threadDetails.key===bridge.selectedKey?bridge.threadDetails:({editors:[],cwd:"",busy:true,error:""})
 readonly property var editors:details.editors || []
 readonly property var editor:editors.find(function(e){return e.id===root.bridge.viewState.preferredEditor}) || editors[0] || null
 readonly property var git:bridge.gitView || ({})
 readonly property bool gitReady:!!git.data&&!git.loading&&!git.error&&git.scope&&git.scope.thread===bridge.selectedThread&&git.scope.environment===bridge.selectedEnvironment
 readonly property var status:gitReady?(git.data.status || {}):({})
 readonly property var scripts:gitReady?(git.data.scripts || []):[]
 readonly property bool canScript:gitReady&&!!git.data.permissions&&git.data.permissions.terminalWrite===true&&!bridge.gitBusy
 readonly property var quick:{var a=gitReady?(git.data.actions || {}):{};var order=[["commit_push_pr","Commit, push & PR"],["commit_push","Commit & push"],["commit","Commit"],["push","Push"],["pull","Pull"],["create_pr","Create PR"]];var hit=order.find(function(o){return a[o[0]]===true});return hit?{operation:hit[0],label:hit[1]}:null}
 readonly property var tasks:(bridge.scheduledTasks.tasks || []).filter(function(t){return t.threadId===root.bridge.selectedThread})
 readonly property var running:(bridge.threadSubagents || []).filter(Subagents.live)
 readonly property var previous:(bridge.threadSubagents || []).filter(function(s){return !Subagents.live(s)})
 readonly property bool hasLineage:!!bridge.parentThread||(bridge.threadSubagents || []).length>0||(bridge.childThreads || []).length>0
 property real now:Date.now()
 function load(){if(!bridge.selectedThread)return;bridge.requestThreadDetails();if(bridge.canOpenGit&&!(git.scope&&git.scope.thread===bridge.selectedThread&&(git.loading||git.data)))bridge.requestGit();bridge.scheduledTasksAction({action:"list"})}
 Component.onCompleted:load()
 Connections{target:root.bridge;function onSelectedKeyChanged(){Qt.callLater(root.load)}}
 Timer{interval:1000;repeat:true;running:root.visible&&root.running.length>0;onTriggered:root.now=Date.now()}
 readonly property real contentHeight:body.implicitHeight+Style.space(20)
 implicitHeight:Math.min(preferredHeight>0?Math.max(Style.space(120),preferredHeight):contentHeight,maxHeight)
 property real maxHeight:Style.space(560)
 // 0 = fit the content; set by dragging the bottom edge.
 property real preferredHeight:0
 signal resized(real width,real height)
 radius:Style.space(10);color:Color.popups.background;border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.12)
 component DetailRow: Rectangle {
  id:row
  property string icon:""
  property string text:""
  property string detail:""
  property color tint:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.62)
  property bool interactive:true
  default property alias trailing:trail.data
  signal activated()
  Layout.fillWidth:true;implicitHeight:Style.space(30);radius:Style.space(6)
  color:interactive&&rowHover.hovered?Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.06):"transparent"
  Accessible.role:Accessible.Button;Accessible.name:text
  HoverHandler{id:rowHover;cursorShape:row.interactive?Qt.PointingHandCursor:Qt.ArrowCursor}
  TapHandler{enabled:row.interactive;onTapped:row.activated()}
  Controls.ToolTip.visible:rowHover.hovered&&!!row.detail;Controls.ToolTip.delay:600;Controls.ToolTip.text:row.detail
  RowLayout {
   anchors{fill:parent;leftMargin:Style.space(6);rightMargin:Style.space(4)}spacing:Style.space(8)
   T3Icon{visible:!!row.icon;name:row.icon || "server";implicitWidth:Style.space(14);tint:row.tint}
   Text{Layout.fillWidth:true;text:row.text;color:Color.popups.text;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
   RowLayout{id:trail;spacing:Style.space(4)}
  }
 }
 component Heading: Text { Layout.fillWidth:true;Layout.topMargin:Style.space(8);Layout.leftMargin:Style.space(6);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.55);font.pixelSize:Style.font.caption;font.weight:Font.Medium;textFormat:Text.PlainText;elide:Text.ElideRight }
 component Divider: Rectangle { Layout.fillWidth:true;Layout.topMargin:Style.space(6);implicitHeight:1;color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.08) }
 Flickable {
  anchors.fill:parent;anchors.margins:Style.space(10);contentHeight:body.implicitHeight;clip:true;boundsBehavior:Flickable.StopAtBounds
  Controls.ScrollBar.vertical:Controls.ScrollBar{policy:Controls.ScrollBar.AsNeeded}
  ColumnLayout {
   id:body;width:parent.width;spacing:Style.space(2)
   RowLayout {
    Layout.fillWidth:true;Layout.leftMargin:Style.space(6);spacing:Style.space(6)
    Text{Layout.fillWidth:true;text:T3I18n.tr("Thread details");color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.Medium;textFormat:Text.PlainText}
    T3IconButton{objectName:"t3ThreadDetailsClose";name:"x";implicitWidth:Style.space(22);implicitHeight:implicitWidth;tooltipText:T3I18n.tr("Close");onClicked:root.closed()}
   }
   // Where it runs: the environment's machine, its workspace and the environment.
   Rectangle {
    objectName:"t3DetailsRunContext";Layout.fillWidth:true;implicitHeight:Style.space(30);radius:Style.space(6);color:"transparent"
    Accessible.role:Accessible.StaticText;Accessible.name:runLabel.text
    HoverHandler{id:runHover}
    Controls.ToolTip.visible:runHover.hovered&&!!root.details.cwd;Controls.ToolTip.delay:600;Controls.ToolTip.text:root.details.cwd || ""
    RowLayout {
     anchors{fill:parent;leftMargin:Style.space(6)}spacing:Style.space(6)
     T3EnvironmentIcon{objectName:"t3DetailsMachine";kind:root.catalog.machine || "server";implicitWidth:Style.space(14);tint:root.dim}
     T3Icon{name:root.info.worktreePath?"folder-git-2":root.info.branch?"folder-git":"folder";implicitWidth:Style.space(14);tint:root.dim}
     Text{id:runLabel;Layout.fillWidth:true;text:root.info.environmentName || root.bridge.selectedEnvironment;color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    }
   }
   // Open in the preferred editor; the chevron picks another one.
   DetailRow {
    objectName:"t3DetailsOpenEditor";visible:!!root.editor
    icon:"external-link";text:root.editor?T3I18n.tr("Open in {editor}",{editor:root.editor.label}):"";detail:root.bridge.editorOpen.error?T3I18n.error(root.bridge.editorOpen.error):root.details.cwd || ""
    tint:root.bridge.editorOpen.error?Color.urgent:root.dim;interactive:!root.bridge.editorOpen.busy
    onActivated:root.bridge.openThreadInEditor(root.editor.id)
    T3IconButton{objectName:"t3DetailsChooseEditor";visible:root.editors.length>1;name:"chevron-down";implicitWidth:Style.space(22);implicitHeight:implicitWidth;tooltipText:T3I18n.tr("Choose editor");onClicked:editorMenu.popup()}
    Controls.Menu{id:editorMenu;Repeater{model:root.editors;Controls.MenuItem{required property var modelData;text:modelData.label;checkable:true;checked:root.editor&&root.editor.id===modelData.id;onTriggered:root.bridge.openThreadInEditor(modelData.id)}}}
   }
   // Project scripts run in the thread's terminal, as from Git & scripts.
   DetailRow {
    objectName:"t3DetailsScripts";visible:root.bridge.canOpenGit
    icon:root.scripts.length?"play":"plus";text:root.scripts.length?T3I18n.tr("Run {script}",{script:root.scripts[0].name}):T3I18n.tr("Add project script")
    interactive:root.scripts.length?root.canScript:true
    onActivated:if(root.scripts.length)root.bridge.requestGitAction({scope:root.git.scope,revision:root.git.data.revision,operation:"script",scriptId:root.scripts[0].id,confirmed:true});else root.gitRequested()
    T3IconButton{visible:root.scripts.length>1;name:"chevron-down";implicitWidth:Style.space(22);implicitHeight:implicitWidth;tooltipText:T3I18n.tr("Script actions");onClicked:scriptMenu.popup()}
    Controls.Menu{id:scriptMenu;Repeater{model:root.scripts;Controls.MenuItem{required property var modelData;text:modelData.name;enabled:root.canScript;onTriggered:root.bridge.requestGitAction({scope:root.git.scope,revision:root.git.data.revision,operation:"script",scriptId:modelData.id,confirmed:true})}}}
   }
   // Version control.
   Divider{visible:root.bridge.canOpenGit}
   Heading{visible:root.bridge.canOpenGit;text:T3I18n.tr("Version control")}
   DetailRow{visible:root.bridge.canOpenGit&&!root.gitReady;interactive:false;icon:"loader-circle";text:root.git.error?T3I18n.error(root.git.error):T3I18n.tr("Loading…")}
   DetailRow{objectName:"t3DetailsInitGit";visible:root.gitReady&&root.status.isRepo===false;icon:"git-branch-plus";text:T3I18n.tr("Initialize Git");onActivated:root.bridge.initializeRepository()}
   DetailRow{
    objectName:"t3DetailsBranch";visible:root.gitReady&&root.status.isRepo===true;icon:"git-branch";text:root.status.refName || T3I18n.tr("Detached HEAD");detail:T3I18n.tr("Git & scripts");onActivated:root.gitRequested()
    Text{visible:(root.status.aheadCount || 0)+(root.status.behindCount || 0)>0;text:"↑"+(root.status.aheadCount || 0)+" ↓"+(root.status.behindCount || 0);color:root.dim;font.pixelSize:Style.font.caption}
   }
   DetailRow{objectName:"t3DetailsGitAction";visible:root.gitReady&&!!root.quick;icon:"git-merge";text:root.quick?T3I18n.tr(root.quick.label):"";onActivated:root.gitRequested()}
   DetailRow{
    objectName:"t3DetailsChanges";visible:root.gitReady&&root.status.isRepo===true;icon:"file-diff";text:T3I18n.tr("Changes");onActivated:root.changesRequested()
    Text{text:"+"+((root.status.workingTree || {}).insertions || 0);color:T3Palette.done;font.pixelSize:Style.font.caption}
    Text{text:"−"+((root.status.workingTree || {}).deletions || 0);color:T3Palette.failed;font.pixelSize:Style.font.caption}
   }
   // Scheduled tasks bound to this thread.
   Divider{visible:root.tasks.length>0}
   Heading{visible:root.tasks.length>0;text:T3I18n.tr("Automations")}
   Repeater {
    model:root.tasks
    DetailRow{
     required property var modelData
     icon:"calendar-clock";text:modelData.title || "";tint:modelData.lastRunStatus==="failed"?T3Palette.failed:modelData.lastRunStatus==="running"?T3Palette.working:root.dim
     detail:modelData.enabled?(modelData.nextRunAt?T3I18n.tr("Next run {time}",{time:new Date(modelData.nextRunAt).toLocaleString(Qt.locale(),Locale.ShortFormat)}):""):T3I18n.tr("Paused")
     onActivated:root.automationsRequested()
    }
   }
   // Lineage: parent, running subagents and how many finished.
   Divider{visible:root.hasLineage}
   Heading{visible:root.hasLineage;text:T3I18n.tr("Lineage")+(root.running.length?" · "+T3I18n.tr("{n} running",{n:root.running.length}):"")}
   T3SubagentRow{visible:!!root.bridge.parentThread;Layout.fillWidth:true;bridge:root.bridge;icon:"corner-left-up";relation:T3I18n.tr("Parent agent");showDetail:false;subagent:root.bridge.parentThread?({title:root.bridge.parentThread.title,status:"completed",childThreadId:root.bridge.parentThread.id}):({});onOpened:root.bridge.openParentThread()}
   Repeater {
    model:root.running.slice(0,4)
    T3SubagentRow{required property var modelData;Layout.fillWidth:true;bridge:root.bridge;subagent:modelData;now:root.now;showDetail:false;onOpened:root.bridge.openSubagent(modelData)}
   }
   DetailRow{objectName:"t3DetailsPreviousAgents";visible:root.previous.length>0||root.running.length>4;icon:"bot";text:T3I18n.tr("Previous agents ({count})",{count:root.previous.length});onActivated:root.agentsRequested()
    T3Icon{name:"chevron-right";implicitWidth:Style.space(13);tint:root.dim}}
  }
 }

 // Drag the left edge for width, the bottom edge for height; double-click fits the content again.
 MouseArea {
  objectName:"t3ThreadDetailsResizeWidth";width:Style.space(6);anchors{left:parent.left;top:parent.top;bottom:parent.bottom;leftMargin:-Style.space(3)}
  cursorShape:Qt.SizeHorCursor;property real startX:0;property real startW:0
  onPressed:function(m){startX=mapToItem(null,m.x,0).x;startW=root.width}
  onPositionChanged:function(m){if(pressed)root.resized(Math.max(Style.space(240),startW-(mapToItem(null,m.x,0).x-startX)),root.preferredHeight)}
  onDoubleClicked:root.resized(0,root.preferredHeight)
 }
 MouseArea {
  objectName:"t3ThreadDetailsResizeHeight";height:Style.space(6);anchors{left:parent.left;right:parent.right;bottom:parent.bottom;bottomMargin:-Style.space(3)}
  cursorShape:Qt.SizeVerCursor;property real startY:0;property real startH:0
  onPressed:function(m){startY=mapToItem(null,0,m.y).y;startH=root.height}
  onPositionChanged:function(m){if(pressed)root.resized(root.width,Math.max(Style.space(120),Math.min(root.maxHeight,startH+mapToItem(null,0,m.y).y-startY)))}
  onDoubleClicked:root.resized(root.width,0)
 }
}
