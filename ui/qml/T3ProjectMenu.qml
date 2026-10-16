import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
import qs.Ui
import "../js/ProjectModel.js" as Projects
import "../js/IconAssets.js" as Icons
Item {
 id:root
 signal groupingChanged(string previous,string current)
 required property var bridge
 property var project:({})
 property string mode:"rename"
 property string requestId:""
 property string error:""
 property var modelSelection:null
 readonly property bool busy:!!bridge.managementBusy || !!bridge.managementRequest
 readonly property bool canWrite:(bridge.catalogs[project.environmentId] || {}).canWrite===true
 readonly property int threadCount:(bridge.state.threads || []).filter(function(t){return t.projectKey===root.project.key}).length
 function configureProject(key){var p=(bridge.state.projects || []).find(function(p){return p.key===key});if(!p)return;project=Object.assign({},p);bridge.ensureCatalog(p.environmentId);edit("settings")}
 function dismissContext(){menu.close()}
 function showProject(key,item,px,py){
  if(busy)return
  var p=(bridge.state.projects || []).find(function(p){return p.key===key});if(!p||p.isScratch)return
  project=Object.assign({},p);error="";bridge.ensureCatalog(p.environmentId)
  menu.parent=item;menu.x=px;menu.y=py;menu.open()
 }
 function edit(which){
  mode=which;error="";requestId="";nameInput.text=project.title || "";modelSelection=project.defaultModelSelection || null
  grouping.currentIndex=Math.max(0,groupingOptions.findIndex(function(v){return v.value===((root.bridge.viewState.sidebarProjectGroupingOverrides || {})[Projects.physical(root.project)] || ((root.bridge.state.nativeProjectGrouping || {}).overrides || {})[Projects.physical(root.project)] || "inherit")}))
  envMode.currentIndex=project.defaultThreadEnvMode==="local"?1:project.defaultThreadEnvMode==="worktree"?2:0
  autoPull.checked=!!project.autoPull;deleteThreads.checked=false;dialog.open()
  if(which==="rename")Qt.callLater(function(){nameInput.forceActiveFocus();nameInput.selectAll()})
 }
 readonly property var groupingState:Object.assign({},bridge.viewState,{nativeProjectGrouping:bridge.state.nativeProjectGrouping})
 readonly property var groupingOptions:[{value:"inherit",title:T3I18n.tr("Use global default")},{value:"repository",title:T3I18n.tr("Group by repository")},{value:"repository_path",title:T3I18n.tr("Group by repository path")},{value:"separate",title:T3I18n.tr("Keep separate")}]
 function save(){
  if(busy)return
  if(mode==="grouping"){
   var overrides=Object.assign({},bridge.viewState.sidebarProjectGroupingOverrides || {}),key=Projects.physical(project),value=groupingOptions[grouping.currentIndex].value
   overrides[key]=value
   var previous=Projects.groupKey(project,groupingState);bridge.rememberView({sidebarProjectGroupingOverrides:overrides});groupingChanged(previous,Projects.groupKey(project,groupingState));dialog.close();return
  }
  if(!canWrite)return
  var cmd={action:mode==="rename"?"project-rename":mode==="remove"?"project-remove":"project-settings",environment:project.environmentId,projectId:project.id}
  if(mode==="rename")cmd.title=nameInput.text.trim()
  if(mode==="remove"){cmd.confirmed=true;cmd.force=deleteThreads.checked}
  if(mode==="settings"){
   var patch={},env=envMode.currentIndex===0?null:envMode.currentIndex===1?"local":"worktree"
   if(nameInput.text.trim()!==project.title)patch.title=nameInput.text.trim()
   if(JSON.stringify(modelSelection)!==JSON.stringify(project.defaultModelSelection || null))patch.defaultModelSelection=modelSelection
   if(env!==(project.defaultThreadEnvMode || null))patch.defaultThreadEnvMode=env
   if(autoPull.checked!==!!project.autoPull)patch.autoPull=autoPull.checked
   if(!Object.keys(patch).length){dialog.close();return}
   cmd.patch=patch
  }
  bridge.manage(cmd);requestId=bridge.managementRequest?bridge.managementRequest.requestId:""
 }
 TextEdit{id:copyBuffer;visible:false;text:root.project.workspaceRoot || "";textFormat:TextEdit.PlainText}
 Controls.Menu {
  id:menu;objectName:"t3ProjectContextMenu"
  palette.window:Color.popups.background;palette.text:Color.popups.text;palette.windowText:Color.popups.text
  background:Rectangle{color:Color.popups.background;radius:Style.space(7);border.width:1;border.color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.2)}
  Controls.MenuItem{objectName:"projectRename";text:T3I18n.tr("Rename");enabled:root.canWrite&&!root.busy;onTriggered:root.edit("rename")}
  Controls.MenuItem{objectName:"projectGrouping";text:T3I18n.tr("Group into…");enabled:!root.busy;onTriggered:root.edit("grouping")}
  Controls.MenuItem{objectName:"projectCopyPath";text:T3I18n.tr("Copy Path");enabled:!!root.project.workspaceRoot;onTriggered:{copyBuffer.selectAll();copyBuffer.copy();copyBuffer.deselect()}}
  Controls.MenuItem{objectName:"projectSettings";icon.source:"data:image/svg+xml,"+encodeURIComponent(Icons.svg("settings").replace(/currentColor/g,String(Color.popups.text)));icon.color:Color.popups.text;text:T3I18n.tr("Project settings");onTriggered:root.edit("settings")}
  Controls.MenuSeparator{}
  Controls.MenuItem{objectName:"projectRemove";icon.source:"data:image/svg+xml,"+encodeURIComponent(Icons.svg("trash-2").replace(/currentColor/g,String(Color.urgent)));icon.color:Color.urgent;text:T3I18n.tr("Remove");enabled:root.canWrite&&!root.busy;palette.text:Color.urgent;palette.windowText:Color.urgent;onTriggered:root.edit("remove")}
 }
 QtObject {
  id:modelBridge
  readonly property var catalog:root.bridge.catalogs[root.project.environmentId] || ({})
  readonly property string selectedEnvironment:root.project.environmentId || ""
  readonly property var favoritePending:root.bridge.favoritePending || ({})
  readonly property string favoriteError:root.bridge.favoriteError || ""
  function chooseModel(index){var m=(catalog.models || [])[index];if(m)root.modelSelection={instanceId:m.instanceId,model:m.model,options:[]}}
  function refreshCatalog(){root.bridge.send({op:"catalog",environment:selectedEnvironment})}
  // This picker chooses defaults for its project; favorites remain T3-owned.
  function toggleModelFavorite(provider,model){root.bridge.toggleModelFavoriteIn(selectedEnvironment,provider,model)}
 }
 Controls.Popup {
  id:dialog;objectName:"t3ProjectDialog"
  parent:Controls.Overlay.overlay;anchors.centerIn:parent;width:Math.min(Style.space(520),parent?parent.width-Style.space(32):Style.space(520));padding:Style.space(20);modal:true;focus:true
  closePolicy:root.busy?Controls.Popup.NoAutoClose:Controls.Popup.CloseOnEscape
  background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:root.mode==="remove"?Color.urgent:Color.accent}
  contentItem:ColumnLayout{
   spacing:Style.space(12)
   RowLayout{
    Layout.fillWidth:true
    Text{Layout.fillWidth:true;text:T3I18n.tr(root.mode==="rename"?"Rename":root.mode==="grouping"?"Project grouping":root.mode==="remove"?"Remove project":"Project settings");color:Color.popups.text;font.pixelSize:Style.font.body;font.bold:true}
    T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");enabled:!root.busy;onClicked:dialog.close()}
   }
   Text{Layout.fillWidth:true;text:root.project.title || "";color:Color.popups.text;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   TextEdit{Layout.fillWidth:true;visible:!!text;text:root.project.workspaceRoot || "";readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText;color:Color.popups.text;opacity:0.65;font.pixelSize:Style.font.caption}
   Text{Layout.fillWidth:true;text:root.project.environmentId || "";color:Color.popups.text;opacity:0.65;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
   Controls.TextField{
    id:nameInput;objectName:"projectNameInput";Layout.fillWidth:true;visible:root.mode==="rename"||root.mode==="settings";enabled:root.canWrite&&!root.busy;selectByMouse:true;color:Color.popups.text;Accessible.name:T3I18n.tr("Project name")
    background:Rectangle{color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.06);radius:Style.space(6);border.width:nameInput.activeFocus?1:0;border.color:Color.accent}
    onAccepted:if(root.mode==="rename"&&text.trim())root.save()
   }
   T3Select{id:grouping;objectName:"projectGroupingSelect";Layout.fillWidth:true;visible:root.mode==="grouping";model:root.groupingOptions;textRole:"title";enabled:!root.busy}
   ColumnLayout{
    visible:root.mode==="settings";Layout.fillWidth:true;spacing:Style.space(10)
    Text{text:T3I18n.tr("Default model");color:Color.popups.text;font.pixelSize:Style.font.caption}
    RowLayout{
     Layout.fillWidth:true
     T3ModelPicker{objectName:"projectDefaultModel";Layout.fillWidth:true;bridge:modelBridge;selection:root.modelSelection || ({});enabled:root.canWrite&&!root.busy}
     T3IconButton{name:"undo-2";tooltipText:T3I18n.tr("Use global default");enabled:root.canWrite&&!root.busy;onClicked:root.modelSelection=null}
    }
    Text{text:T3I18n.tr("Default environment");color:Color.popups.text;font.pixelSize:Style.font.caption}
    T3Select{id:envMode;objectName:"projectDefaultEnvironment";Layout.fillWidth:true;model:[T3I18n.tr("Use global default"),"Local","Worktree"];enabled:root.canWrite&&!root.busy}
    Controls.CheckBox{id:autoPull;objectName:"projectAutoPull";text:T3I18n.tr("Auto pull");palette.windowText:Color.popups.text;enabled:root.canWrite&&!root.busy}
   }
   Text{Layout.fillWidth:true;visible:root.mode==="remove";text:T3I18n.tr("Removing this project deletes its T3 threads. Files on disk are kept.")+"\n"+T3I18n.tr("This action cannot be undone.");color:Color.urgent;font.pixelSize:Style.font.body;wrapMode:Text.Wrap}
   Controls.CheckBox{id:deleteThreads;objectName:"projectDeleteThreads";visible:root.mode==="remove";text:T3I18n.tr("Delete all project threads");palette.windowText:Color.urgent;enabled:!root.busy}
   Text{Layout.fillWidth:true;visible:!!root.error;text:T3I18n.error(root.error);color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   RowLayout{
    Layout.fillWidth:true
    WorkIndicator{visible:root.busy;active:visible&&dialog.visible}
    Item{Layout.fillWidth:true}
    Button{focusable:true;text:T3I18n.tr("Cancel");enabled:!root.busy;onClicked:dialog.close()}
    Button{
     objectName:"projectSave";foreground:root.mode==="remove"?Color.urgent:Color.popups.text;focusable:true;text:root.bridge.managementRequest?T3I18n.tr("Retry"):T3I18n.tr(root.mode==="remove"?"Remove":"Save")
     enabled:!root.bridge.managementBusy&&(root.mode==="grouping"||root.canWrite)&&((root.mode!=="rename"&&root.mode!=="settings")||!!nameInput.text.trim())&&(root.mode!=="remove"||root.threadCount===0||deleteThreads.checked)
     onClicked:{if(root.bridge.managementRequest){root.bridge.manage({});root.requestId=root.bridge.managementRequest.requestId}else root.save()}
    }
   }
  }
 }
 Connections{target:root.bridge;function onManagementFinished(data){if(data.requestId!==root.requestId)return;root.error=data.error || "";if(!data.error)dialog.close()}}
}
