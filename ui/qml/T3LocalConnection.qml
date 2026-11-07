import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
ColumnLayout{
 id:root
 required property var bridge
 readonly property color fg:Color.popups.text
 readonly property color muted:Qt.rgba(fg.r,fg.g,fg.b,0.58)
 spacing:Style.space(6)
  RowLayout{Layout.fillWidth:true
   T3Icon{name:"laptop";tint:root.fg}
   Text{Layout.fillWidth:true;text:((((root.bridge.state || {}).environments || []).find(function(e){return e.id==="local"}) || {}).name || "T3 Code")+" · "+T3I18n.tr("Local");color:root.fg;font.pixelSize:Style.font.body}
   T3Switch{objectName:"connectionLocalEnabled";checked:(root.bridge.connectionSettings || {}).localEnabled!==false;enabled:!(root.bridge.connectionSettings || {}).busy&&!!(root.bridge.connectionSettings || {}).revision;onToggled:root.bridge.manageConnections("local-enabled",{enabled:checked})}
  }
  RowLayout{Layout.fillWidth:true
   Text{Layout.fillWidth:true;text:(root.bridge.localBackend || {}).owner || "";color:root.muted;font.pixelSize:Style.font.caption}
   T3PickerButton{primary:false;objectName:"connectionStartBackend";text:T3I18n.tr((root.bridge.backendStart || {}).busy?"Loading…":"Start local backend");visible:!(root.bridge.localBackend || {}).running;enabled:!!(root.bridge.localBackend || {}).canControl&&!(root.bridge.backendStart || {}).busy;onClicked:root.bridge.controlBackend("start")}
   T3IconButton{name:"refresh-cw";tooltipText:T3I18n.tr("Restart local backend");visible:!!(root.bridge.localBackend || {}).canControl;enabled:!(root.bridge.backendStart || {}).busy;onClicked:root.bridge.controlBackend("restart")}
   T3IconButton{name:"square";tint:Color.urgent;tooltipText:T3I18n.tr("Stop local backend");visible:!!(root.bridge.localBackend || {}).canControl;enabled:!(root.bridge.backendStart || {}).busy;onClicked:root.bridge.controlBackend("stop")}
  }
  Text{objectName:"connectionBackendError";Layout.fillWidth:true;visible:!!text;text:T3I18n.error((root.bridge.backendStart || {}).error || (root.bridge.localBackend || {}).error || "");color:Color.urgent;wrapMode:Text.Wrap;font.pixelSize:Style.font.caption}
 }
