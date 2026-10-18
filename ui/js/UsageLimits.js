.pragma library
.import "NativeUsageLimits.js" as Native
// Thin presentation adapter. Account identity, workspace disambiguation, fresh
// observation selection, pooling, pace and notices remain the original helpers.
function present(data, now) {
  var environments = new Map()
  ;(data.environments || []).forEach(function(e) {
    environments.set(e.id, {entry:{target:{label:e.name || e.id}},serverConfig:{providers:e.providers || [],usageLimitSources:e.usageLimitSources || []}})
  })
  if ((data.localSources || []).length) {
    var local = environments.get("local")
    if (!local) {local={entry:{target:{label:"This device"}},serverConfig:{providers:[],usageLimitSources:[]}};environments.set("ot3-local-credentials",local)}
    local.serverConfig.usageLimitSources=local.serverConfig.usageLimitSources.concat(data.localSources)
  }
  var accounts=Native.collectLimitAccounts(environments)
  var providers=Native.collectLimitPools(accounts,now).map(function(pool) {
    var rows=Native.displayLimitWindows(pool).map(function(w) {
      var expected=w.members.map(function(m){return expectedAvailable(m.window,now)}).filter(function(v){return v!==null})
      return {id:w.id,kind:w.kind,label:w.label,availablePercent:w.remainingPercent,pace:w.pace,
        expectedAvailable:expected.length?expected.reduce(function(sum,v){return sum+v},0)/expected.length:null,
        segments:w.columns.map(function(s){return s.window?{key:s.account.key,availablePercent:Native.remainingPercent(s.window),expectedAvailable:expectedAvailable(s.window,now),reset:Native.formatResetsIn(s.window,now)}:null}),
        nextReset:w.resets.length?Native.formatResetsIn(w.resets[0].member.window,now):""}
    })
    var accountRows=pool.accounts.map(function(a,i){
      var windows=a.limits.windows.map(function(w){
        return {label:w.label,availablePercent:Native.remainingPercent(w),reset:Native.formatResetsIn(w,now)}
      })
      return {key:a.key,label:a.displayName || a.email || ("Account "+(i+1)),plan:a.plan || "",
        hosts:a.environments.map(function(e){return e.label}).join(", ") || a.sourceLabel || "",windows:windows}
    })
    return {provider:pool.driver,label:label(pool.driver),accounts:accountRows,windows:rows,
      weekly:rows.find(function(w){return w.kind==="weekly"}) || null,shortWindow:rows.find(function(w){return w.kind==="session"}) || null}
  })
  var notices=Native.collectLimitNotices(environments)
  ;(data.environments || []).forEach(function(e){if(!e.connected)notices.push((e.name || e.id)+": "+(e.error==="permission_denied"?"Access denied":"Disconnected; readings may be outdated"));else if(e.refreshFailed)notices.push((e.name || e.id)+": Could not refresh limits")})
  return {providers:providers,notices:notices,external:Native.collectExternalUsageLinks(environments)}
}
function label(driver) {
  var labels={codex:"Codex",claudeAgent:"Claude",claudeCode:"Claude",claude:"Claude",opencode:"OpenCode Go",cursor:"Cursor",antigravity:"Antigravity","kimi-coding":"Kimi",nous:"Nous",grok:"Grok",pi:"Pi"}
  return labels[driver] || driver
}

function expectedAvailable(window,now){var elapsed=Native.elapsedShare(window,now);return elapsed===null?null:(1-elapsed)*100}
