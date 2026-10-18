import QtQuick 6.11
import qs.Commons

// Animated sky. One instance owns the clock; others follow it through
// `leader` so every layer of the panel shares the same drifting field.
//
// mode: "auto" | "full" | "light" | "off". Animating any item re-renders the
// whole window, so cost is frame rate, not memory: light ticks at 8 fps while
// idle (drift is ~5 px/s, visually identical) and only speeds up while busy.
// Auto starts full and drops to light for the session when ticks fall behind;
// the software renderer is always static.
ShaderEffect {
  id: field
  property var leader: null
  property string mode: "auto"
  property bool busy: false
  property bool paused: false
  property bool header: false
  readonly property bool software: GraphicsInfo.api === GraphicsInfo.Software
  property bool degraded: false
  readonly property string effectiveMode: leader ? leader.effectiveMode
    : software || mode === "off" ? "off"
    : mode === "light" || mode === "auto" && degraded ? "light" : "full"
  readonly property bool animated: effectiveMode !== "off"
  property real energy: busy && animated ? 1 : 0
  Behavior on energy { NumberAnimation { duration: 1600; easing.type: Easing.InOutSine } }
  property real idleSpeed: 5
  // User preferences, read from the leader: motion speed and meteor amount.
  property real speedScale: 1
  property real meteorScale: 1
  property real busySpeed: 42
  // Meteor chance per channel roll; light mode keeps them in the header only.
  property real meteorRate: 0
  property real drift: leader ? leader.drift : 0
  property real clock: leader ? leader.clock : 0
  // Accumulated scroll for parallax. Feed it through follow(); jumps (thread
  // switch, history paging) are skipped so the sky never snaps.
  property real scroll: leader ? leader.scroll : 0
  property real lastScrollY: NaN
  function follow(y) {
    var d = y - lastScrollY
    lastScrollY = y
    if (Math.abs(d) < 400) scroll += d
  }
  readonly property real meteors: effectiveMode === "full" || header && effectiveMode === "light"
    ? Math.min(1, meteorRate * (leader ? leader.meteorScale : meteorScale)) * (leader ? leader.energy : energy) : 0
  property real brightness: 1
  property real nebula: 0
  // Fraction of the height that fades out at the bottom edge.
  property real fade: 0
  readonly property vector2d size: Qt.vector2d(width, height)
  property vector2d origin: Qt.vector2d(0, 0)
  property color star: Color.popups.text
  property color accent: Color.accent
  property color base: "transparent"
  blending: true

  property double lastTick: 0
  property real slowTicks: 0
  Timer {
    running: !field.leader && field.visible && !field.paused && field.animated
    repeat: true
    interval: field.effectiveMode === "full" ? 16 : field.busy ? 40 : 125
    onRunningChanged: { field.lastTick = 0; field.slowTicks = 0 }
    onTriggered: {
      var now = Date.now(), dt = field.lastTick ? Math.min((now - field.lastTick) / 1000, 0.25) : interval / 1000
      field.lastTick = now
      var step = dt * field.speedScale
      field.clock += step
      field.drift += step * (field.idleSpeed + (field.busySpeed - field.idleSpeed) * field.energy)
      // Sustained >30 ms frames in full mode: this machine can't keep up.
      if (field.mode === "auto" && field.effectiveMode === "full") {
        field.slowTicks = dt > 0.03 ? field.slowTicks + 1 : Math.max(0, field.slowTicks - 0.5)
        if (field.slowTicks > 90) field.degraded = true
      }
    }
  }
  fragmentShader: Qt.resolvedUrl("../assets/shaders/starfield.frag.qsb")
}
