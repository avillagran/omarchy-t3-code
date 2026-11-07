import QtQuick 6.11
import QtQuick.Layouts 6.11
import qs.Commons
// Connected hosts where providers are signed out, one card per host. Sign-ins
// belong to each host: a host just connected starts signed out, so agents
// cannot run there until each provider signs in on it. A provider T3 signs in
// itself opens that host's provider settings; Codex and Claude open a terminal
// running their CLI's login on that host, and the card waits for it.
ColumnLayout {
 id:root
 objectName:"t3ProviderSignInNotice"
 required property var bridge
 signal signInRequested(string environment)
 readonly property var hosts:bridge.providerSignIns || []
 readonly property color fg:Color.popups.text
 readonly property color dim:Qt.rgba(fg.r,fg.g,fg.b,0.65)
 visible:hosts.length>0
 spacing:Style.space(8)
 Repeater {
  model:root.hosts
  Rectangle {
   id:card
   required property var modelData
   objectName:"t3ProviderSignIn_"+modelData.environment
   Layout.fillWidth:true
   implicitHeight:body.implicitHeight+Style.space(24)
   radius:Style.space(10);color:Color.popups.background;border.width:1;border.color:T3Palette.waiting
   ColumnLayout {
    id:body;anchors{left:parent.left;right:parent.right;top:parent.top;margins:Style.space(12)}spacing:Style.space(6)
    RowLayout {
     Layout.fillWidth:true;spacing:Style.space(8)
     T3Icon{name:"lock";tint:T3Palette.waiting;implicitWidth:Style.space(16)}
     Text{Layout.fillWidth:true;text:T3I18n.tr("Sign in needed · {host}",{host:card.modelData.name});color:root.fg;font.pixelSize:Style.font.body;font.weight:Font.Medium;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    }
    Text{objectName:"t3ProviderSignInNames_"+card.modelData.environment;Layout.fillWidth:true;text:T3I18n.tr("{providers} cannot run agents on this host until you sign in there. Each host keeps its own sign-in.",{providers:card.modelData.providers.map(function(p){return p.name}).join(", ")});color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    RowLayout {
     Layout.fillWidth:true;spacing:Style.space(8)
     Flow {
      Layout.fillWidth:true;spacing:Style.space(8)
      Repeater {
       model:card.modelData.providers
       T3PickerButton {
        required property var modelData
        readonly property var login:root.bridge.providerLogin(card.modelData.environment,modelData.instanceId)
        objectName:"t3ProviderSignInOpen_"+card.modelData.environment+"_"+modelData.instanceId
        text:login&&!login.error?T3I18n.tr("Waiting for {provider}…",{provider:modelData.name}):T3I18n.tr("Sign in to {provider}…",{provider:modelData.name})
        primary:true
        onClicked:if(modelData.method==="cli")root.bridge.signInProvider(card.modelData.environment,modelData);else root.signInRequested(card.modelData.environment)
       }
      }
      T3PickerButton{objectName:"t3ProviderSignInLater_"+card.modelData.environment;text:T3I18n.tr("Not now");primary:false;outlined:true;onClicked:root.bridge.dismissProviderSignIn(card.modelData.environment,card.modelData.key)}
     }
    }
    Text{Layout.fillWidth:true;visible:card.modelData.providers.some(function(p){var l=root.bridge.providerLogin(card.modelData.environment,p.instanceId);return l&&!l.error});text:T3I18n.tr("Finish the sign-in in the terminal that opened. This card closes by itself once it is done.");color:root.dim;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
    Text{objectName:"t3ProviderSignInError_"+card.modelData.environment;Layout.fillWidth:true;visible:!!text;text:card.modelData.providers.map(function(p){var l=root.bridge.providerLogin(card.modelData.environment,p.instanceId);return l&&l.error?p.name+": "+(l.error==="ssh_environment_required"?T3I18n.tr("Sign in on that host directly: this connection has no SSH."):T3I18n.error(l.error)):""}).filter(function(t){return !!t}).join("\n");color:Color.urgent;font.pixelSize:Style.font.caption;wrapMode:Text.Wrap;textFormat:Text.PlainText}
   }
  }
 }
}
