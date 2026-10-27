import QtQuick 6.11
import QtQuick.Effects

// Loaded only for registry artwork. The layer stays alive across async reloads;
// never toggle it during Qt's DPR traversal (which snapshots raw child pointers).
Item {
 id:root
 property url source
 property color tint
 property real rasterScale:1
 readonly property bool ready:registry.status===Image.Ready
 Image{id:registry;objectName:"t3ProviderRegistry";anchors.fill:parent;source:root.source;asynchronous:true;cache:true;fillMode:Image.PreserveAspectFit;visible:false;layer.enabled:true;sourceSize.width:Math.round(root.width*root.rasterScale);sourceSize.height:Math.round(root.height*root.rasterScale)}
 MultiEffect{anchors.fill:parent;source:registry;visible:root.ready;colorization:1;colorizationColor:root.tint;opacity:root.tint.a}
}
