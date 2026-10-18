import QtQuick 6.11
import QtQuick.Controls as Controls
import QtQuick.Layouts
import qs.Commons
Controls.Button {
 id:root
 objectName:"t3PermissionPicker"
 required property var bridge
 property alias popup:menu
 signal picked()
 readonly property var definitions:[
  {id:"approval-required",label:"Supervised",description:"Ask before commands and file changes.",icon:"lock"},
  {id:"auto-accept-edits",label:"Auto-accept edits",description:"Auto-approve edits, ask before other actions.",icon:"pen-line"},
  {id:"auto",label:"Auto",description:"Supported providers approve routine actions; others still ask.",icon:"sparkles"},
  {id:"full-access",label:"Full access",description:"Allow commands and edits without prompts.",icon:"lock-open"}
 ]
 readonly property string storedMode:bridge.draft.runtimeMode || bridge.selectedInfo.runtimeMode || "approval-required"
 readonly property string mode:modes.some(function(d){return d.id===root.storedMode})?storedMode:(modes[0] || definitions[0]).id
 readonly property var current:definitions.find(function(d){return d.id===root.mode}) || definitions[0]
 readonly property var provider:(bridge.catalog.providers || []).find(function(p){return p.instanceId===(root.bridge.draft.modelSelection || root.bridge.selectedInfo.modelSelection || {}).instanceId}) || ({})
 readonly property var modes:definitions.filter(function(d){return !(root.provider.supportedRuntimeModes || []).length || root.provider.supportedRuntimeModes.indexOf(d.id)>=0})
 implicitHeight:Style.space(30);implicitWidth:permissionMetrics.width+Style.space(54)
 TextMetrics{id:permissionMetrics;text:T3I18n.tr(root.current.label);font.pixelSize:Style.font.caption}
 leftPadding:Style.space(8);rightPadding:leftPadding
 hoverEnabled:true
 contentItem:RowLayout {
  spacing:Style.space(6)
  T3Icon{name:root.current.icon;implicitWidth:Style.space(15);opacity:root.enabled?0.75:0.4}
  Text{Layout.fillWidth:true;text:T3I18n.tr(root.current.label);color:Color.popups.text;font.pixelSize:Style.font.caption;elide:Text.ElideRight;visible:root.width>Style.space(70);opacity:root.enabled?1:0.4}
  T3Icon{name:"chevron-down";implicitWidth:Style.space(11);opacity:root.enabled?0.5:0.3}
 }
 background:Rectangle{radius:Style.space(6);color:root.hovered?Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.07):"transparent";border.width:root.activeFocus?1:0;border.color:Color.accent}
 Controls.ToolTip.visible:hovered&&!menu.opened
 Controls.ToolTip.text:T3I18n.tr("Thread permissions")+"\n"+T3I18n.tr(current.description)
 Controls.ToolTip.delay:400
 function choose(mode){if(!enabled||!modes.some(function(d){return d.id===mode}))return;bridge.chooseRuntimeMode(mode);menu.close();picked()}
 onClicked:menu.open()
 Controls.Popup {
  id:menu;objectName:"t3PermissionMenu"
  x:0;y:-height-Style.space(6);width:Style.space(350);height:modeList.contentHeight+Style.space(12)
  padding:Style.space(6);modal:true;focus:true;margins:Style.space(8)
  closePolicy:Controls.Popup.CloseOnEscape|Controls.Popup.CloseOnPressOutside
  background:Rectangle{color:Color.popups.background;radius:Style.space(9);border.width:1;border.color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.15)}
  onOpened:{modeList.currentIndex=Math.max(0,root.modes.findIndex(function(d){return d.id===root.mode}));modeList.forceActiveFocus()}
  contentItem:ListView {
   id:modeList;model:root.modes;implicitHeight:contentHeight;clip:true;keyNavigationEnabled:true;activeFocusOnTab:true
   Keys.onReturnPressed:root.choose(root.modes[currentIndex].id)
   Keys.onEnterPressed:root.choose(root.modes[currentIndex].id)
   Keys.onSpacePressed:root.choose(root.modes[currentIndex].id)
   delegate:Controls.ItemDelegate {
    required property var modelData;required property int index
    objectName:"permissionMode_"+modelData.id;width:modeList.width;implicitHeight:Math.max(Style.space(62),details.implicitHeight+Style.space(20));padding:Style.space(10)
    highlighted:modeList.currentIndex===index;hoverEnabled:true
    background:Rectangle{radius:Style.space(6);color:parent.hovered||parent.highlighted?Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.065):"transparent"}
    contentItem:RowLayout {
     spacing:Style.space(10)
     T3Icon{name:modelData.icon;implicitWidth:Style.space(16);opacity:0.6}
     ColumnLayout{id:details;Layout.fillWidth:true;spacing:Style.space(3)
      Text{Layout.fillWidth:true;text:T3I18n.tr(modelData.label);color:Color.popups.text;font.pixelSize:Style.font.body}
      Text{Layout.fillWidth:true;text:T3I18n.tr(modelData.description);color:Color.popups.text;opacity:0.55;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
     }
     T3Icon{name:"check";visible:root.mode===modelData.id;tint:Color.accent}
    }
    onClicked:root.choose(modelData.id)
   }
  }
 }
}
