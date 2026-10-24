//! Provider (harness) updates, after T3's ProviderUpdateLaunchNotification:
//! which providers are behind their latest version and whether T3 can update
//! them in one step (server.updateProvider). The update command itself always
//! runs on the server; ot3 only names the provider.
use crate::*;
use serde_json::{json, Value};
use std::time::Duration;
use t3_protocol as protocol;

/// T3 PROVIDER_DISPLAY_NAMES.
pub fn display_name(driver: &str) -> &str {
    match driver {
        "antigravity" => "Antigravity",
        "codex" => "Codex",
        "claudeAgent" => "Claude",
        "cursor" => "Cursor",
        "grok" => "Grok",
        "muse" => "Muse Code",
        "acpRegistry" => "ACP Registry",
        "pi" => "Pi",
        "opencode" => "OpenCode",
        other => other,
    }
}

/// What the panel needs from one native provider entry; null when no newer
/// version is known (T3 isProviderUpdateCandidate).
pub fn advisory(native: &Value) -> Value {
    let a = &native["versionAdvisory"];
    let broken = matches!(native["compatibilityAdvisory"]["latestVersionStatus"].as_str(), Some("broken" | "unsupported"));
    let Some(latest) = a["latestVersion"].as_str().filter(|_| native["enabled"] == true && !broken && a["status"] == "behind_latest") else {
        return Value::Null;
    };
    let state = native["updateState"]["status"].as_str().unwrap_or("idle");
    let one_click = a["canUpdate"] == true && a["updateCommand"].as_str().is_some_and(|c| !c.trim().is_empty()) && !matches!(state, "queued" | "running");
    let driver = text(native, "driver");
    json!({"driver":driver,"name":display_name(driver),"currentVersion":a["currentVersion"],"latestVersion":latest,
        "canUpdate":a["canUpdate"]==true,"oneClick":one_click,"state":state,"stateMessage":native["updateState"]["message"]})
}

/// server.updateProvider answers only once the update finished (T3 allows it five
/// minutes, and queues updates behind each other), so it gets the long RPC
/// deadline. If the answer still never arrives, the server's own update state
/// says what happened instead of a bare "outcome unknown".
pub fn run_update(e: &Environment, driver: &str, instance: &str) -> Result<Value> {
    let payload = json!({"provider":driver,"instanceId":instance});
    let result = e
        .connection(true)
        .and_then(|c| c.rpc_with_timeout("server.updateProvider", payload, Duration::from_secs(300)));
    match result {
        Err(error) if protocol::error_code(&error) == "rpc_outcome_unknown" => {
            let config = protocol::api::server::config(&e.connection(true)?)?;
            let state = config["providers"].as_array().into_iter().flatten().find(|p| p["instanceId"] == instance).map(|p| p["updateState"].clone()).unwrap_or(Value::Null);
            if state.is_null() {
                return Err(error);
            }
            Ok(json!({"updateState":state,"outcomeUnknown":true}))
        }
        other => other,
    }
}

/// One-click update of a provider the server itself reports as updatable.
pub fn update(e: &Environment, input: &Value) -> Result<Value> {
    let instance = text(input, "instanceId");
    if !valid_id(instance) {
        return Err("invalid_provider".into());
    }
    let config = protocol::api::server::config(&e.connection(true)?)?;
    let native = config["providers"].as_array().into_iter().flatten().find(|p| p["instanceId"] == instance).ok_or("invalid_provider")?;
    let a = advisory(native);
    if a["oneClick"] != true {
        return Err("provider_update_unavailable".into());
    }
    let result = run_update(e, text(native, "driver"), instance)?;
    Ok(json!({"environment":e.id,"instanceId":instance,"driver":native["driver"],"queued":true,"result":result}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn provider(status: &str, can: bool, command: Value) -> Value {
        json!({"instanceId":"claudeAgent","driver":"claudeAgent","enabled":true,"compatibilityAdvisory":{"latestVersionStatus":"supported"},
            "versionAdvisory":{"status":status,"currentVersion":"2.1.296","latestVersion":"2.1.307","canUpdate":can,"updateCommand":command},"updateState":{"status":null}})
    }
    #[test]
    fn only_providers_behind_their_latest_version_are_candidates() {
        let a = advisory(&provider("behind_latest", true, json!("npm i -g x")));
        assert_eq!(a["name"], "Claude");
        assert_eq!(a["latestVersion"], "2.1.307");
        assert_eq!(a["oneClick"], true);
        assert!(advisory(&provider("current", true, json!("npm i -g x"))).is_null());
        let manual = advisory(&provider("behind_latest", false, Value::Null));
        assert_eq!(manual["oneClick"], false);
        let mut off = provider("behind_latest", true, json!("x"));
        off["enabled"] = json!(false);
        assert!(advisory(&off).is_null());
        let mut broken = provider("behind_latest", true, json!("x"));
        broken["compatibilityAdvisory"]["latestVersionStatus"] = json!("broken");
        assert!(advisory(&broken).is_null());
        let mut running = provider("behind_latest", true, json!("x"));
        running["updateState"]["status"] = json!("running");
        assert_eq!(advisory(&running)["oneClick"], false);
        assert_eq!(advisory(&running)["state"], "running");
    }
}
