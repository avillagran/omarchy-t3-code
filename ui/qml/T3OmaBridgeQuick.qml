import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
// Content of the OmaBridge bar popup: the state at a glance and the same device
// management as Preferences (T3OmaBridgeDevice), so OmaBridge can be configured
// from the bar. Per-connection routes stay in Preferences.
ColumnLayout {
 id:root
 required property var bridge
 signal settingsRequested()
 readonly property var store:bridge.connectionOmaBridge || ({})
 readonly property var value:store.data || ({})
 readonly property var published:value.published || ({})
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 readonly property alias device:device
 function servicesText(list){return (list || []).map(function(s){return s==="t3"?"T3":s==="ssh"?"SSH":s}).join(", ")}
 function statusText(){
  if(value.available===false)return T3I18n.tr("OmaBridge is not installed. Rerun the installer to add it.")
  if(!value.device)return T3I18n.tr("This device has no OmaBridge identity yet.")
  var peers=(value.peers || []).length
  var state=published.active?T3I18n.tr("Published")+" · "+servicesText(published.services):T3I18n.tr("Not published")
  return state+" · "+T3I18n.tr("{count} paired",{count:peers})
 }
 spacing:Style.space(8)
 Text{Layout.fillWidth:true;text:"OmaBridge";color:root.fg;font.bold:true;font.pixelSize:Style.font.body}
 Text{objectName:"omabridgeIndicatorStatus";Layout.fillWidth:true;text:root.statusText();color:root.muted;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
 T3OmaBridgeDevice{id:device;objectName:"omabridgeIndicatorDevice";Layout.fillWidth:true;bridge:root.bridge;showBarToggle:false}
 Text{Layout.fillWidth:true;visible:!!root.store.error&&root.device.dialogHeight===0;text:root.device.errorText(root.store.error || "");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap}
 RowLayout{Layout.fillWidth:true
  Item{Layout.fillWidth:true}
  T3PickerButton{objectName:"omabridgeIndicatorSettings";primary:false;text:T3I18n.tr("OmaBridge settings…");onClicked:root.settingsRequested()}
 }
}
