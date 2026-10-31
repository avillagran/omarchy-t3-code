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
// Search reads like a person would: the title first, then project, group,
// branch and host. Matches at the start of a field or word rank highest, then
// whole words, initials ("otc" → omarchy-t3-code), inner text and, only when
// nothing matches exactly, small typos. Accents, case and word order never
// matter. Prefixes aim at one field: t: title, p: project, g: group,
// b: branch, h: host, is: state (working, input, done, failed, pinned,
// snoozed, settled); "quoted words" stay together and -word excludes.
// Threads whose messages matched on the server (context.hits) are found too.
function fold(text) { return String(text || "").toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "") }
var FIELD_NAMES = {t: "title", title: "title", titulo: "title", p: "project", project: "project", proyecto: "project", g: "group", group: "group", grupo: "group",
                   b: "branch", branch: "branch", rama: "branch", h: "host", host: "host", is: "is", estado: "is", state: "is"}
var FIELD_WEIGHT = {title: 1, project: 0.85, group: 0.85, branch: 0.6, host: 0.5}
function parse(query) {
  var out = [], re = /(-?)(?:([a-z]+):)?(?:"([^"]*)"?|(\S+))/g, q = fold(query), m
  while ((m = re.exec(q))) {
    var field = m[2] ? FIELD_NAMES[m[2]] || "" : "", text = m[3] !== undefined ? m[3] : m[4] || ""
    if (m[2] && !field) text = m[2] + ":" + text
    text = text.replace(/^[\/·,]+|[\/·,]+$/g, "").trim()
    if (text) out.push({text: text, field: field, not: m[1] === "-", phrase: m[3] !== undefined})
  }
  return out
}
function terms(query) { return parse(query).map(function(t) { return t.text }) }
// The words the server should look for in messages: no field or excluded terms.
function freeText(query) { return parse(query).filter(function(t) { return !t.field && !t.not }).map(function(t) { return t.phrase ? '"' + t.text + '"' : t.text }).join(" ") }
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
function words(text) { return text.split(/[^a-z0-9]+/).filter(function(w) { return w.length > 0 }) }
// How well one term fits one field, 0 to 100.
function fit(term, text, fuzzy) {
  if (!text || !term) return 0
  var ws = words(text)
  if (text.indexOf(term) === 0) return 100
  if (ws.indexOf(term) >= 0) return 90
  if (ws.some(function(w) { return w.indexOf(term) === 0 })) return 75
  if (term.length >= 2 && ws.length > 1 && ws.map(function(w) { return w[0] }).join("").indexOf(term) === 0) return 60
  if (term.length >= 3 && text.indexOf(term) >= 0) return 45
  if (fuzzy) {
    var limit = term.length >= 8 ? 2 : term.length >= 5 ? 1 : 0
    if (limit && ws.some(function(w) { return Math.abs(w.length - term.length) <= limit + 3 && near(term, w, limit) })) return 20
  }
  return 0
}
function fields(t, context) {
  var c = context || {}, hosts = c.hosts || {}, groups = c.groups || {}
  return {title: fold(t.title), project: fold(t.project), group: fold(groups[t.key] || ""),
          branch: fold([t.branch || "", t.worktreeBranch || ""].join(" ")).trim(), host: fold([hosts[t.environmentId] || "", t.environmentName || "", t.environmentId || ""].join(" "))}
}
var STATES = {working: function(t) { return state(t) === "working" }, trabajando: function(t) { return state(t) === "working" },
              input: function(t) { return state(t) === "waiting" }, waiting: function(t) { return state(t) === "waiting" }, esperando: function(t) { return state(t) === "waiting" },
              done: function(t) { return detail(t) === "done" }, ready: function(t) { return detail(t) === "done" }, listo: function(t) { return detail(t) === "done" },
              failed: function(t) { return state(t) === "failed" }, error: function(t) { return state(t) === "failed" }, fallido: function(t) { return state(t) === "failed" },
              pinned: function(t) { return !!t.pinned }, fijado: function(t) { return !!t.pinned },
              snoozed: function(t) { return !!t.snoozed }, pospuesto: function(t) { return !!t.snoozed },
              settled: function(t) { return !!t.settled }}
function stateIs(t, value) { var key = Object.keys(STATES).find(function(k) { return k.indexOf(value) === 0 }); return !!key && STATES[key](t) }
function termScore(term, f, fuzzy) {
  var best = 0
  ;(term.field ? [term.field] : Object.keys(FIELD_WEIGHT)).forEach(function(name) { best = Math.max(best, fit(term.text, f[name] || "", fuzzy) * (FIELD_WEIGHT[name] || 1)) })
  return best
}
// {matched, total, score}: how many wanted terms the thread meets and how well.
// An excluded term that matches drops the thread (matched -1).
function evaluate(t, list, context, fuzzy) {
  var f = fields(t, context), matched = 0, total = 0, score = 0
  for (var i = 0; i < list.length; i++) {
    var term = list[i], s = term.field === "is" ? (stateIs(t, term.text) ? 100 : 0) : termScore(term, f, fuzzy)
    if (term.not) { if (s >= 45) return {matched: -1, total: 0, score: 0}; continue }
    total++
    if (s > 0) { matched++; score += s }
  }
  // Words found only in the thread's messages count, below any field match.
  if (matched < total && context && context.hits && context.hits[t.key]) { score += 15 * (total - matched); matched = total }
  return {matched: matched, total: total, score: total ? score / total : 0}
}
// Every thread's relevance for a query, best tier only: exact matches of all
// words; else with typos; else the most words any thread matches.
function rank(threads, query, context) {
  var list = parse(query), out = {}
  if (!list.length) return out
  function pass(fuzzy) { return threads.map(function(t) { return {t: t, e: evaluate(t, list, context, fuzzy)} }) }
  var strict = pass(false), all = strict.filter(function(r) { return r.e.matched >= 0 && r.e.matched === r.e.total })
  if (!all.length) { strict = pass(true); all = strict.filter(function(r) { return r.e.matched >= 0 && r.e.matched === r.e.total }) }
  if (!all.length) {
    var most = Math.max.apply(null, [0].concat(strict.map(function(r) { return r.e.matched })))
    all = most > 0 ? strict.filter(function(r) { return r.e.matched === most }) : []
  }
  all.forEach(function(r) { out[r.t.key || r.t.id] = Math.max(1, r.e.score) })
  return out
}
// Kept for callers that score one thread: 2 every word, 1 some word, 0 none.
function searchScore(t, query, context) {
  var list = parse(query)
  if (!list.length) return 2
  var e = evaluate(t, list, context, true)
  return e.matched < 0 ? 0 : e.matched === e.total ? 2 : e.matched > 0 ? 1 : 0
}
function rows(threads, filter, project, expanded, query, context) {
  var result = [], settled = [], needle = parse(query).length > 0
  var visible = threads.filter(function(t) { return matches(t, filter, project) })
  var ranks = needle ? rank(visible, query, context) : null
  visible.forEach(function(t) { if (!ranks || ranks[t.key || t.id]) (t.settled ? settled : result).push(t) })
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
// Chronological mode includes pins; grouping alone gives pins a separate section.
// Older servers may omit reply timestamps; retain their completion clock as fallback.
function replyTime(t){
  var reply=t.repliedAt || (t.settled && t.settledSortAt) || t.inboxAt || t.updatedAt || 0
  // While a run is active the shell exposes activity, not individual token times.
  // Do not bury that run below its previous completed answer.
  return t.working ? Math.max(reply,t.updatedAt || 0,t.startedAt || 0) : reply
}
function compareReply(a,b){return replyTime(b)-replyTime(a) || tie(a,b)}
function compareActive(a,b,grouped,returned,byReply) {
  if(byReply)return compareReply(a,b)
  if(!!a.pinned!==!!b.pinned)return a.pinned?-1:1
  if(a.pinned&&b.pinned)return String(a.pinOrderKey || "").localeCompare(String(b.pinOrderKey || "")) || (a.pinnedAt || 0)-(b.pinnedAt || 0) || tie(a,b)
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
function childrenByParent(threads, compare) {
  var map = {}
  threads.forEach(function(t) {
    if (!isSubthread(t)) return
    var key = t.environmentId + "/" + t.parentThreadId
    ;(map[key] = map[key] || []).push(t)
  })
  Object.keys(map).forEach(function(k) { map[k].sort(compare || function(a,b){return (b.lastSentAt || b.updatedAt || 0)-(a.lastSentAt || a.updatedAt || 0) || tie(a,b)}) })
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
function nest(rows, threads, overrides, selectedKey, compare) {
  var map = childrenByParent(threads || [], compare)
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
