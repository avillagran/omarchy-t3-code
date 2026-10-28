.pragma library
// T3's row status (resolveSidebarThreadStatus, computed in sidebar.rs) refined
// for the row label: approval, input, plan, waiting, goal, working, limited,
// failed, woke, done or "" (nothing to say).
function detail(t) {
  var s = t.sidebarStatus
  if (s === undefined) {
    var legacy = legacyState(t)
    return legacy === "waiting" ? (t.reason === "plan" ? "plan" : "input") : legacy === "idle" || (legacy === "done" && !t.pending) ? "" : legacy
  }
  if (s === "approval" || s === "input") return s
  if (s === "ready" && t.pending && t.reason === "plan") return "plan"
  if (s !== "ready") return s
  if (t.woke && !t.snoozed) return "woke"
  if (t.pending && t.reason === "review") return "done"
  return ""
}
// The shelf and filter bucket: waiting (needs you), working, failed, done, woke or idle.
function state(t) {
  if (t.sidebarStatus === undefined) return legacyState(t)
  return ({approval: "waiting", input: "waiting", plan: "waiting", goal: "working", working: "working", limited: "failed", failed: "failed", done: "done", woke: "woke"})[detail(t)] || "idle"
}
function legacyState(t) {
  if (t.pending && ["request", "plan", "waiting"].indexOf(t.reason) >= 0) return "waiting"
  if (t.working) return "working"
  if (t.status === "failed") return "failed"
  if (t.status === "completed") return "done"
  return "idle"
}
function matches(t, filter, project) {
  if (t.relationshipToParent === "subagent") return false
  return (!project || (Array.isArray(project) ? project.indexOf(t.projectKey)>=0 : t.projectKey === project)) && (filter === "all" || (filter === "settled" ? !!t.settled : filter === "pending" ? !!t.pending : state(t) === filter))
}
// Search forgives accents, case, word order and small typos, and also finds
// threads whose messages matched on the server (context.hits, keyed by thread).
function fold(text) { return String(text || "").toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "") }
function terms(query) { return fold(query).split(/[\s\/·,]+/).filter(function(w) { return w.length > 0 }) }
// Edits needed to turn a into a prefix of b, stopping once over the limit.
function near(a, b, limit) {
  var prev = [], i, j
  for (j = 0; j <= b.length; j++) prev.push(0)
  for (i = 1; i <= a.length; i++) {
    var cur = [i], best = i
    for (j = 1; j <= b.length; j++) { cur.push(Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1))); best = Math.min(best, cur[j]) }
    if (best > limit) return false
    prev = cur
  }
  return Math.min.apply(null, prev) <= limit
}
function termMatches(term, text, words) {
  if (text.indexOf(term) >= 0) return true
  var limit = term.length >= 7 ? 2 : term.length >= 4 ? 1 : 0
  return limit > 0 && words.some(function(w) { return Math.abs(w.length - term.length) <= limit + 3 && near(term, w, limit) })
}
function haystack(t, context) {
  var hosts = (context && context.hosts) || {}
  return fold([t.title, t.project, hosts[t.environmentId] || "", t.environmentId, t.branch || "", t.worktreeBranch || ""].join(" "))
}
// 2: every word matches; 1: some word matches; 0: no match.
function searchScore(t, query, context) {
  var list = terms(query)
  if (!list.length) return 2
  if (context && context.hits && context.hits[t.key]) return 2
  var text = haystack(t, context), words = text.split(/[^a-z0-9]+/).filter(function(w) { return w.length > 1 })
  var found = list.filter(function(term) { return termMatches(term, text, words) }).length
  return found === list.length ? 2 : found > 0 ? 1 : 0
}
function rows(threads, filter, project, expanded, query, context) {
  var result = [], settled = []
  var needle = terms(query).length > 0, scored = [], best = 0
  threads.forEach(function(t) {
    if (!matches(t, filter, project)) return
    var score = needle ? searchScore(t, query, context) : 2
    if (score > 0) { scored.push({t: t, score: score}); best = Math.max(best, score) }
  })
  // Nothing matches every word: show what matches any of them rather than nothing.
  scored.forEach(function(s) { if (s.score === best) (s.t.settled ? settled : result).push(s.t) })
  if (settled.length > 0) {
    result.push({sectionHeader: true, count: settled.length, key: "@settled"})
    if (expanded || filter === "settled" || needle) result = result.concat(settled)
  }
  return result
}
function count(threads, filter, project) { return threads.filter(function(t) { return matches(t, filter, project) }).length }
function counts(threads) {
  var totals={working:0,done:0,waiting:0,failed:0}
  threads.filter(function(t){return !t.settled&&t.relationshipToParent!=="subagent"}).forEach(function(t){var s=state(t);if(totals[s]!==undefined)totals[s]++})
  return totals
}

// Fold only work that does not need a decision; keep attention requests in the inbox.
function inWorkingShelf(t) { return state(t) === "working" && !t.pinned }
function observeReturns(previous, threads, now) {
  var working={},present={},returned={},prior=previous || {working:null,returned:{}}
  threads.forEach(function(t){present[t.key]=true;if(state(t)==="working")working[t.key]=true})
  Object.keys(prior.returned).forEach(function(k){if(present[k])returned[k]=prior.returned[k]})
  Object.keys(prior.working || {}).forEach(function(k){if(present[k]&&!working[k])returned[k]=now})
  return {working:working,returned:returned}
}
function tie(a,b){return String(a.id).localeCompare(String(b.id)) || String(a.environmentId || "").localeCompare(String(b.environmentId || ""))}
function compareInbox(a,b,returned) {
  return Math.max(b.inboxAt || 0,returned[b.key] || 0)-Math.max(a.inboxAt || 0,returned[a.key] || 0) || tie(a,b)
}
// The agent's latest answer first (O3's default); pinned threads keep their place on top.
function compareReply(a,b){return (b.repliedAt || b.inboxAt || 0)-(a.repliedAt || a.inboxAt || 0) || tie(a,b)}
function compareActive(a,b,grouped,returned,byReply) {
  if(!!a.pinned!==!!b.pinned)return a.pinned?-1:1
  if(a.pinned&&b.pinned)return String(a.pinOrderKey || "").localeCompare(String(b.pinOrderKey || "")) || (a.pinnedAt || 0)-(b.pinnedAt || 0) || tie(a,b)
  if(byReply)return compareReply(a,b)
  if(grouped)return compareInbox(a,b,returned)
  if(a.activeOrderKey==null&&b.activeOrderKey!=null)return -1
  if(a.activeOrderKey!=null&&b.activeOrderKey==null)return 1
  return (a.activeOrderKey!=null&&b.activeOrderKey!=null?String(a.activeOrderKey).localeCompare(String(b.activeOrderKey)):(b.activeAnchorAt || 0)-(a.activeAnchorAt || 0)) || tie(a,b)
}
function compareSettled(a,b){return (b.settledSortAt || b.updatedAt || 0)-(a.settledSortAt || a.updatedAt || 0) || tie(a,b)}
function shelfRows(rows, expanded, selectedKey, reveal) {
  return expanded || reveal ? rows : rows.filter(function(t){return t.key===selectedKey})
}

// Sub-threads (subagents) hang under their parent thread, newest first.
function isSubthread(t) { return t.relationshipToParent === "subagent" && !!t.parentThreadId }
function childrenByParent(threads) {
  var map = {}
  threads.forEach(function(t) {
    if (!isSubthread(t)) return
    var key = t.environmentId + "/" + t.parentThreadId
    ;(map[key] = map[key] || []).push(t)
  })
  Object.keys(map).forEach(function(k) { map[k].sort(function(a,b){return (b.lastSentAt || b.updatedAt || 0)-(a.lastSentAt || a.updatedAt || 0) || tie(a,b)}) })
  return map
}
function descendantKeys(key, map, depth) {
  var out = []
  if ((depth || 0) > 4) return out
  ;(map[key] || []).forEach(function(c) { out.push(c.key); out = out.concat(descendantKeys(c.key, map, (depth || 0) + 1)) })
  return out
}
// A parent opens on its own while a child works or waits, or holds the
// selection; an explicit choice (overrides[key]) always wins.
function subthreadsExpanded(key, map, overrides, selectedKey) {
  if (overrides && overrides[key] !== undefined) return !!overrides[key]
  var kids = map[key] || []
  return kids.some(function(c) { var s = state(c); return s === "working" || s === "waiting" }) || descendantKeys(key, map).indexOf(selectedKey) >= 0
}
function childSummary(kids) {
  var s = {count: kids.length, working: 0, waiting: 0, failed: 0, done: 0}
  kids.forEach(function(c) { var k = state(c); if (s[k] !== undefined) s[k]++ })
  return s
}
function nest(rows, threads, overrides, selectedKey) {
  var map = childrenByParent(threads || [])
  if (!Object.keys(map).length) return rows
  var out = []
  function add(t, depth, parentKey) {
    var kids = depth < 4 ? (map[t.key] || []) : []
    var row = t
    if (kids.length || depth) {
      row = Object.assign({}, t, {depth: depth, subthreadParentKey: parentKey || ""})
      if (kids.length) { row.subthreads = childSummary(kids); row.subthreadsExpanded = subthreadsExpanded(t.key, map, overrides, selectedKey) }
    }
    out.push(row)
    if (kids.length && row.subthreadsExpanded) kids.forEach(function(c) { add(c, depth + 1, t.key) })
  }
  rows.forEach(function(t) { if (t.sectionHeader) out.push(t); else if (!isSubthread(t)) add(t, 0, "") })
  return out
}
// Siblings of a sub-thread in the order the sidebar shows them.
function siblings(threads, t) {
  if (!t || !isSubthread(t)) return []
  return childrenByParent(threads || [])[t.environmentId + "/" + t.parentThreadId] || []
}
