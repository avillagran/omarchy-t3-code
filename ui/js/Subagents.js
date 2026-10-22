.pragma library
// Presentation of T3 subagents (projection `subagents`, see src/lib.rs
// subagent_summary), after T3's SubagentTimelineLink and ThreadRelationshipIcon.
var LIVE = ["pending", "running", "waiting"]
function live(s) { return !!s && LIVE.indexOf(s.status) >= 0 }
function settled(s) { return !!s && ["completed", "failed", "cancelled", "interrupted", "idle"].indexOf(s.status) >= 0 }
// English source strings; callers translate them.
function label(status) {
  return ({pending: "Queued", running: "Running", waiting: "Waiting", idle: "Idle · resumable", completed: "Completed",
           failed: "Failed", cancelled: "Stopped", interrupted: "Stopped"})[status] || "Unknown"
}
// Sidebar state names, for T3Palette.stateColor.
function tone(status) {
  return status === "waiting" ? "waiting" : live({status: status}) ? "working" : status === "completed" ? "done" : status === "failed" ? "failed" : "idle"
}
function detail(s) {
  if (!s) return ""
  return String((live(s) ? (s.progress || s.result) : (s.result || s.progress)) || "")
}
function compactDuration(ms) {
  if (!(ms > 0)) return ""
  var s = Math.floor(ms / 1000), m = Math.floor(s / 60), h = Math.floor(m / 60)
  function two(n) { return (n < 10 ? "0" : "") + n }
  return h > 0 ? h + "h " + two(m % 60) + "m" : m > 0 ? m + "m " + two(s % 60) + "s" : s + "s"
}
function elapsed(s, now) {
  if (!s || !s.startedAt) return ""
  var end = live(s) ? now : (s.completedAt || s.updatedAt || 0)
  return end > s.startedAt ? compactDuration(end - s.startedAt) : ""
}
// The newest known state: a card's copy from the timeline, overlaid with the
// thread's live subagent of the same id.
function merge(card, list) {
  var fresh = (list || []).find(function(s) { return s.id === card.id && card.id != null })
  return fresh ? Object.assign({}, card, fresh) : card
}
// "2 working · 1 done · 1 failed" (T3 summarizeSubagentStatuses); English parts.
function buckets(list) {
  var b = {working: 0, done: 0, failed: 0, stopped: 0, idle: 0}
  ;(list || []).forEach(function(s) {
    if (live(s)) b.working++
    else if (s.status === "completed") b.done++
    else if (s.status === "failed") b.failed++
    else if (s.status === "idle") b.idle++
    else b.stopped++
  })
  return b
}
// A sub-thread from the sidebar roster seen as a subagent, for threads the
// projection does not list (e.g. older children).
function fromThread(t) {
  var status = t.working ? "running" : t.pending && t.reason === "waiting" ? "waiting" : t.status === "failed" ? "failed" : t.status === "completed" ? "completed" : "idle"
  return {id: "thread:" + t.key, title: t.title || "", status: status, childThreadId: t.id, startedAt: t.working ? (t.startedAt || 0) : 0, updatedAt: t.updatedAt || 0, providerInstanceId: t.provider, model: t.model, fromThread: true}
}
