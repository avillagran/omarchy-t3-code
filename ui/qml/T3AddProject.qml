import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons
Controls.Popup {
 id:root
 required property var bridge
 property int step:0
 property string environment:""
 readonly property var environments:(bridge.state.environments || []).filter(function(e){return e.state==="ready"})
 readonly property var browse:bridge.folderBrowse || ({entries:[]})
 readonly property bool locked:!!bridge.managementBusy || !!bridge.managementRequest
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.6)
 parent:Controls.Overlay.overlay;anchors.centerIn:parent
 width:Math.min(Style.space(620),parent?parent.width-Style.space(32):Style.space(620))
 padding:Style.space(16);modal:true
 background:Rectangle{color:Color.popups.background;radius:Style.space(12);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.18)}
 function begin(){
  if(bridge.managementRequest&&bridge.managementRequest.action==="add-project"){
   environment=bridge.managementRequest.environment;step=2;path.text=bridge.managementRequest.workspaceRoot || "~/"
  }else{environment=environments.length===1?environments[0].id:"";step=environments.length===1?1:0;path.text="~/";bridge.managementError=""}
  open()
 }
 function browsePath(value){if(locked||!environment)return;path.text=value.endsWith("/")?value:value+"/";bridge.browseFolders(environment,path.text)}
 function back(){if(locked)return;if(step===2)step=1;else step=0}
 function chooseEnvironment(id){if(locked)return;environment=id;step=1}
 function openFolders(){step=2;browsePath("~/")}
 function create(){
  if(bridge.managementBusy)return
  if(bridge.managementRequest){bridge.manage({});return}
  if(browse.busy||browse.error||browse.environment!==environment||!browse.parentPath)return
  bridge.manage({action:"add-project",environment:environment,workspaceRoot:browse.parentPath})
 }
 contentItem:ColumnLayout {
  spacing:Style.space(12)
  RowLayout {
   T3IconButton{objectName:"projectBack";name:"chevron-left";tooltipText:T3I18n.tr("Back");visible:root.step>0;enabled:!root.locked;onClicked:root.back()}
   Text{Layout.fillWidth:true;text:T3I18n.tr(root.step===0?"Choose environment":root.step===1?"Add project":"Choose folder");color:root.fg;font.bold:true;font.pixelSize:Style.font.body}
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
  Controls.ItemDelegate {
   objectName:"projectLocalFolder";visible:root.step===1;Layout.fillWidth:true;implicitHeight:Style.space(64)
   contentItem:RowLayout{T3Icon{name:"folder-plus";tint:root.fg}ColumnLayout{Text{text:T3I18n.tr("Local folder");color:root.fg;font.pixelSize:Style.font.body}Text{text:T3I18n.tr("Browse a folder on disk");color:root.muted;font.pixelSize:Style.font.caption}}Item{Layout.fillWidth:true}T3Icon{name:"chevron-right";tint:root.muted}}
   onClicked:root.openFolders()
  }
  ColumnLayout {
   visible:root.step===2;Layout.fillWidth:true;spacing:Style.space(8)
   RowLayout {
    Layout.fillWidth:true
    T3IconButton{objectName:"projectParentFolder";name:"arrow-up";tooltipText:T3I18n.tr("Parent folder");enabled:!root.locked&&!root.browse.busy&&!!root.browse.parentPath&&root.browse.parentPath!=="/";onClicked:{var p=root.browse.parentPath.replace(/\/$/,"");root.browsePath(p.slice(0,p.lastIndexOf("/")) || "/")}}
    Controls.TextField{id:path;objectName:"projectFolderPath";Layout.fillWidth:true;enabled:!root.locked;selectByMouse:true;onAccepted:root.browsePath(text)}
    T3IconButton{name:"arrow-right-left";tooltipText:T3I18n.tr("Open folder");enabled:!root.locked&&!root.browse.busy;onClicked:root.browsePath(path.text)}
   }
   Text{Layout.fillWidth:true;text:root.browse.busy?T3I18n.tr("Loading…"):root.browse.parentPath || "";color:root.muted;font.pixelSize:Style.font.caption;elide:Text.ElideMiddle;textFormat:Text.PlainText}
   ListView {
    id:folders;objectName:"projectFolders";Layout.fillWidth:true;Layout.preferredHeight:Math.min(Style.space(310),root.parent?root.parent.height*0.38:Style.space(310));clip:true
    model:root.browse.environment===root.environment?root.browse.entries || []:[]
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
   RowLayout{Layout.fillWidth:true;Item{Layout.fillWidth:true}Controls.Button{objectName:"projectOpenFolder";text:T3I18n.tr(root.bridge.managementRequest?"Retry same action":"Open project");enabled:!root.bridge.managementBusy&&(!!root.bridge.managementRequest || (!root.browse.busy&&!root.browse.error&&root.browse.environment===root.environment&&!!root.browse.parentPath));onClicked:root.create()}}
  }
 }
 Connections{target:root.bridge;function onManagementFinished(data){if(!data.error&&data.result&&data.result.action==="add-project")root.close()}}
}
