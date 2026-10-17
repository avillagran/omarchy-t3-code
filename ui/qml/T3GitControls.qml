import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// Host-owned popup content. No RPC or shell access; signals are intents, never acknowledgements.
Item {
 id: root
 objectName: "t3GitControls"
 required property var scope // environment, thread, projectId, cwd, requestId
 required property var view // loading, error, snapshot (git_controls::execute)
 property bool canSourceControlWrite: false
 property bool canTerminalWrite: false
 property bool canReviewChanges: false // enable only with a real working-tree diff host handler
 property bool busy: false
 property string operationState: "" // pending | unknown | succeeded | failed, owned by outbox
 property string operationError: ""
 property var operation:({})
 property bool reconciled:false
 property int tab:0
 signal terminalRequested()
 signal reconcileRequested()
 signal nativeRequested()
 property var pending: null
 property bool defaultBranchConfirmed: false
 property string commitMessage: ""
 property var selectedFiles: ({})
 signal closed()
 signal refreshRequested()
 signal actionRequested(var intent) // parent allocates UUID and validates/builds against trusted snapshot
 signal changesRequested(string path)
 signal pullRequestRequested(var pullRequest)
 readonly property color fg: Color.popups.text
 readonly property color dim: Qt.rgba(fg.r, fg.g, fg.b, 0.6)
 readonly property var snapshot: view && view.data || null
 readonly property bool matchesScope: {
  if(!snapshot || !snapshot.scope || !scope)return false
  var fields=["environment","thread","projectId","cwd","requestId"]
  return fields.every(function(k){return !!scope[k] && snapshot.scope[k]===scope[k]})
 }
 readonly property bool ready: matchesScope && !view.loading && !view.error
 readonly property var status: ready ? snapshot.status : null
 readonly property var files: status && status.workingTree ? status.workingTree.files : []
 readonly property var scripts: ready ? snapshot.scripts || [] : []
 readonly property bool locked: busy || operationState==="pending" || operationState==="unknown"
 readonly property bool includesCommit: pending && ["commit","commit_push","commit_push_pr"].indexOf(pending.operation)>=0
 readonly property bool needsDefaultConfirmation: pending && status && status.isDefaultRef && pending.operation!=="script" && pending.operation!=="commit" && pending.operation!=="pull"
 readonly property var menu: [
  {operation:"commit",label:"Commit"}, {operation:"push",label:"Push"}, {operation:"pull",label:"Pull"},
  {operation:"commit_push",label:"Commit & push"}, {operation:"create_pr",label:"Create PR"},
  {operation:"commit_push_pr",label:"Commit, push & PR"}
 ]
 readonly property string pendingLabel: {
  if(!pending)return T3I18n.tr("Confirm Git operation")
  if(pending.script)return pending.script.name
  var entry=menu.find(function(item){return item.operation===pending.operation})
  return T3I18n.tr(entry?entry.label:"Confirm Git operation")
 }
 function allowed(operation){return ready && !locked && (operation==="script" ? canTerminalWrite : canSourceControlWrite && snapshot.actions && snapshot.actions[operation]===true)}
 function request(operation, scriptId){
  if(!allowed(operation))return false
  var script=operation==="script"?scripts.find(function(s){return s.id===scriptId}):null
  if(operation==="script"&&!script)return false
  pending={operation:operation,scriptId:scriptId || "",script:script,revision:snapshot.revision}
  commitMessage="";selectedFiles=({});defaultBranchConfirmed=false
  return true
 }
 function cancel(){pending=null;commitMessage="";selectedFiles=({});defaultBranchConfirmed=false}
 function toggleFile(path){var next=Object.assign({},selectedFiles);if(next[path])delete next[path];else next[path]=true;selectedFiles=next}
 function confirm(){
  if(!pending || !allowed(pending.operation) || pending.revision!==snapshot.revision || (needsDefaultConfirmation&&!defaultBranchConfirmed))return false
  var intent={scope:snapshot.scope,revision:snapshot.revision,operation:pending.operation,confirmed:true,defaultBranchConfirmed:defaultBranchConfirmed}
  if(pending.operation==="script")intent.scriptId=pending.scriptId
  if(includesCommit){if(commitMessage.trim())intent.commitMessage=commitMessage.trim();var paths=Object.keys(selectedFiles);if(paths.length)intent.filePaths=paths}
  pending=null;actionRequested(intent);return true
 }
 onScopeChanged:{cancel();reconciled=false}
 onSnapshotChanged:cancel()
 onReadyChanged:if(!ready)cancel()
 activeFocusOnTab:true
 Keys.onEscapePressed:{if(pending)cancel();else closed()}
 ColumnLayout {
  anchors.fill:parent;spacing:Style.space(8)
  RowLayout {
   Layout.fillWidth:true
   T3Icon{name:"git-branch";tint:Color.accent;implicitWidth:Style.space(16)}
   Text{text:T3I18n.tr("Git & scripts");color:root.fg;font.pixelSize:Style.font.body;font.bold:true;Layout.fillWidth:true}
   T3IconButton{name:"refresh-cw";tooltipText:T3I18n.tr("Refresh");enabled:!root.view.loading&&!root.locked;onClicked:{root.cancel();root.refreshRequested()}}
   T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.closed()}
  }
  Text{Layout.fillWidth:true;text:root.scope.cwd || "";color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
  WorkIndicator{Layout.alignment:Qt.AlignHCenter;visible:!!root.view.loading;active:visible}
  Text{Layout.fillWidth:true;visible:!!root.view.error;text:T3I18n.error(root.view.error || "");color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  T3PickerButton{primary:false;visible:!!root.view.error;text:T3I18n.tr("Retry");enabled:!root.locked;onClicked:root.refreshRequested()}
  Text{Layout.fillWidth:true;visible:!root.view.loading&&!root.view.error&&!root.matchesScope;text:T3I18n.tr("Refresh to load this checkout.");color:root.dim;wrapMode:Text.Wrap}
  Text{Layout.fillWidth:true;visible:root.ready&&!root.status.isRepo;text:T3I18n.tr("Not a Git repository.");color:root.dim}
  RowLayout {
   visible:root.ready&&!!root.status.isRepo;Layout.fillWidth:true
   Text{objectName:"t3GitBranch";Layout.fillWidth:true;text:root.status?(root.status.refName || T3I18n.tr("Detached HEAD")):"";color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideMiddle;textFormat:Text.PlainText}
   T3PickerButton{objectName:"t3GitReviewAll";primary:false;text:T3I18n.tr("Review");enabled:root.ready&&root.canReviewChanges;onClicked:root.changesRequested("")}
   Text{text:root.status?"↑ "+root.status.aheadCount+"  ↓ "+root.status.behindCount:"";color:root.dim;font.pixelSize:Style.font.caption}
  }
  Text{visible:root.ready&&!!root.status.isRepo;Layout.fillWidth:true;text:!root.status?"":root.status.hasWorkingTreeChanges?T3I18n.tr("Uncommitted changes"):T3I18n.tr("Working tree clean");color:root.dim;font.pixelSize:Style.font.caption}
  T3PickerButton{primary:false;visible:root.ready&&!!root.status.pr;enabled:!!root.snapshot&&!!root.snapshot.pullRequestReference;text:root.status&&root.status.pr?"#"+root.status.pr.number+" · "+root.status.pr.title:"";Layout.fillWidth:true;onClicked:root.pullRequestRequested(root.status.pr);contentItem:Text{text:parent.text;color:root.fg;elide:Text.ElideRight;textFormat:Text.PlainText;font.pixelSize:Style.font.caption}}
  Text{visible:root.ready&&!!root.status.isRepo&&!root.canSourceControlWrite;text:T3I18n.tr("Source control is read-only.");color:root.dim;font.pixelSize:Style.font.caption}
  Flow {
   Layout.fillWidth:true;spacing:Style.space(5);visible:root.ready&&!!root.status.isRepo&&!root.pending
   Repeater{model:root.menu
    T3PickerButton{primary:false;required property var modelData;text:T3I18n.tr(modelData.label);enabled:root.allowed(modelData.operation);onClicked:root.request(modelData.operation,"");palette.button:Color.popups.background;palette.buttonText:root.fg}
   }
  }
  Text{visible:root.locked;Layout.fillWidth:true;text:T3I18n.tr(root.operationState==="unknown"?"Outcome unknown. Check native T3 before retrying.":"Operation pending in native T3.");color:Color.accent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
  Text{visible:!!root.operation.phase&&root.locked;Layout.fillWidth:true;text:root.operation.phase || "";color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  Text{visible:root.operationState==="succeeded";Layout.fillWidth:true;text:root.operation.result&&root.operation.result.toast?root.operation.result.toast.title:T3I18n.tr("Completed");color:Color.accent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  RowLayout{visible:!!root.operation.terminalId
   Text{visible:root.operationState==="submitted";Layout.fillWidth:true;text:T3I18n.tr("Command delivered to native terminal.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   T3PickerButton{primary:false;text:T3I18n.tr("Open native terminal");onClicked:root.terminalRequested()}
  }
  ColumnLayout{visible:root.operationState==="unknown";Layout.fillWidth:true
   T3PickerButton{primary:false;text:T3I18n.tr("Open in T3");onClicked:root.nativeRequested()}
   Controls.CheckBox{Layout.fillWidth:true;text:T3I18n.tr("I verified this operation has finished in native T3.");checked:root.reconciled;onToggled:root.reconciled=checked}
   T3PickerButton{primary:false;text:T3I18n.tr("Clear resolved operation");enabled:root.reconciled&&!root.busy;onClicked:{root.reconciled=false;root.reconcileRequested()}}
  }
  Text{visible:!!root.operationError;Layout.fillWidth:true;text:T3I18n.error(root.operationError);color:Color.urgent;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  Rectangle {
   visible:!!root.pending;Layout.fillWidth:true;implicitHeight:confirmation.implicitHeight+Style.space(16)
   color:Qt.rgba(Color.accent.r,Color.accent.g,Color.accent.b,0.07);radius:Style.space(6)
   ColumnLayout {
    id:confirmation;anchors.fill:parent;anchors.margins:Style.space(8)
    Text{Layout.fillWidth:true;text:root.pendingLabel;color:root.fg;textFormat:Text.PlainText;font.bold:true}
    Text{Layout.fillWidth:true;visible:root.pending&&root.pending.operation==="script";text:root.pending&&root.pending.script?root.pending.script.command:"";font.family:"monospace";font.pixelSize:Style.font.caption;color:root.fg;wrapMode:Text.WrapAnywhere;textFormat:Text.PlainText}
    Text{Layout.fillWidth:true;text:T3I18n.tr("This operation can change files or remote state.");color:root.dim;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
    Controls.TextField{visible:!!root.includesCommit;Layout.fillWidth:true;placeholderText:T3I18n.tr("Commit message (optional)");text:root.commitMessage;onTextEdited:root.commitMessage=text;color:root.fg;maximumLength:10000}
    Text{visible:!!root.includesCommit;Layout.fillWidth:true;text:T3I18n.tr("Select files below, or leave all unchecked to commit all changes.");color:root.dim;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
    Controls.CheckBox{objectName:"t3GitDefaultConfirm";visible:!!root.needsDefaultConfirmation;text:T3I18n.tr("I confirm this operates on the default branch.");checked:root.defaultBranchConfirmed;onToggled:root.defaultBranchConfirmed=checked}
    RowLayout {
     T3PickerButton{primary:false;text:T3I18n.tr("Cancel");onClicked:root.cancel()}
     T3PickerButton{primary:true;objectName:"t3GitConfirm";text:T3I18n.tr("Confirm");enabled:!!root.pending&&root.allowed(root.pending.operation)&&(!root.needsDefaultConfirmation||root.defaultBranchConfirmed);onClicked:root.confirm()}
    }
   }
  }
  RowLayout {
   Layout.fillWidth:true;spacing:Style.space(4)
   T3PickerButton{primary:root.tab===0;Layout.fillWidth:true;text:T3I18n.tr("Changes")+" · "+root.files.length;onClicked:root.tab=0}
   T3PickerButton{primary:root.tab===1;Layout.fillWidth:true;text:T3I18n.tr("Scripts")+" · "+root.scripts.length;onClicked:root.tab=1}
  }
  ListView {
   id:rows;objectName:"t3GitRows";Layout.fillWidth:true;Layout.fillHeight:true;clip:true;reuseItems:true
   model:root.tab===0?root.files:root.scripts
   T3Wheel{flickable:rows}
   Controls.ScrollBar.vertical:Controls.ScrollBar{}
   delegate:Rectangle {
    id:row;required property var modelData;required property int index
    width:rows.width;height:Style.space(root.tab===0?36:62);radius:Style.space(4);color:"transparent"
    RowLayout {
     anchors.fill:parent;spacing:Style.space(6)
     Controls.CheckBox{visible:root.tab===0&&!!root.includesCommit;checked:!!root.selectedFiles[row.modelData.path];onToggled:root.toggleFile(row.modelData.path)}
     ColumnLayout {
      Layout.fillWidth:true;spacing:Style.space(2)
      Text{Layout.fillWidth:true;text:root.tab===0?row.modelData.path:row.modelData.name;color:root.fg;elide:Text.ElideMiddle;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
      Text{visible:root.tab===1;Layout.fillWidth:true;text:row.modelData.command || "";color:root.dim;elide:Text.ElideRight;font.family:"monospace";font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
      Text{visible:root.tab===1&&(row.modelData.runOnWorktreeCreate||row.modelData.runOnSettle);text:(row.modelData.runOnWorktreeCreate?T3I18n.tr("Setup"):"")+(row.modelData.runOnSettle?" · "+T3I18n.tr("On settle"):"");color:Color.accent;font.pixelSize:Style.font.caption}
     }
     Text{visible:root.tab===0;text:"+"+(row.modelData.insertions || 0)+" −"+(row.modelData.deletions || 0);color:root.dim;font.pixelSize:Style.font.caption}
     T3PickerButton{primary:false;text:T3I18n.tr(root.tab===0?"Review":"Run");enabled:root.tab===0?root.ready&&root.canReviewChanges:root.allowed("script");onClicked:{if(root.tab===0)root.changesRequested(row.modelData.path);else root.request("script",row.modelData.id)}}
    }
   }
   Text{anchors.centerIn:parent;width:parent.width;horizontalAlignment:Text.AlignHCenter;visible:root.ready&&rows.count===0;text:T3I18n.tr(root.tab===0?"No changed files.":"No project scripts configured.");color:root.dim;wrapMode:Text.Wrap}
  }
 }
}
