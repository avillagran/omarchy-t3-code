import QtQuick
import qs.Commons
Rectangle {
 id:root
 property var project:({})
 property string monogram:"PR"
 property string projectKey:""
 property var projects:[]
 readonly property var overrideIcon:project.projectIcon || ({})
 readonly property color tint:T3Palette.pick(overrideIcon.color || "",T3Palette.projectColor(projectKey,projects.length?projects:[{key:projectKey}]))
 readonly property string label:overrideIcon.kind==="emoji"?overrideIcon.emoji || "":overrideIcon.kind==="monogram"?overrideIcon.text || monogram:monogram
 readonly property string svg:overrideIcon.kind==="lucide"?(T3ProjectIcons.icons[overrideIcon.name] || ""):""
 implicitWidth:Style.space(16);implicitHeight:implicitWidth
 radius:Style.space(4)
 clip:true
 color:overrideIcon.kind==="emoji"||overrideIcon.kind==="lucide"?"transparent":Qt.rgba(tint.r,tint.g,tint.b,0.14)
 Text{anchors.centerIn:parent;visible:root.overrideIcon.kind!=="lucide";text:root.label;font.pixelSize:root.height*(root.overrideIcon.kind==="emoji"?0.8:0.5);font.bold:root.overrideIcon.kind!=="emoji";color:root.tint;textFormat:Text.PlainText}
 Image{anchors.fill:parent;visible:!!root.svg;source:root.svg?"data:image/svg+xml,"+encodeURIComponent(root.svg.replace(/currentColor/g,String(root.tint))):"";sourceSize:Qt.size(32,32);fillMode:Image.PreserveAspectFit}
 T3Icon{anchors.fill:parent;name:"folder";tint:root.tint;visible:root.overrideIcon.kind==="lucide"&&!root.svg}
}
