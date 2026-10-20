import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
// Content of the OmaBridge bar popup: the same OmaBridge page as Preferences
// and the Omarchy Control Panel (T3OmaBridgeManage), on the same device.
// Per-connection routes stay in Preferences.
ColumnLayout {
 id:root
 required property var bridge
 signal settingsRequested()
 readonly property alias manage:manage
 readonly property color fg:Color.popups.text
 function servicesText(list){return (list || []).map(function(s){return s==="t3"?"T3":s==="ssh"?"SSH":s}).join(", ")}
 function statusText(){
  var value=manage.snapshot
  if(!value)return manage.pageState==="missing"?T3I18n.tr("OmaBridge is not installed. Rerun the installer to add it."):"OmaBridge"
  if(!value.device)return T3I18n.tr("This device has no OmaBridge identity yet.")
  var published=value.published || ({})
  var state=published.active?T3I18n.tr("Published")+" · "+servicesText(published.services):T3I18n.tr("Not published")
  return state+" · "+T3I18n.tr("{count} paired",{count:(value.peers || []).length})
 }
 spacing:Style.space(8)
 T3OmaBridgeManage{id:manage;objectName:"omabridgeIndicatorManage";Layout.fillWidth:true;bridge:root.bridge}
 RowLayout{Layout.fillWidth:true
  Item{Layout.fillWidth:true}
  T3PickerButton{objectName:"omabridgeIndicatorSettings";primary:false;text:T3I18n.tr("OmaBridge settings…");onClicked:root.settingsRequested()}
 }
}
