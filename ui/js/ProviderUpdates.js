.pragma library
// Provider (harness) update notices, after T3's ProviderUpdateLaunchNotification.
// Each environment catalog lists its providers with `update` (src/provider_updates.rs)
// when a newer version is known.

// One entry per environment and driver, the first instance found.
function candidates(catalogs, environments) {
  var out = [], seen = {}
  ;(environments || []).forEach(function(env) {
    var catalog = (catalogs || {})[env.id]
    ;((catalog && catalog.providers) || []).forEach(function(p) {
      var u = p.update
      if (!u || !u.latestVersion) return
      var id = env.id + "=" + u.driver
      if (seen[id]) return
      seen[id] = true
      out.push({environment: env.id, environmentName: env.name || env.id, local: env.id === "local", instanceId: p.instanceId,
                driver: u.driver, name: u.name || p.name || u.driver, currentVersion: u.currentVersion || "", latestVersion: u.latestVersion,
                canUpdate: !!u.canUpdate, oneClick: !!u.oneClick, state: u.state || "idle", stateMessage: u.stateMessage || "", provider: p})
    })
  })
  return out
}
// T3 providerUpdateNotificationKey: a new version makes a new notice.
function key(list) {
  return (list || []).map(function(u) { return u.environment + "=" + u.driver + ":" + u.latestVersion }).sort().join("|")
}
// The notice to show: every candidate unless this exact set was dismissed.
function notice(list, dismissed, closed) {
  var k = key(list)
  if (!k || (dismissed || []).indexOf(k) >= 0 || (closed || {})[k]) return null
  return {key: k, updates: list}
}
// Remembered dismissals stay bounded; the newest last.
function dismiss(dismissed, k) {
  var all = (dismissed || []).filter(function(d) { return d !== k })
  all.push(k)
  return all.slice(-32)
}
