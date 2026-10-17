.pragma library
function state(t) {
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
function rows(threads, filter, project, expanded, query) {
  var result = [], settled = []
  var needle = String(query || "").toLowerCase()
  threads.forEach(function(t) { if (matches(t, filter, project) && (!needle || [t.title,t.project,t.environmentId].join(" ").toLowerCase().indexOf(needle) >= 0)) (t.settled ? settled : result).push(t) })
  if (settled.length > 0) {
    result.push({sectionHeader: true, count: settled.length, key: "@settled"})
    if (expanded || filter === "settled" || needle) result = result.concat(settled)
  }
  return result
}
function count(threads, filter, project) { return threads.filter(function(t) { return matches(t, filter, project) }).length }
function counts(threads) {
  var totals={working:0,done:0,waiting:0}
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
function compareActive(a,b,grouped,returned) {
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
