import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Controls.Popup {
 id:root
 objectName:"t3AddProjectDialog"
 required property var bridge
 property int step:0
 property string environment:""
 property string source:"local"
 property string browseDirectory:""
 property string createdAt:""
 readonly property var sourceState:bridge.projectSources || ({})
 readonly property var sources:buildSources()
 readonly property string filterText:path.text.endsWith("/")?"":path.text.slice(path.text.lastIndexOf("/")+1).toLocaleLowerCase()
 readonly property var filteredFolders:root.browse.environment===root.environment?(root.browse.entries || []).filter(function(e){return !root.filterText||e.name.toLocaleLowerCase().indexOf(root.filterText)>=0}):[]
 readonly property var environments:(bridge.state.environments || []).filter(function(e){return e.state==="ready"})
 readonly property var browse:bridge.folderBrowse || ({entries:[]})
 readonly property bool locked:!!bridge.managementBusy || !!bridge.managementRequest
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 parent:Controls.Overlay.overlay;anchors.centerIn:parent
 width:Math.min(Style.space(620),parent?parent.width-Style.space(32):Style.space(620))
 padding:Style.space(16);modal:true;focus:true
 background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.18)}
 function begin(){
  if(bridge.managementRequest&&["add-project","create-project","clone-project"].indexOf(bridge.managementRequest.action)>=0){
   var request=bridge.managementRequest;environment=request.environment;source=request.action==="clone-project"?request.source:request.action==="create-project"?"new":"local";step=source==="new"?4:2;path.text=request.workspaceRoot || "~/";repository.text=request.repository || "";projectName.text=request.name || "";createdAt=request.createdAt || ""
  }else{environment=environments.length===1?environments[0].id:"";step=environments.length===1?1:0;source="local";path.text="~/";repository.text="";projectName.text="";createdAt=new Date().toISOString();bridge.managementError="";if(environment)bridge.loadProjectSources(environment)}
  open()
 }
 function buildSources(){
  var list=[];var state=sourceState.environment===environment?sourceState:({})
  if(state.newProjectsRoot)list.push({id:"new",title:T3I18n.tr("New project"),description:T3I18n.tr("Start a new Git repository from a name"),icon:"folder-git-2",ready:true})
  list.push({id:"local",title:T3I18n.tr("Local folder"),description:T3I18n.tr("Browse a folder on disk"),icon:"folder-plus",ready:true})
  list.push({id:"url",title:"Git URL",description:T3I18n.tr("Clone from a remote URL"),icon:"git-branch",ready:state.cloneTracking===true})
  var definitions=[{id:"github",name:"GitHub"},{id:"gitlab",name:"GitLab"},{id:"gitcafe",name:"GitCafe"},{id:"forgejo",name:"Forgejo"},{id:"bitbucket",name:"Bitbucket"},{id:"azure-devops",name:"Azure DevOps"}]
  var providers=definitions.map(function(d){var p=(state.providers || []).find(function(p){return p.kind===d.id});return {id:d.id,title:T3I18n.tr("{provider} repository",{provider:d.name}),description:T3I18n.tr("Clone a repository"),icon:"git-branch",ready:state.cloneTracking===true&&!!p&&p.status==="available"&&p.auth.status!=="unauthenticated"}})
  providers.sort(function(a,b){return Number(b.ready)-Number(a.ready)||a.title.localeCompare(b.title)})
  return list.concat(providers)
 }
 function sourceButton(id){for(var i=0;i<sourceRows.count;i++){var item=sourceRows.itemAt(i);if(item&&item.modelData.id===id)return item}return null}
 function browsePath(value){if(locked||!environment)return;filterDelay.stop();path.text=value.endsWith("/")?value:value+"/";browseDirectory=path.text;bridge.browseFolders(environment,path.text);Qt.callLater(function(){if(root.visible&&root.step===2){path.forceActiveFocus();path.cursorPosition=path.text.length}})}
 function filterPath(){if(locked||step!==2)return;var t=path.text;var dir=t.indexOf("/")<0?browseDirectory:t.slice(0,t.lastIndexOf("/")+1);if(dir&&dir!==browseDirectory){browseDirectory=dir;bridge.browseFolders(environment,dir)}}
 function back(){if(locked)return;filterDelay.stop();if(step===2)step=source==="local"?1:3;else if(step>=3)step=1;else step=0}
 function chooseEnvironment(id){if(locked)return;environment=id;step=1;bridge.loadProjectSources(id)}
 function chooseSource(id){source=id;if(id==="local")openFolders();else if(id==="new"){step=4;projectName.forceActiveFocus()}else{step=3;repository.forceActiveFocus()}}
 function openFolders(){step=2;var state=sourceState.environment===environment?sourceState:({});browsePath(state.baseDirectory || "~/")}
 function cloneName(){return repository.text.trim().split(/[?#]/)[0].replace(/\/$/,"").split(/[/:]/).pop().replace(/\.git$/,"")}
 function create(){
  if(bridge.managementBusy)return
  if(bridge.managementRequest){bridge.manage({});return}
  if(filterDelay.running||browse.busy||browse.error||browse.environment!==environment||!browse.parentPath)return
  if(source==="local")bridge.manage({action:"add-project",environment:environment,workspaceRoot:browse.parentPath})
  else {var name=cloneName();if(!name||name==="."||name==="..")return;bridge.manage({action:"clone-project",environment:environment,source:source,repository:repository.text.trim(),workspaceRoot:browse.parentPath.replace(/\/$/,"")+"/"+name,createdAt:createdAt})}
 }
 Timer{id:filterDelay;interval:160;onTriggered:root.filterPath()}
 contentItem:ColumnLayout {
  spacing:Style.space(12)
  RowLayout {
   T3IconButton{objectName:"projectBack";name:"chevron-left";tooltipText:T3I18n.tr("Back");visible:root.step>0;enabled:!root.locked;onClicked:root.back()}
   Text{Layout.fillWidth:true;text:T3I18n.tr(root.step===0?"Choose environment":root.step===1?"Add project":root.step===3?"Clone a repository":root.step===4?"New project":"Choose folder");color:root.fg;font.bold:true;font.pixelSize:Style.font.body}
   Text{visible:root.step>0;text:(root.environments.find(function(e){return e.id===root.environment}) || {}).name || root.environment;color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideRight;Layout.maximumWidth:Style.space(180)}
   T3IconButton{name:"x";tooltipText:T3I18n.tr("Close");onClicked:root.close()}
  }
  ColumnLayout {
   visible:root.step===0;Layout.fillWidth:true
   Repeater{model:root.environments
    delegate:Controls.ItemDelegate{required property var modelData;objectName:"projectEnvironment_"+modelData.id;Layout.fillWidth:true;text:modelData.name;onClicked:root.chooseEnvironment(modelData.id)}
   }
   Text{visible:!root.environments.length;text:T3I18n.tr("Environment unavailable");color:root.muted}
  }
  ColumnLayout {
   visible:root.step===1;Layout.fillWidth:true;spacing:0
   Text{visible:!!root.sourceState.busy;text:T3I18n.tr("Loading…");color:root.muted}
   Repeater{id:sourceRows;model:root.sources
    delegate:Controls.ItemDelegate {
     required property var modelData
     objectName:modelData.id==="local"?"projectLocalFolder":"projectSource_"+modelData.id
     Layout.fillWidth:true;implicitHeight:Style.space(48);enabled:modelData.ready&&!root.locked
     contentItem:RowLayout{T3Icon{name:modelData.icon;tint:root.fg}ColumnLayout{Text{text:modelData.title;color:root.fg;font.pixelSize:Style.font.body}Text{text:modelData.description;color:root.muted;font.pixelSize:Style.font.caption}}Item{Layout.fillWidth:true}Text{visible:!modelData.ready;text:T3I18n.tr("Setup required");color:root.muted;font.pixelSize:Style.font.caption}T3Icon{name:"chevron-right";tint:root.muted}}
     onClicked:root.chooseSource(modelData.id)
    }
   }
   Text{visible:!!root.sourceState.error;Layout.fillWidth:true;text:T3I18n.error(root.sourceState.error || "");color:Color.urgent;wrapMode:Text.Wrap}
  }
  ColumnLayout {
   visible:root.step===3;Layout.fillWidth:true
   Controls.TextField{id:repository;objectName:"projectRepository";Layout.fillWidth:true;placeholderText:root.source==="url"?"https://host/owner/repository.git":"owner/repository";enabled:!root.locked;onAccepted:if(text.trim())root.openFolders()}
   Controls.Button{objectName:"projectChooseDestination";text:T3I18n.tr("Choose folder");enabled:!!repository.text.trim()&&!root.locked;onClicked:root.openFolders()}
  }
  ColumnLayout {
   visible:root.step===4;Layout.fillWidth:true
   Controls.TextField{id:projectName;objectName:"projectName";Layout.fillWidth:true;placeholderText:T3I18n.tr("Project name");enabled:!root.locked;maximumLength:200;onAccepted:if(text.trim())root.bridge.manage({action:"create-project",environment:root.environment,name:text.trim()})}
   Text{Layout.fillWidth:true;text:root.sourceState.newProjectsRoot || "";color:root.muted;elide:Text.ElideMiddle}
   Text{visible:!!root.bridge.managementError;Layout.fillWidth:true;text:T3I18n.error(root.bridge.managementError);color:Color.urgent;wrapMode:Text.Wrap}
   Controls.Button{objectName:"projectCreateNew";text:T3I18n.tr("New project");enabled:!!projectName.text.trim()&&!root.locked;onClicked:root.bridge.manage({action:"create-project",environment:root.environment,name:projectName.text.trim()})}
  }
  ColumnLayout {
   visible:root.step===2;Layout.fillWidth:true;spacing:Style.space(8)
   RowLayout {
    Layout.fillWidth:true
    T3IconButton{objectName:"projectParentFolder";name:"arrow-up";tooltipText:T3I18n.tr("Parent folder");enabled:!root.locked&&!root.browse.busy&&!!root.browse.parentPath&&root.browse.parentPath!=="/";onClicked:{var p=root.browse.parentPath.replace(/\/$/,"");root.browsePath(p.slice(0,p.lastIndexOf("/")) || "/")}}
    Controls.TextField{id:path;objectName:"projectFolderPath";Layout.fillWidth:true;enabled:!root.locked;selectByMouse:true;onTextEdited:filterDelay.restart();Keys.onDownPressed:if(root.filteredFolders.length){folders.currentIndex=0;folders.forceActiveFocus()}
     onAccepted:{root.filterPath();if(root.filteredFolders.length===1&&!text.endsWith("/"))root.browsePath(root.filteredFolders[0].fullPath);else root.browsePath(text.indexOf("/")<0?root.browseDirectory+text:text)}}
    T3IconButton{name:"arrow-right-left";tooltipText:T3I18n.tr("Open folder");enabled:!root.locked&&!root.browse.busy;onClicked:root.browsePath(path.text)}
   }
   Text{Layout.fillWidth:true;text:root.browse.busy?T3I18n.tr("Loading…"):root.browse.parentPath || "";color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
   ListView {
    id:folders;objectName:"projectFolders";Layout.fillWidth:true;Layout.preferredHeight:Math.min(Style.space(310),root.parent?root.parent.height*0.38:Style.space(310));clip:true
    model:root.filteredFolders
    activeFocusOnTab:true;keyNavigationEnabled:true
    Keys.onReturnPressed:if(currentIndex>=0&&!root.locked)root.browsePath(model[currentIndex].fullPath)
    delegate:Controls.ItemDelegate {
     required property var modelData;required property int index
     width:folders.width;enabled:!root.locked&&!root.browse.busy;highlighted:folders.currentIndex===index
     contentItem:RowLayout{T3Icon{name:"folder";tint:root.muted}Text{Layout.fillWidth:true;text:modelData.name;color:root.fg;font.pixelSize:Style.font.body;elide:Text.ElideRight;textFormat:Text.PlainText}T3Icon{name:"chevron-right";tint:root.muted}}
     onClicked:{folders.currentIndex=index;root.browsePath(modelData.fullPath)}
    }
    T3Wheel{flickable:folders}
    Controls.ScrollBar.vertical:Controls.ScrollBar{}
   }
   Text{Layout.fillWidth:true;visible:!!text;text:T3I18n.error(root.browse.error || root.bridge.managementError || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
   Text{visible:root.source!=="local";Layout.fillWidth:true;text:(root.browse.parentPath || "").replace(/\/$/,"")+"/"+root.cloneName();color:root.fg;elide:Text.ElideMiddle}
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}Controls.Button{objectName:"projectOpenFolder";text:T3I18n.tr(root.bridge.managementRequest?"Retry same action":root.source==="local"?"Open project":"Clone a repository");enabled:!root.bridge.managementBusy&&(!!root.bridge.managementRequest || (!filterDelay.running&&!root.browse.busy&&!root.browse.error&&root.browse.environment===root.environment&&!!root.browse.parentPath));onClicked:root.create()}}
  }
 }
 Connections{target:root.bridge;function onManagementFinished(data){if(!data.error&&data.environment===root.environment&&data.result&&["add-project","create-project","clone-project"].indexOf(data.result.action)>=0)root.close()}}
}
