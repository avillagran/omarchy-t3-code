import QtQuick
import QtQuick.Controls as Controls
import qs.Commons
import "../js/TimelineModel.js" as Timeline

Item {
    id: root
    objectName: "t3TimelineMinimap"
    required property var rows
    required property var view
    property bool active: true
    property string threadKey: ""
    readonly property var items: Timeline.derive(rows)
    property int hoverIndex: -1
    property int currentIndex: -1
    property var visibleMarkers: ({})
    property bool refreshQueued: false
    readonly property var preview: hoverIndex >= 0 && hoverIndex < items.length ? items[hoverIndex] : null
    readonly property real stripWidth: Math.min(Style.space(14), Math.max(0, view.x - Style.space(6)))
    readonly property real railHeight: Math.min(Math.max(1, (items.length - 1) * Style.space(8)), Math.max(1, height - Style.space(64)))
    visible: active && items.length >= 2 && stripWidth > 0
    signal selected(var entry)
    signal wheelRequested(real angle, real pixels, bool touchpad)

    function queueViewport() {
        if (refreshQueued || !active) return
        refreshQueued = true
        Qt.callLater(function() { root.refreshQueued = false; if (root.active) root.updateViewport() })
    }
    function updateViewport() {
        var top = view.contentY, bottom = top + view.height, visibleRows = {}, first = rows.length
        // Inspect instantiated delegates only. Never force layout, instantiate offscreen
        // rows or write to contentY while tracking the reader's position.
        var children = view.contentItem.children
        for (var i = 0; i < children.length; ++i) {
            var child = children[i]
            if (!child.hasOwnProperty("row") || !child.row || child.y >= bottom || child.y + child.height <= top) continue
            visibleRows[child.row.id] = true
            var index = rows.findIndex(function(r) { return r.id === child.row.id })
            if (index >= 0) first = Math.min(first, index)
        }
        var markers = {}, current = -1, preceding = -1
        for (var n = 0; n < items.length; ++n) {
            if (visibleRows[items[n].id]) { markers[items[n].id] = true; if (current < 0) current = n }
            if (items[n].rowIndex <= first) preceding = n
        }
        if (current < 0) current = preceding
        if (view.atYEnd && first === rows.length) current = items.length - 1
        if (currentIndex !== current) currentIndex = current
        if (JSON.stringify(visibleMarkers) !== JSON.stringify(markers)) visibleMarkers = markers
    }
    function choose(index) { if (index >= 0 && index < items.length) selected(items[index]) }
    function moveHover(delta) { hoverIndex = Math.max(0, Math.min(items.length - 1, (hoverIndex < 0 ? Math.max(0,currentIndex) : hoverIndex) + delta)) }
    onItemsChanged: { if (hoverIndex >= items.length) hoverIndex = -1; queueViewport() }
    onThreadKeyChanged: { hoverIndex = -1; currentIndex = -1; visibleMarkers = ({}); queueViewport() }
    onActiveChanged: { if (!active) hoverIndex = -1; else queueViewport() }
    Connections {
        target: root.view
        function onContentYChanged() { root.queueViewport() }
        function onContentHeightChanged() { root.queueViewport() }
        function onHeightChanged() { root.queueViewport() }
        function onCountChanged() { root.queueViewport() }
    }
    Component.onCompleted: queueViewport()

    Item {
        id: rail
        objectName: "t3TimelineRail"
        x: Style.space(6)
        y: (root.height - height) / 2
        width: root.preview ? Math.min(root.width - x, Style.space(354)) : root.stripWidth
        height: root.railHeight
        activeFocusOnTab: root.visible
        Accessible.role: Accessible.Slider
        Accessible.name: T3I18n.tr("Conversation timeline")
        Accessible.description: root.preview ? Timeline.compact(root.preview.userText) : ""
        onActiveFocusChanged: { if (activeFocus) root.hoverIndex = Math.max(0,root.currentIndex); else if (!pointer.containsMouse) root.hoverIndex = -1 }
        Keys.onUpPressed: root.moveHover(-1)
        Keys.onDownPressed: root.moveHover(1)
        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Home) {root.hoverIndex=0;event.accepted=true}
            else if(event.key===Qt.Key_End){root.hoverIndex=root.items.length-1;event.accepted=true}
        }
        Keys.onReturnPressed: root.choose(root.hoverIndex)
        Keys.onEnterPressed: root.choose(root.hoverIndex)
        Keys.onSpacePressed: root.choose(root.hoverIndex)
        Keys.onEscapePressed: { root.hoverIndex = -1; root.view.forceActiveFocus() }

        Rectangle { x: Style.space(3); width: 1; height: parent.height; color: Color.popups.text; opacity: 0.06 }
        Repeater {
            // Keep delegates alive while an assistant response streams.
            model: root.items.length
            delegate: Item {
                id: mark
                objectName: "t3TimelineMarker" + index
                required property int index
                readonly property var entry: root.items[index]
                readonly property int distance: root.hoverIndex < 0 ? 99 : Math.abs(index-root.hoverIndex)
                x: 0; y: root.items.length <= 1 ? 0 : index/(root.items.length-1)*rail.height
                width: Style.space(24); height: Style.space(2)
                property real stretch: distance === 0 ? 1 : distance === 1 ? 0.667 : distance === 2 ? 0.417 : 0.333
                transform: Scale { xScale: mark.stretch }
                Behavior on stretch { NumberAnimation { duration: 150 } }
                Rectangle { anchors.fill: parent; radius: height/2; color: Color.popups.text; opacity: parent.distance === 0 ? 0.75 : 0.35 }
                Rectangle { anchors.fill: parent; radius: height/2; color: Color.popups.text; opacity: entry && root.visibleMarkers[entry.id] ? 0.9 : 0; Behavior on opacity { NumberAnimation { duration: 150 } } }
            }
        }
        MouseArea {
            id: pointer
            objectName: "t3TimelinePointer"
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: mouseX <= Style.space(24) ? Qt.PointingHandCursor : Qt.ArrowCursor
            onPositionChanged: function(mouse) { if (mouse.x <= Style.space(24)) root.hoverIndex = Timeline.indexAt(mouse.y,height,root.items.length) }
            onEntered: root.hoverIndex = Timeline.indexAt(mouseY,height,root.items.length)
            onExited: if (!rail.activeFocus) root.hoverIndex = -1
            onClicked: function(mouse) { if(mouse.x <= Style.space(24)) root.choose(Timeline.indexAt(mouse.y,height,root.items.length)) }
            onWheel: function(event) { event.accepted=true; root.wheelRequested(event.angleDelta.y,event.pixelDelta.y,event.device.type===PointerDevice.TouchPad) }
        }
        Rectangle {
            id: previewCard
            objectName: "t3TimelinePreview"
            visible: !!root.preview
            x: Style.space(32)
            y: Math.max(-rail.y + Style.space(4), Math.min(root.height-rail.y-height-Style.space(4), root.hoverIndex/Math.max(1,root.items.length-1)*rail.height - (root.hoverIndex===0 ? 0 : root.hoverIndex===root.items.length-1 ? height : height/2)))
            width: Math.max(1,Math.min(Style.space(320),root.width-rail.x-x-Style.space(8)))
            height: previewText.implicitHeight + Style.space(24)
            radius: Style.space(12)
            color: Color.popups.background
            border.width: 1; border.color: Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.15)
            FontMetrics { id: previewFont; font.pixelSize: Style.font.body }
            Column {
                id: previewText
                anchors { left: parent.left; right: parent.right; top: parent.top; margins: Style.space(12) }
                spacing: Style.space(4)
                Text { width: parent.width; text: root.preview ? Timeline.compact(root.preview.userText) || T3I18n.tr("You") : ""; color: Color.popups.text; font.pixelSize: Style.font.body; font.bold: true; textFormat: Text.PlainText; elide: Text.ElideRight }
                TextEdit { visible: text.length>0; width:parent.width; height:Math.min(implicitHeight,previewFont.height+previewFont.lineSpacing*2); clip:true; text:root.preview?Timeline.compact(root.preview.assistantText):"";textFormat:TextEdit.PlainText;readOnly:true;selectByMouse:true;wrapMode:TextEdit.Wrap;color:Qt.rgba(Color.popups.text.r,Color.popups.text.g,Color.popups.text.b,0.6);font.pixelSize:Style.font.body }
            }
        }
        T3IconButton {
            objectName: "t3TimelinePrevious"
            implicitWidth:Style.space(20);implicitHeight:implicitWidth
            HoverHandler { id: previousHover }
            x: Style.space(4)-width/2; y: -height-Style.space(4)
            name: "chevron-up"; tooltipText:T3I18n.tr("Previous turn")
            enabled: root.currentIndex > 0
            opacity: (enabled ? 1 : 0.3) * (pointer.containsMouse || rail.activeFocus || previousHover.hovered || nextHover.hovered || activeFocus ? 1 : 0)
            onClicked: root.choose(root.currentIndex-1)
        }
        T3IconButton {
            objectName: "t3TimelineNext"
            implicitWidth:Style.space(20);implicitHeight:implicitWidth
            HoverHandler { id: nextHover }
            x: Style.space(4)-width/2; y: rail.height+Style.space(4)
            name: "chevron-down"; tooltipText:T3I18n.tr("Next turn")
            enabled: root.currentIndex >= 0 && root.currentIndex < root.items.length-1
            opacity: pointer.containsMouse || rail.activeFocus ? 1 : 0
            onClicked: root.choose(root.currentIndex+1)
        }
    }
}
