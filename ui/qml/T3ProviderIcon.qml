import QtQuick 6.11
import QtQuick.Window 6.11
import QtQuick.Effects
import qs.Commons
Item {
 id:root
 readonly property real displayScale:Screen.devicePixelRatio
 property real rasterScale:1
 function updateRasterScale(){rasterScale=Math.max(0.5,displayScale || 1)}
 onDisplayScaleChanged:Qt.callLater(root.updateRasterScale)
 Component.onCompleted:Qt.callLater(root.updateRasterScale)
 property var provider:({})
 property color tint:Color.popups.text
 implicitWidth:Style.space(16);implicitHeight:implicitWidth
 function glyph(){var kind=provider.driverKind || provider.instanceId || "";return /codex|openai/.test(kind)?"openai":/claude|anthropic/.test(kind)?"claude":/opencode/.test(kind)?"opencode":/cursor/.test(kind)?"cursor":"acp"}
 T3Icon{anchors.fill:parent;name:root.glyph();tint:root.tint;visible:registry.status!==Image.Ready}
 // Keep the texture layer alive across async reloads. Qt's DPR traversal
 // snapshots raw child pointers; toggling the layer inside it can destroy children.
 Image{id:registry;objectName:"t3ProviderRegistry";anchors.fill:parent;source:root.provider.registryIcon || "";asynchronous:true;cache:true;fillMode:Image.PreserveAspectFit;visible:false;layer.enabled:true;sourceSize.width:Math.round(root.width*root.rasterScale);sourceSize.height:Math.round(root.height*root.rasterScale)}
 MultiEffect{anchors.fill:parent;source:registry;visible:registry.status===Image.Ready;colorization:1;colorizationColor:root.tint;opacity:root.tint.a}
}
