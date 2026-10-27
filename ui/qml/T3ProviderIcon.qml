import QtQuick 6.11
import QtQuick.Window 6.11
import qs.Commons
Item {
 id:root
 readonly property real displayScale:Screen.devicePixelRatio
 property real rasterScale:1
 function updateRasterScale(){rasterScale=Math.max(0.5,displayScale || 1)}
 onDisplayScaleChanged:Qt.callLater(root.updateRasterScale)
 Component.onCompleted:{Qt.callLater(root.updateRasterScale);if(iconSource)artworkCreated=true}
 property var provider:({})
 readonly property string iconSource:provider.registryIcon || ""
 property bool artworkCreated:false
 onIconSourceChanged:if(iconSource)artworkCreated=true
 property color tint:Color.popups.text
 implicitWidth:Style.space(16);implicitHeight:implicitWidth
 function glyph(){var kind=provider.driverKind || provider.instanceId || "";return /codex|openai/.test(kind)?"openai":/claude|anthropic/.test(kind)?"claude":/opencode/.test(kind)?"opencode":/cursor/.test(kind)?"cursor":"acp"}
 T3Icon{anchors.fill:parent;name:root.glyph();tint:root.tint;visible:!artwork.item||!artwork.item.ready}
 Loader {
  id:artwork;anchors.fill:parent;active:root.artworkCreated
  onActiveChanged:if(active)setSource(Qt.resolvedUrl("T3TintedProviderIcon.qml"),{source:Qt.binding(function(){return root.provider.registryIcon || ""}),tint:Qt.binding(function(){return root.tint}),rasterScale:Qt.binding(function(){return root.rasterScale})});else source=""
 }
}
