import QtQuick 6.11
import QtQuick.Layouts
import QtQuick.Controls as Controls
import qs.Commons

// An agent's request_secret, from T3's SecretRequestCard. The value goes straight to the
// server, which keeps it under a one-use reference; the thread only learns the outcome.
Rectangle {
 id:root;objectName:"t3SecretRequest"
 required property var bridge
 property var request:({})
 readonly property color fg:Color.popups.text
 readonly property bool pending:request.status==="pending"&&request.local!==false
 readonly property var operation:(bridge.secretAnswers||{})[request.turnItemId]||({})
 implicitHeight:pending?form.implicitHeight+Style.space(28):outcome.implicitHeight+Style.space(12)
 radius:Style.space(12)
 color:pending?Qt.rgba(fg.r,fg.g,fg.b,0.04):"transparent"
 border.width:pending?1:0;border.color:Qt.rgba(fg.r,fg.g,fg.b,0.12)
 RowLayout{
  id:outcome;visible:!root.pending;anchors.left:parent.left;anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;spacing:Style.space(8)
  T3Icon{name:root.request.status==="saved"?"check":root.request.status==="declined"?"minus":"lock";implicitWidth:Style.space(14);tint:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.6)}
  Text{Layout.fillWidth:true;wrapMode:Text.Wrap;textFormat:Text.PlainText;color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.7);font.pixelSize:Style.font.caption
   text:String(root.request.label||"")+" · "+T3I18n.tr(root.request.status==="pending"?"Waiting for an answer in the original thread":root.request.status==="saved"?"Saved securely and kept private":root.request.status==="declined"?"Declined":"Request ended")}
 }
 ColumnLayout{
  id:form;visible:root.pending;anchors.fill:parent;anchors.margins:Style.space(14);spacing:Style.space(10)
  RowLayout{spacing:Style.space(8)
   T3Icon{name:"lock";implicitWidth:Style.space(15);tint:Color.accent}
   Text{Layout.fillWidth:true;text:String(root.request.label||"");color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.DemiBold;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  }
  Text{visible:!!String(root.request.reason||"").trim();Layout.fillWidth:true;text:String(root.request.reason||"");color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.7);font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  RowLayout{Layout.fillWidth:true;spacing:Style.space(8)
   Controls.TextField{
    id:secret;objectName:"t3SecretInput";Layout.fillWidth:true;echoMode:TextInput.Password;passwordCharacter:"•";inputMethodHints:Qt.ImhSensitiveData|Qt.ImhNoPredictiveText|Qt.ImhNoAutoUppercase
    enabled:!root.operation.busy;placeholderText:String(root.request.placeholder||T3I18n.tr("Paste the secret"));color:root.fg;font.pixelSize:Style.font.caption
    background:Rectangle{radius:Style.space(6);color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.05);border.width:1;border.color:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.15)}
    onAccepted:save.clicked()
   }
   T3PickerButton{id:save;objectName:"t3SecretSave";implicitHeight:Style.space(30);text:T3I18n.tr("Save");enabled:!root.operation.busy&&!!secret.text.trim();onClicked:{root.bridge.answerSecret(root.request,{type:"save",secret:secret.text.trim()});secret.text=""}}
   T3PickerButton{objectName:"t3SecretDecline";implicitHeight:Style.space(30);primary:false;text:T3I18n.tr("Decline");enabled:!root.operation.busy;onClicked:root.bridge.answerSecret(root.request,{type:"decline"})}
  }
  RowLayout{spacing:Style.space(6)
   T3Icon{name:"lock";implicitWidth:Style.space(11);tint:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.5)}
   Text{Layout.fillWidth:true;text:root.operation.error?T3I18n.error(root.operation.error):T3I18n.tr("Stored securely, never shown to the agent");color:root.operation.error?Color.urgent:Qt.rgba(root.fg.r,root.fg.g,root.fg.b,0.5);font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
  }
 }
}
