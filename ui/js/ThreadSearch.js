.pragma library
.import "NativeJson.js" as NativeJson

// Qt owns the two debounced consumers; V8 owns the original Unicode tokens.
function field(channel) { return channel === "sidebar" ? "sidebarSearch" : "threadSearch" }
function current(owner, channel) { return owner[field(channel)] }
function publish(owner, channel, value) { owner[field(channel)] = value }
function present(owner, channel, next, rows) {
  var scope = next.scope || {}, searchScope = {}
  ;["projectIds", "environmentLabels", "titlesOnly"].forEach(function(key) {
    if (scope[key] !== undefined) searchScope[key] = scope[key]
  })
  var environments = (scope.environmentIds || []).slice().sort(function(a,b){return a.localeCompare(b)})
  var key = next.settledQuery ? NativeJson.stringify(Object.keys(searchScope).length
    ? [environments, next.settledQuery, searchScope] : [environments, next.settledQuery]) : ""
  rows = key ? rows || [] : []
  var signature = NativeJson.stringify([key, rows]), changed = current(owner, channel).mountSignature !== signature
  publish(owner, channel, Object.assign({}, next, {rows:rows, mountSignature:signature}))
  if (changed) {
    // The Rust reader handles this control before the refresh, in pipe order.
    owner.send({op:"thread-search-mount", payloadJson:NativeJson.stringify({consumer:channel, key:key, rows:rows}), requestId:owner.uuid()})
    owner.refresh()
  }
}
function queue(owner, channel, raw, scope) {
  // Same UTF16 normalization as useThreadSearch in state/queries.ts.
  var query = String(raw || "").trim().slice(0, 200).trim()
  var previous = current(owner, channel), key = NativeJson.stringify(scope || {})
  if (query === previous.query && key === previous.scopeKey && !previous.error) return
  var generation = (previous.generation || 0) + 1
  var next = {query:query, settledQuery:previous.settledQuery || "",
    matches:previous.matches || [], requestId:"", generation:generation,
    scope:scope || {}, scopeKey:key, planReady:false, debounced:false,
    busy:query.length > 0, error:"", previous:previous.previous || null,
    rows:previous.rows || [], mountSignature:previous.mountSignature}
  if (!query || !next.scope.environmentIds || next.scope.environmentIds.length === 0) {
    next.matches = []; next.settledQuery = ""; next.busy = false
    present(owner, channel, next, [])
    return
  }
  publish(owner, channel, next)
  owner.restartThreadSearchDebounce(channel)
  owner.nativeSearch({action:"query", query:query,
    previousQuery:next.previous ? next.previous.query : "",
    anyOlderSettled:next.scope.anyOlderSettled === true,
    titlesOnly:next.scope.titlesOnly === true}, function(plan, error) {
    var state = current(owner, channel)
    if (state.generation !== generation) return
    var failure = error || (!plan ? "invalid_native_search_result" : "")
    var update = {planReady:true, plan:plan, error:failure}, rows = []
    if (failure || !plan.canSearch) {
      update.matches = []; update.settledQuery = ""; update.busy = false
    } else {
      var held = state.previous
      update.settledQuery = plan.visibleQuery || ""
      update.matches = update.settledQuery && held ? held.matches : []
      if (update.settledQuery && held && held.scopeKey === state.scopeKey) rows = held.rows
    }
    present(owner, channel, Object.assign({}, state, update), rows)
    dispatch(owner, channel)
  })
}
function debounced(owner, channel) {
  publish(owner, channel, Object.assign({}, current(owner, channel), {debounced:true}))
  dispatch(owner, channel)
}
function dispatch(owner, channel) {
  var state = current(owner, channel)
  if (!state.debounced || !state.planReady || state.requestId || state.error || !state.plan.canSearch) return
  var id = owner.uuid(), scope = state.scope
  publish(owner, channel, Object.assign({}, state, {requestId:id, busy:true}))
  owner.send({op:"thread-search", requestId:id, payloadJson:NativeJson.stringify({query:state.query,
    environmentIds:scope.environmentIds, projectIds:scope.projectIds,
    environmentLabels:scope.environmentLabels, searchMessages:state.plan.searchMessages})})
}
function accept(owner, data) {
  var channel = data.requestId === owner.sidebarSearch.requestId ? "sidebar"
    : data.requestId === owner.threadSearch.requestId ? "palette" : ""
  if (!channel || !data.requestId) return false
  var state = current(owner, channel), result = data.result || {}
  if (!data.error && typeof result.resultJson === "string") {
    try { result = JSON.parse(result.resultJson) }
    catch (_) { data = Object.assign({}, data, {error:"invalid_search_json"}); result = {} }
  }
  var matches = result.matches || [], pending = result.pending > 0
  var update = {busy:pending, error:data.error || ""}
  // An empty partial answer does not replace the previous visible query.
  if (data.error || !pending || matches.length || (result.rows || []).length) {
    update.matches = data.error ? [] : matches
    update.settledQuery = state.query
    update.previous = {query:state.query, matches:update.matches,
      rows:data.error ? [] : result.rows || [], scopeKey:state.scopeKey}
  }
  var next = Object.assign({}, state, update)
  present(owner, channel, next, update.previous ? update.previous.rows : state.rows)
  return true
}
function lost(owner) {
  ;["sidebar", "palette"].forEach(function(channel) {
    var state = current(owner, channel)
    publish(owner, channel, Object.assign({}, state, {generation:(state.generation || 0) + 1,
      requestId:"", planReady:false, busy:false, error:"disconnected", mountSignature:""}))
  })
}

  // Search mounts retain routing metadata in Rust and native text in validated
  // JSON. Restore it before publishing a snapshot or ranking its actual rows.
function unpackSnapshot(snapshot) {
    return Object.assign({}, snapshot, {threads:(snapshot.threads || []).map(function(thread) {
      if (typeof thread.searchShellJson !== "string") return thread
      var shell=JSON.parse(thread.searchShellJson)
      if (shell.id !== thread.id || shell.environmentId !== thread.environmentId) throw Error("invalid_search_row_scope")
      var restored=Object.assign({},thread,{searchShell:shell})
      if (typeof thread.searchTitleJson === "string") restored.title=JSON.parse(thread.searchTitleJson)
      if (typeof thread.searchBranchJson === "string") restored.branch=JSON.parse(thread.searchBranchJson)
      return restored
    })})
  }

// The date conversion and optional pick are shared by the actual caller and its
// private Qt transport fixture. JSON syntax is the scalar-safe pipe envelope.
function visitPacket(environment,thread,ms,opened,pickedQuery,requestId) {
  var request={op:"thread-visit",environment:environment,thread:thread,
    visitedAt:new Date(ms).toISOString(),requestId:requestId}
  if(opened){request.opened=true;if(pickedQuery!==undefined)request.searchQuery=pickedQuery}
  return {op:request.op,environment:environment,thread:thread,requestId:requestId,payloadJson:NativeJson.stringify(request)}
}
