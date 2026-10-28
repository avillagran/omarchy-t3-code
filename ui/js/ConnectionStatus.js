.pragma library
// Describe only what the reported error establishes; do not infer authentication
// failures from an unavailable SSH transport.
function reason(code) {
    switch (code) {
    case "ssh_unavailable": return "The SSH connection or tunnel could not be established."
    case "ssh_timeout": return "The SSH connection timed out."
    case "t3_auth_unavailable": case "unauthorized": case "t3_credentials_unavailable": return "T3 access could not be authenticated."
    case "t3_not_running": case "t3_remote_start_failed": return "The T3 server is not running or did not become ready."
    case "t3_environment_mismatch": return "The host answered with a different T3 environment."
    case "catalog_locked": return "The saved connections could not be unlocked."
    case "unreachable": case "disconnected": return "The connection to T3 was interrupted."
    default: return ""
    }
}
function failures(bridge) {
    var rows = ((bridge.state || {}).environments || []).filter(function(e) { return e.state && e.state !== "ready" })
    if ((bridge.backendStart || {}).error && !rows.some(function(e) { return e.id === "local" })) rows.push({id:"local",name:"T3 Code",state:bridge.backendStart.error})
    if (!rows.length && bridge.stale && bridge.error) rows = [{id:"client",name:"O3",state:bridge.error}]
    return rows
}
