import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
import "../js/ConnectionStatus.js" as Status
// Persistent, non-modal connection failures. Recovery removes the notice;
// retrying never hides the reported reason behind a spinner.
ColumnLayout {
 id:root
 objectName:"t3ConnectionNotice"
 required property var bridge
 signal settingsRequested()
 readonly property var failures:Status.failures(bridge)
 visible:failures.length>0
 spacing:Style.space(6)
 Repeater {
  model:root.failures
  Rectangle {
   id:card
   required property var modelData
   objectName:"t3ConnectionFailure_"+modelData.id
   Layout.fillWidth:true
   implicitHeight:body.implicitHeight+Style.space(20)
   radius:Style.space(8);color:Color.popups.background;border.width:1;border.color:Color.urgent
   ColumnLayout {
    id:body;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(10)}spacing:Style.space(5)
    RowLayout {
     Layout.fillWidth:true;spacing:Style.space(8)
     T3Icon{name:"circle-alert";tint:Color.urgent;implicitWidth:Style.space(16)}
     Text{Layout.fillWidth:true;text:T3I18n.tr("Connection problem · {host}",{host:card.modelData.name || card.modelData.id});color:Color.popups.text;font.pixelSize:Style.font.body;font.weight:Font.Medium;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    }
    Text{objectName:"connectionReason_"+card.modelData.id;Layout.fillWidth:true;text:Status.reason(card.modelData.state)?T3I18n.tr(Status.reason(card.modelData.state)):T3I18n.error(card.modelData.state);color:Color.popups.text;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    Text{Layout.fillWidth:true;text:T3I18n.tr("Check the host, network and access in Connections. O3 retries automatically.");color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.65);font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    TextEdit{objectName:"connectionCode_"+card.modelData.id;Layout.fillWidth:true;text:card.modelData.state;readOnly:true;selectByMouse:true;color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.65);font.pixelSize:Style.font.caption;wrapMode:TextEdit.Wrap;textFormat:TextEdit.PlainText;Accessible.name:T3I18n.tr("Error code")}
    RowLayout {
     Layout.fillWidth:true;spacing:Style.space(8)
     T3PickerButton{objectName:"connectionRetry_"+card.modelData.id;text:T3I18n.tr("Reconnect");primary:true;enabled:!(root.bridge.connectionSettings || {}).busy;onClicked:{if(card.modelData.id!=="local"&&card.modelData.id!=="client")root.bridge.manageConnections("reconnect",{environmentId:card.modelData.id});root.bridge.refresh()}}
     T3PickerButton{objectName:"connectionSettings_"+card.modelData.id;text:T3I18n.tr("Connections");primary:false;outlined:true;onClicked:root.settingsRequested()}
     Item{Layout.fillWidth:true}
    }
   }
  }
 }
}
