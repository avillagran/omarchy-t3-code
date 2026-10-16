import QtQuick
import QtQuick.Controls as Controls
import qs.Commons
Rectangle {
 id:root
 required property var bridge
 required property var attachment
 property bool active:false
 signal enlarge(var attachment)
 readonly property string key:bridge.imageKey(attachment)
 readonly property var imageState:bridge.imageStates[key] || ({})
 function request(){if(active)bridge.requestImage(attachment)}
 onActiveChanged:request()
 onAttachmentChanged:Qt.callLater(request)
 Component.onCompleted:request()
 Connections{target:root.bridge;function onImageStatesChanged(){if(root.active&&!root.bridge.imageStates[root.key])Qt.callLater(root.request)}}
 implicitWidth:Style.space(220);implicitHeight:Style.space(160)
 radius:Style.space(8);color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.04)
 border.width:1;border.color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.12)
 clip:true
 Image {
  objectName:"t3MessageThumbnail"
  anchors.fill:parent;anchors.margins:Style.space(4)
  source:root.active?root.imageState.url || "":""
  asynchronous:true;cache:false;sourceSize:Qt.size(640,480);fillMode:Image.PreserveAspectFit
 }
 WorkIndicator{anchors.centerIn:parent;visible:!!root.imageState.loading;active:visible&&root.active}
 T3Icon{anchors.centerIn:parent;name:root.imageState.error?"circle-alert":"file";visible:!root.imageState.url&&!root.imageState.loading;tint:root.imageState.error?Color.urgent:Color.muted}
 activeFocusOnTab:true
 Accessible.role:Accessible.Button
 Accessible.name:attachment.name
 Keys.onReturnPressed:root.enlarge(root.attachment)
 Keys.onSpacePressed:root.enlarge(root.attachment)
 MouseArea{id:mouse;anchors.fill:parent;hoverEnabled:true;cursorShape:Qt.PointingHandCursor;onClicked:root.enlarge(root.attachment)}
 Controls.ToolTip.visible:mouse.containsMouse
 Controls.ToolTip.text:root.imageState.error?T3I18n.error(root.imageState.error):root.attachment.name
}
