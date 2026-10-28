import QtQuick 6.11
import qs.Commons

// A one-line label that elides at rest and, while `active`, slides to show the
// rest of a long text: a pause, a glide to the end, a pause, back. It only
// animates while active (hover or selection), so idle rows cost no frames.
Item {
  id: root
  property string text: ""
  property color color: Color.popups.text
  property int pixelSize: Style.font.body
  property int weight: Font.Normal
  property bool active: false
  readonly property real overflow: Math.max(0, full.implicitWidth - width)
  readonly property bool sliding: active && overflow > 1 && T3Motion.enabled
  implicitHeight: full.implicitHeight
  implicitWidth: full.implicitWidth
  clip: sliding
  Text {
    id: full
    visible: root.sliding
    text: root.text; color: root.color; textFormat: Text.PlainText
    font.pixelSize: root.pixelSize; font.weight: root.weight
    anchors.verticalCenter: parent.verticalCenter
    x: 0
  }
  Text {
    objectName: "marqueeElided"
    visible: !root.sliding
    width: root.width
    anchors.verticalCenter: parent.verticalCenter
    text: root.text; color: root.color; textFormat: Text.PlainText; elide: Text.ElideRight
    font.pixelSize: root.pixelSize; font.weight: root.weight
  }
  SequentialAnimation {
    running: root.sliding
    loops: Animation.Infinite
    onStopped: full.x = 0
    PauseAnimation { duration: 700 }
    NumberAnimation { target: full; property: "x"; to: -root.overflow; duration: Math.max(900, root.overflow * 28); easing.type: Easing.InOutSine }
    PauseAnimation { duration: 1100 }
    NumberAnimation { target: full; property: "x"; to: 0; duration: Math.max(500, root.overflow * 10); easing.type: Easing.InOutSine }
  }
}
