import QtQuick
import QtQuick.Window
import QtQuick.Effects
import qs.Commons
Item {
 id:root
 property var provider:({})
 property color tint:Color.popups.text
 implicitWidth:Style.space(16);implicitHeight:implicitWidth
 function glyph(){var kind=provider.driverKind || provider.instanceId || "";return /codex|openai/.test(kind)?"openai":/claude|anthropic/.test(kind)?"claude":/opencode/.test(kind)?"opencode":/cursor/.test(kind)?"cursor":"acp"}
 T3Icon{anchors.fill:parent;name:root.glyph();tint:root.tint;visible:registry.status!==Image.Ready}
 Image{id:registry;anchors.fill:parent;source:root.provider.registryIcon || "";asynchronous:true;cache:true;fillMode:Image.PreserveAspectFit;visible:false;layer.enabled:status===Image.Ready;sourceSize.width:Math.round(root.width*Screen.devicePixelRatio);sourceSize.height:Math.round(root.height*Screen.devicePixelRatio)}
 MultiEffect{anchors.fill:parent;source:registry;visible:registry.status===Image.Ready;colorization:1;colorizationColor:root.tint;opacity:root.tint.a}
}
