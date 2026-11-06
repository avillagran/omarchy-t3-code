import QtQuick 6.11
import qs.Commons
import "../js/NativeTerminalLinks.js" as Links

Item {
  id: root
  required property var surface
  readonly property var helpers: Links.create(function(){return []})
  readonly property var scrollbarState: ({total:surface.scrollbackLines+surface.rows,len:surface.rows,offset:surface.scrollbackLines-surface.scrollOffset})
  readonly property var geometry: helpers.terminalScrollbarGeometry(scrollbarState,height)
  // Qt's ScrollBar accessibility value interface reads these item properties.
  readonly property real minimumValue: 0
  readonly property real maximumValue: geometry ? geometry.maxOffset : 0
  property real value: 0
  Binding { target: root; property: "value"; value: Math.max(0,Math.min(root.scrollbarState.offset,root.maximumValue)) }
  onValueChanged: if(value!==scrollbarState.offset)surface.scrollOffset=surface.scrollbackLines-value
  readonly property real stepSize: 1
  visible: !!geometry
  activeFocusOnTab: visible
  Accessible.role: Accessible.ScrollBar
  Accessible.name: T3I18n.tr("Terminal scrollback")
  Accessible.focusable: true
  Accessible.onIncreaseAction: root.move(1)
  Accessible.onDecreaseAction: root.move(-1)
  function move(delta){surface.scrollOffset=surface.scrollOffset-delta}
  function pointer(y,offset){surface.scrollOffset=surface.scrollbackLines-helpers.terminalScrollbarOffsetAtPointer(scrollbarState,height,y,offset)}
  Keys.onPressed: function(event){
    switch(event.key){
    case Qt.Key_Up: move(-1);break
    case Qt.Key_Down: move(1);break
    case Qt.Key_PageUp: move(-Math.max(1,scrollbarState.len));break
    case Qt.Key_PageDown: move(Math.max(1,scrollbarState.len));break
    case Qt.Key_Home: move(-scrollbarState.offset);break
    case Qt.Key_End: move(scrollbarState.total-scrollbarState.len-scrollbarState.offset);break
    default:return
    }
    event.accepted=true
  }
  Rectangle {
    id: thumb
    x: 1; width: Math.max(0,parent.width-2)
    y: root.geometry ? root.geometry.thumbTop : 0
    height: root.geometry ? root.geometry.thumbHeight : 0
    radius: 3
    color: root.surface.foreground
    opacity: mouse.containsMouse || root.activeFocus ? 0.5 : 0.25
  }
  MouseArea {
    id: mouse; anchors.fill: parent; hoverEnabled: true
    acceptedButtons: Qt.LeftButton
    property real pointerOffset: 0
    onPressed: function(event){
      pointerOffset=event.y>=thumb.y&&event.y<=thumb.y+thumb.height ? event.y-thumb.y : thumb.height/2
      root.pointer(event.y,pointerOffset)
    }
    onPositionChanged: function(event){if(pressed)root.pointer(event.y,pointerOffset)}
  }
}
