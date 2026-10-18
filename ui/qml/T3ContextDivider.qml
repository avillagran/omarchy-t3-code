import QtQuick 6.11
import qs.Commons
Item {
 id:root
 property var boundary:({})
 property var catalog:({})
 property bool active:false
 readonly property bool handoff:boundary.kind==="handoff"
 readonly property color tone:boundary.failed?Color.urgent:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.55)
 readonly property string label:T3I18n.tr(handoff?(boundary.active?"Context handoff pending":"Context handoff"):(boundary.active?"Compacting context":"Context compacted"))
 readonly property var endpoints:handoff?(boundary.from || []).concat(boundary.to?[boundary.to]:[]):[]
 function provider(e){return (catalog.providers || []).find(function(p){return p.instanceId===e.instanceId}) || {instanceId:e.instanceId}}
 function modelName(e){var m=(catalog.models || []).find(function(m){return m.instanceId===e.instanceId&&m.model===e.model});return m?(m.shortName || m.displayName || m.model):(e.model || provider(e).name || e.instanceId || "")}
 implicitHeight:content.height+Style.space(8)
 TextMetrics{id:measure;font.pixelSize:Style.font.caption;text:root.label+root.endpoints.map(function(e){return "  "+root.modelName(e)}).join(", ")}
 Rectangle{anchors{left:parent.left;right:content.left;rightMargin:Style.space(10);verticalCenter:parent.verticalCenter}height:1;color:root.tone;opacity:0.18}
 Rectangle{anchors{left:content.right;right:parent.right;leftMargin:Style.space(10);verticalCenter:parent.verticalCenter}height:1;color:root.tone;opacity:0.18}
 Flow {
  id:content;anchors.centerIn:parent;width:Math.max(0,Math.min(root.width-Style.space(40),measure.width+Style.space(26+root.endpoints.length*25)));height:childrenRect.height;spacing:Style.space(6)
  Row {spacing:Style.space(6)
   T3Icon{name:root.handoff?"arrow-right-left":"shrink";implicitWidth:Style.space(12);tint:root.tone;anchors.verticalCenter:parent.verticalCenter}
   T3ActivityText{text:root.label;active:root.active&&root.boundary.active===true;tint:root.tone;visible:root.boundary.active===true}
   Text{visible:root.boundary.active!==true;text:root.label;color:root.tone;font.pixelSize:Style.font.caption;textFormat:Text.PlainText}
  }
  Repeater {
   model:root.endpoints
   Row {required property var modelData;required property int index;spacing:Style.space(4)
    Text{visible:index>0;text:index===root.endpoints.length-1?"→":",";color:root.tone;font.pixelSize:Style.font.caption}
    T3ProviderIcon{provider:root.provider(modelData);tint:root.tone;implicitWidth:Style.space(13);anchors.verticalCenter:parent.verticalCenter}
    Text{text:root.modelName(modelData);color:root.tone;font.pixelSize:Style.font.caption;textFormat:Text.PlainText;elide:Text.ElideRight;width:Math.min(implicitWidth,Math.max(0,content.width-Style.space(30)))}
   }
  }
 }
}
