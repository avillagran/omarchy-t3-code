import QtQuick 6.11
import QtQuick.Layouts
import qs.Commons
import "../js/HostStyle.js" as HostStyle

RowLayout {
    id: root
    required property var bridge
    required property string environmentId
    property string label: ((bridge.state.environments || []).find(function(e){return e.id===root.environmentId}) || {}).name || environmentId
    readonly property string text: label
    readonly property var look: HostStyle.of((bridge.viewState || {}).hostStyles || {}, environmentId, ((bridge.catalogs || {})[environmentId] || {}).machine)
    readonly property color tint: look.color || Color.accent
    spacing: Style.space(4)
    T3Icon { objectName: "hostBadgeIcon"; name: root.look.icon; tint: root.tint; implicitWidth: Style.space(12) }
    Text { objectName: "hostBadgeName"; Layout.maximumWidth: Style.space(120); text: root.label; color: root.tint; font.pixelSize: Style.font.caption; elide: Text.ElideRight; textFormat: Text.PlainText }
}
