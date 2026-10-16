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
  threads.filter(function(t){return t.relationshipToParent!=="subagent"}).forEach(function(t){var s=state(t);if(totals[s]!==undefined)totals[s]++})
  return totals
}

// Fold only work that does not need a decision; keep attention requests in the inbox.
function inWorkingShelf(t) { return state(t) === "working" && !t.pinned }
function shelfRows(rows, expanded, selectedKey, reveal) {
  return expanded || reveal ? rows : rows.filter(function(t){return t.key===selectedKey})
}
