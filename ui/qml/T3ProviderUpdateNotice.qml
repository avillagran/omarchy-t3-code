import QtQuick 6.11
import QtQuick.Layouts 6.11
import QtQuick.Controls as Controls
import qs.Commons
// Provider (harness) updates, after T3's ProviderUpdateLaunchNotification:
// "Update Available: Claude v2.1.307". Update runs T3's own update command on
// that environment; Settings opens the providers page. Closing with X hides
// this set of versions for good; a newer version shows again.
Rectangle {
 id:root
 objectName:"t3ProviderUpdateNotice"
 required property var bridge
 property var notice:null
 signal settingsRequested()
 readonly property var updates:notice?notice.updates:[]
 readonly property bool single:updates.length===1
 readonly property bool anyOneClick:updates.some(function(u){return u.oneClick})
 readonly property bool remote:updates.some(function(u){return !u.local})
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.65)
 function where(u){return u.local?"":" · "+u.environmentName}
 function catalogProvider(u){return u.provider || ({driverKind:u.driver,instanceId:u.instanceId})}
 function names(){var seen={},out=[];updates.forEach(function(u){if(!seen[u.name]){seen[u.name]=true;out.push(u.name)}});return out.join(", ")}
 function manualNames(){var seen={},out=[];updates.forEach(function(u){if(!u.canUpdate&&!seen[u.name]){seen[u.name]=true;out.push(u.name)}});return out}
 function description(){
  var manual=manualNames(),parts=[]
  if(anyOneClick)parts.push(T3I18n.tr("Install the update now or review provider settings."))
  else if(manual.length<updates.length)parts.push(T3I18n.tr("{names} can be updated from provider settings.",{names:names()}))
  if(manual.length)parts.push(T3I18n.tr(manual.length===1?"Update {names} with the app or tool that installed it.":"Update {names} with the app or tool that installed them.",{names:manual.join(", ")}))
  return parts.join(" ")
 }
 // This app's own request first; otherwise the server's update state, so an
 // update started elsewhere (T3, another client) shows here too.
 function requestText(u){
  var r=root.bridge.providerUpdateRequest(u)
  var state=r?r.state:u.state==="queued"||u.state==="running"?"running":u.state==="failed"?"failed":"",error=r?(r.error?T3I18n.error(r.error):""):(u.stateMessage||"")
  return state==="queued"?T3I18n.tr("{name}: waiting for the other updates",{name:u.name}):state==="pending"||state==="running"?T3I18n.tr("Updating {name}…",{name:u.name}):state==="succeeded"?T3I18n.tr("{name} updated",{name:u.name}):state==="unchanged"?T3I18n.tr("{name} is already up to date",{name:u.name}):state==="failed"?T3I18n.tr("Update failed")+(error?": "+error:""):""
 }
 visible:!!notice
 implicitHeight:column.implicitHeight+Style.space(24)
 radius:Style.space(10);color:Color.popups.background;border.width:1;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.14)
 ColumnLayout {
  id:column;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(12)}spacing:Style.space(6)
  RowLayout {
   Layout.fillWidth:true;spacing:Style.space(10)
   Item {
    Layout.preferredWidth:Style.space(22);Layout.preferredHeight:Style.space(22);Layout.alignment:Qt.AlignTop
    T3ProviderIcon{anchors.fill:parent;provider:root.single?root.catalogProvider(root.updates[0]):({driverKind:"acp"});tint:root.fg}
    Rectangle{width:Style.space(12);height:width;radius:width/2;anchors.right:parent.right;anchors.bottom:parent.bottom;anchors.margins:-Style.space(3);color:T3Palette.done
     T3Icon{anchors.centerIn:parent;name:"download";implicitWidth:Style.space(9);tint:Color.popups.background}}
   }
   ColumnLayout {
    Layout.fillWidth:true;spacing:Style.space(2)
    Text{objectName:"t3ProviderUpdateTitle";Layout.fillWidth:true;text:root.single?T3I18n.tr("Update Available: {name} v{version}",{name:root.updates[0].name,version:root.updates[0].latestVersion})+root.where(root.updates[0]):T3I18n.tr("Updates Available: {count} providers",{count:root.updates.length});color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.Medium;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    Text{Layout.fillWidth:true;text:root.description();color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   }
   T3IconButton{objectName:"t3ProviderUpdateClose";name:"x";implicitWidth:Style.space(22);implicitHeight:implicitWidth;Layout.alignment:Qt.AlignTop;tooltipText:T3I18n.tr("Dismiss");onClicked:root.bridge.closeProviderUpdates(root.notice.key,true)}
  }
  // Several updates: one line each, with its own Update button.
  Repeater {
   model:root.single?[]:root.updates
   RowLayout {
    required property var modelData
    Layout.fillWidth:true;Layout.leftMargin:Style.space(32);spacing:Style.space(8)
    T3ProviderIcon{provider:root.catalogProvider(modelData);tint:root.dim;implicitWidth:Style.space(13)}
    Text{Layout.fillWidth:true;text:modelData.name+" "+modelData.currentVersion+" → "+modelData.latestVersion+root.where(modelData);color:root.fg;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    Text{visible:!!root.requestText(modelData);Layout.maximumWidth:Style.space(180);text:root.requestText(modelData);color:root.dim;font.pixelSize:Style.font.caption;elide:Text.ElideRight;textFormat:Text.PlainText}
    Text{visible:!modelData.canUpdate&&!root.requestText(modelData);text:T3I18n.tr("Manual update");color:root.dim;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
    T3PickerButton{objectName:"t3ProviderUpdateRow";visible:modelData.oneClick&&!root.requestText(modelData);primary:false;outlined:true;implicitHeight:Style.space(24);leftPadding:Style.space(9);pixelSize:Style.font.caption;text:T3I18n.tr("Update");onClicked:root.bridge.updateProvider(modelData)}
   }
  }
  Text{visible:root.single&&!!root.requestText(root.updates[0]);Layout.fillWidth:true;Layout.leftMargin:Style.space(32);text:root.single?root.requestText(root.updates[0]):"";color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  RowLayout {
   Layout.fillWidth:true;Layout.leftMargin:Style.space(32);spacing:Style.space(8)
   T3PickerButton{objectName:"t3ProviderUpdateNow";primary:true;implicitHeight:Style.space(26);leftPadding:Style.space(10);pixelSize:Style.font.caption;visible:root.single&&root.updates[0].oneClick&&!root.requestText(root.updates[0]);text:T3I18n.tr("Update");onClicked:root.bridge.updateProvider(root.updates[0])}
   T3PickerButton{objectName:"t3ProviderUpdateAll";primary:true;implicitHeight:Style.space(26);leftPadding:Style.space(10);pixelSize:Style.font.caption;readonly property var ready:root.bridge.providerUpdates.filter(function(u){return u.oneClick&&!root.requestText(u)});visible:!(root.single&&root.updates[0].oneClick&&ready.length<=1);text:ready.length?T3I18n.tr("Update all ({count})",{count:ready.length}):T3I18n.tr("Update all");hoverEnabled:true;Controls.ToolTip.visible:hovered;Controls.ToolTip.delay:300;Controls.ToolTip.text:ready.length?T3I18n.tr("Every environment, one after another"):T3I18n.tr("T3 cannot update {names} here: they were installed with a version manager or another tool, so update them there.",{names:root.manualNames().join(", ")})+" "+T3I18n.tr("Opens provider settings.");onClicked:{if(ready.length)root.bridge.updateAllProviders(ready);else{root.bridge.closeProviderUpdates(root.notice.key,false);root.settingsRequested()}}}
   T3PickerButton{objectName:"t3ProviderUpdateSettings";primary:false;outlined:true;implicitHeight:Style.space(26);leftPadding:Style.space(10);pixelSize:Style.font.caption;text:T3I18n.tr("Settings");onClicked:{root.bridge.closeProviderUpdates(root.notice.key,false);root.settingsRequested()}}
   Item{Layout.fillWidth:true}
  }
 }
}
