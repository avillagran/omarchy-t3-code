import QtQuick 6.11

// A subscription to T3Ticker: `time` advances (in seconds) only while
// `running`, at the rate T3Ticker gives `pace` (slow, ambient, smooth, fast).
// Read `time` only behind `running` so idle users add no work on each tick.
// A tick nobody reads still raises the shared rate for a visible animation
// that follows another clock (thread comets ride the sky's).
QtObject {
  id: root
  property bool running: false
  property string pace: "smooth"
  property int interval: T3Ticker.intervalFor(pace)
  property int ticket: 0
  readonly property real time: running ? T3Ticker.time : 0
  // Reading `interval` can re-enter sync() (its binding settles during
  // creation); read it first and never overwrite a ticket a nested call took,
  // or the orphan keeps the shared clock at full rate for good.
  function sync() {
    var want = running && T3Motion.enabled ? Math.max(8, interval) : 0
    if (ticket && T3Ticker.requests[ticket] !== want) { T3Ticker.release(ticket); ticket = 0 }
    if (want && !ticket) { var taken = T3Ticker.acquire(want); if (ticket) T3Ticker.release(taken); else ticket = taken }
  }
  property bool motionEnabled: T3Motion.enabled
  onMotionEnabledChanged: sync()
  onRunningChanged: sync()
  onIntervalChanged: sync()
  Component.onCompleted: sync()
  Component.onDestruction: T3Ticker.release(ticket)
}
