import QtQuick 6.11
import qs.Commons

// Animated sky. One instance owns the clock; others follow it through
// `leader` so every layer of the panel shares the same drifting field.
//
// mode: "auto" | "full" | "light" | "off". Animating any item re-renders the
// whole window, and the compositor recomposes it for every frame, so cost is
// frame rate, not memory. Idle skies tick at 8 fps (drift is ~5 px/s, visually
// identical). While busy the sky ticks at the ambient pace (25 fps in light),
// an explicit "full" at the smooth pace, and any meteor on screen at the
// display rate (Preferences > Animations > Animation smoothness). Auto drops to light for the
// session when ticks fall behind; the software renderer is always static.
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

  property real lastTime: -1
  property real slowTicks: 0
  // Shooting stars cross the panel in about a second, so the leader runs at
  // the display rate while one is on screen (or about to be) and drops back
  // to the ambient cadence for the slow drift. The schedule below mirrors
  // meteor() in starfield.frag, which is deterministic in `clock`.
  // meteorReach: the highest meteor chance among the skies following this
  // one (the sidebar header shows more of them than the panel).
  property real meteorReach: 0
  property bool meteorNow: false
  function fract(v) { return v - Math.floor(v) }
  function hash12(x, y) {
    var a = fract(x * 0.1031), b = fract(y * 0.1031), c = a
    var d = a * (b + 33.33) + b * (c + 33.33) + c * (a + 33.33)
    a += d; b += d; c += d
    return fract((a + b) * c)
  }
  function meteorLive(c, level) {
    for (var channel = 0; channel < 4; channel++) {
      var period = 2.1 + channel * 1.37, phase = c / period + channel * 0.31
      var slot = Math.floor(phase), t = (phase - slot) * period
      // A small margin covers float32 rounding on the GPU near the threshold.
      if (hash12(slot, channel * 17) > level + 0.02) continue
      if (t <= 0.75 + 0.5 * hash12(slot + 2, channel * 17 + 2) + 0.05) return true
    }
    return false
  }
  // The leader advances on the shared T3Ticker, so stars, spinners and the
  // activity shimmer change in the same frame instead of each adding its own.
  T3Tick {
    id: tick
    running: !field.leader && field.visible && !field.paused && field.animated
    pace: field.meteorNow ? "fast" : field.mode === "full" ? "smooth" : !field.busy ? "slow" : "ambient"
    interval: field.effectiveMode === "light" && !field.meteorNow ? Math.max(40, T3Ticker.intervalFor(pace)) : T3Ticker.intervalFor(pace)
    onRunningChanged: { field.lastTime = -1; field.slowTicks = 0 }
    onTimeChanged: {
      if (!running) return
      var dt = field.lastTime < 0 ? interval / 1000 : time - field.lastTime
      field.lastTime = time
      if (dt <= 0) return
      var step = dt * field.speedScale
      field.clock += step
      field.drift += step * (field.idleSpeed + (field.busySpeed - field.idleSpeed) * field.energy)
      // Look one slow tick ahead so a meteor never starts at the low rate.
      var reach = field.meteorReach * field.energy
      field.meteorNow = reach > 0.001 && field.meteorLive(field.clock + 0.13 * field.speedScale, reach)
      // Ticks sustained at twice their interval: this machine can't keep up.
      if (field.mode === "auto" && field.effectiveMode === "full" && field.busy && !field.meteorNow) {
        field.slowTicks = dt > interval / 500 ? field.slowTicks + 1 : Math.max(0, field.slowTicks - 0.5)
        if (field.slowTicks > 90) field.degraded = true
      }
    }
  }
  fragmentShader: Qt.resolvedUrl("../assets/shaders/starfield.frag.qsb")
}
