pragma Singleton
import QtQuick 6.11
import Quickshell

// One clock for every decorative animation (sky, spinners, shimmer, comets).
// Each display-rate animation would repaint its window on its own schedule,
// and the compositor recomposes the whole surface for every frame; sharing
// one timer folds them into a single frame per tick. Each user states the
// pace it needs through T3Tick and the fastest request wins, so the panel
// only runs fast while something fast is on screen; nothing runs without
// requests.
Singleton {
  id: root
  // Seconds of wall time, advanced only while someone listens.
  property real time: 0
  // Preferences > Animations > Animation smoothness: "adaptive" gives each
  // pace its own rate, "saver" caps everything at 30 fps and "max" runs every
  // pace at the display rate.
  property string mode: "adaptive"
  // slow: drift of an idle sky. ambient: the sky while work runs. smooth:
  // spinners, the working shimmer, comets and state icons. fast: meteors.
  function intervalFor(pace) {
    if (mode === "max") return 8
    if (pace === "slow") return 125
    if (mode === "saver") return 33
    return pace === "fast" ? 8 : pace === "smooth" ? 16 : 33
  }
  property var requests: ({})
  property int nextTicket: 1
  readonly property int interval: {
    var fastest = 0
    for (var key in requests) if (!fastest || requests[key] < fastest) fastest = requests[key]
    return fastest
  }
  function acquire(ms) {
    var ticket = nextTicket++, next = Object.assign({}, requests)
    next[ticket] = Math.max(8, ms); requests = next
    return ticket
  }
  function release(ticket) {
    if (!ticket || !(ticket in requests)) return
    var next = Object.assign({}, requests); delete next[ticket]; requests = next
  }
  Timer {
    id: timer
    property double last: 0
    interval: Math.max(8, root.interval)
    repeat: true; running: root.interval > 0
    onRunningChanged: last = 0
    onTriggered: {
      var now = Date.now()
      root.time += last ? Math.min((now - last) / 1000, 0.25) : interval / 1000
      last = now
    }
  }
}
