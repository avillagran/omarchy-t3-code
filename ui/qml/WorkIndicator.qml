import QtQuick 6.11
import QtQuick.Shapes
import qs.Commons

// Busy spinner: a round-capped arc that stretches and contracts while it
// travels around a faint track, eased in when it starts. It moves on the
// shared T3Ticker clock so it never adds frames of its own.
Item {
  id: root
  property bool active: false
  property color tint: Color.accent
  readonly property bool running: active && visible && T3Motion.enabled
  implicitWidth: Style.space(12)
  implicitHeight: implicitWidth
  readonly property real size: Math.min(width, height)
  readonly property real stroke: Math.max(1.4, size / 7.5)
  readonly property real radius: Math.max(0, (size - stroke) / 2)
  // One cycle: the head runs ahead, then the tail catches up; each cycle
  // starts where the previous tail stopped, so the motion never jumps.
  T3Tick { id: tick; running: root.running }
  readonly property real phase: (tick.time / 1.25) % 1
  property real cycle: 0
  readonly property real head: 270 * ease(Math.min(1, phase * 1.6))
  readonly property real tail: 270 * ease(Math.max(0, phase * 1.6 - 0.6))
  function ease(v) { return v < 0.5 ? 4 * v * v * v : 1 - Math.pow(-2 * v + 2, 3) / 2 }
  property real lastPhase: 0
  onPhaseChanged: { if (phase < lastPhase) cycle = (cycle + 270) % 360; lastPhase = phase }
  Shape {
    id: spinner
    anchors.centerIn: parent
    width: root.size; height: root.size
    preferredRendererType: Shape.CurveRenderer
    opacity: 0; scale: 0.6
    ShapePath {
      strokeColor: Qt.rgba(root.tint.r, root.tint.g, root.tint.b, 0.22); fillColor: "transparent"
      strokeWidth: root.stroke
      PathAngleArc { centerX: root.size / 2; centerY: root.size / 2; radiusX: root.radius; radiusY: root.radius; startAngle: 0; sweepAngle: 360 }
    }
    ShapePath {
      strokeColor: root.tint; fillColor: "transparent"
      strokeWidth: root.stroke; capStyle: ShapePath.RoundCap
      PathAngleArc {
        centerX: root.size / 2; centerY: root.size / 2; radiusX: root.radius; radiusY: root.radius
        startAngle: root.cycle + root.tail - 90; sweepAngle: Math.max(14, root.head - root.tail)
      }
    }
    rotation: (tick.time * 225) % 360
  }
  ParallelAnimation {
    running: T3Motion.enabled && (root.running)
    NumberAnimation { target: spinner; property: "opacity"; from: 0; to: 1; duration: T3Motion.duration(220); easing.type: Easing.OutCubic }
    NumberAnimation { target: spinner; property: "scale"; from: 0.6; to: 1; duration: T3Motion.duration(320); easing.type: Easing.OutBack; easing.overshoot: 1.6 }
  }
  // A still indicator (inactive but shown) keeps a calm partial ring.
  states: State {
    when: !root.running
    PropertyChanges { spinner.opacity: 1; spinner.scale: 1 }
  }
}
