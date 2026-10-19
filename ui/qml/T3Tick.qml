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
  function sync() {
    if (ticket && (!running || T3Ticker.requests[ticket] !== Math.max(8, interval))) { T3Ticker.release(ticket); ticket = 0 }
    if (running && !ticket) ticket = T3Ticker.acquire(interval)
  }
  onRunningChanged: sync()
  onIntervalChanged: sync()
  Component.onCompleted: sync()
  Component.onDestruction: T3Ticker.release(ticket)
}
