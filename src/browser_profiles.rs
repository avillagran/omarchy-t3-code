//! Native browser profile cleanup. T3 owns server browser partitions. Metadata
//! is removed only after all applicable local and server targets acknowledge cleanup.
use crate::{json, protocol, text, Environment, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Mutex, OnceLock},
    time::Duration,
};
static VIEWS: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
pub(crate) static ACTION: Mutex<()> = Mutex::new(());
fn views() -> &'static Mutex<HashMap<String, Value>> {
    VIEWS.get_or_init(Default::default)
}
fn origin(e: &Environment) -> String {
    format!("{:x}", Sha256::digest(e.base_url.as_bytes()))
}
fn connection(e: &Environment) -> Result<protocol::Connection> {
    if let Some(error) = &e.connection_error {
        return Err(error.clone());
    }
    if let Some(c) = &e.connection {
        return Ok(c.as_ref().clone());
    }
    protocol::Connection::new(
        &e.base_url,
        None,
        protocol::Credential::bearer(&crate::environment_token(e, true)?)?,
    )
}
pub(crate) fn inspect(e: &Environment) -> Result<(protocol::Connection, Value)> {
    let c = connection(e)?;
    let auth = c.request_json("GET", "/api/auth/session", None, false)?;
    let has = |scope: &str| {
        auth["authenticated"] == true
            && auth
                .get("permissions")
                .or_else(|| auth.get("scopes"))
                .and_then(Value::as_array)
                .is_some_and(|p| p.iter().any(|v| v == scope))
    };
    if !has("orchestration:read") {
        return Err("permission_denied".into());
    }
    let can_clear = has("preview:operate");
    let config = c.rpc("server.getConfig", json!({}))?;
    let data = json!({"environment":e.id,"originKey":origin(e),"serverBrowser":config["environment"]["capabilities"]["serverBrowser"]==true,"canClear":can_clear,"error":""});
    Ok((c, data))
}
fn ids(input: &Value) -> Result<Vec<String>> {
    let values = input["environments"]
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 64)
        .ok_or("settings_scope_empty")?;
    let mut seen = HashSet::new();
    let mut out = vec![];
    for v in values {
        let id = v
            .as_str()
            .filter(|v| crate::valid_id(v))
            .ok_or("invalid_settings_scope")?;
        if !seen.insert(id) {
            return Err("invalid_settings_scope".into());
        }
        out.push(id.to_owned())
    }
    Ok(out)
}
fn store_key(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_owned())
        .to_string_lossy()
        .into_owned()
}
pub(crate) fn native_profiles(client: &Value) -> Vec<Value> {
    // Original resolveBrowserProfiles: built-ins cannot be shadowed, duplicates
    // first-win, user entries labelled persistent. Incognito is not a settings row.
    let mut out = vec![json!({"id":"default","name":"Default","kind":"persistent"})];
    let mut seen = HashSet::from(["default".to_owned(), "incognito".to_owned()]);
    for p in client["browserProfiles"].as_array().into_iter().flatten() {
        let id = text(p, "id");
        if seen.insert(id.to_owned()) {
            out.push(json!({"id":id,"name":p["name"],"kind":"persistent"}))
        }
    }
    out
}
pub fn read_at(environments: &[Environment], input: &Value, path: &Path) -> Result<Value> {
    read_with_at(environments, input, path, None)
}
pub fn read_with_at(
    environments: &[Environment],
    input: &Value,
    path: &Path,
    runtime: Option<&dyn crate::browser_cleanup::Runtime>,
) -> Result<Value> {
    let local = runtime.is_some_and(|r| r.available());
    let selected = ids(input)?;
    let client = crate::connection_preferences::read_client_preferences_at(path)?;
    let mut sessions = vec![];
    for id in selected {
        let e = environments.iter().find(|e| e.id == id);
        sessions.push(match e {Some(e)=>match inspect(e){Ok((_,s))=>s,Err(_)=>json!({"environment":id,"originKey":origin(e),"serverBrowser":false,"canClear":false,"error":"browser_profile_environment_unavailable"})},None=>json!({"environment":id,"serverBrowser":false,"canClear":false,"error":"unknown_environment"})});
    }
    let ready = sessions.iter().all(|s| s["error"] == "");
    let capable = sessions.iter().any(|s| s["serverBrowser"] == true);
    let available = ready
        && (capable || local)
        && sessions
            .iter()
            .filter(|s| s["serverBrowser"] == true)
            .all(|s| s["canClear"] == true);
    let id = uuid::Uuid::new_v4().to_string();
    let profiles = native_profiles(&client);
    let cached = json!({"sessions":sessions,"profiles":profiles,"clientStore":store_key(path),"localRuntime":local});
    let mut cache = views().lock().map_err(|_| "local_error")?;
    if cache.len() >= 128 {
        cache.clear();
    }
    cache.insert(id.clone(), cached);
    Ok(json!({"viewId":id,"sessions":sessions,"profiles":profiles,"available":available}))
}
pub fn apply_at(environments: &[Environment], input: &Value, path: &Path) -> Result<Value> {
    apply_with_at(environments, input, path, None)
}
pub fn apply_with_at(
    environments: &[Environment],
    input: &Value,
    path: &Path,
    runtime: Option<&dyn crate::browser_cleanup::Runtime>,
) -> Result<Value> {
    let local = runtime.is_some_and(|r| r.available());
    let _lock = ACTION.try_lock().map_err(|_| "browser_profiles_busy")?;
    if input["confirmed"] != true {
        return Err("confirmation_required".into());
    }
    let action = text(input, "action");
    if !matches!(action, "clear" | "remove") {
        return Err("invalid_browser_profile_action".into());
    }
    let profile_id = text(input, "profileId");
    let cached = views()
        .lock()
        .map_err(|_| "local_error")?
        .get(text(input, "viewId"))
        .cloned()
        .ok_or("browser_profile_view_expired")?;
    if cached["clientStore"] != store_key(path) || cached["localRuntime"] != local {
        return Err("browser_profile_scope_changed".into());
    }
    let profile = cached["profiles"]
        .as_array()
        .and_then(|p| p.iter().find(|p| p["id"] == profile_id))
        .ok_or("browser_profile_unavailable")?;
    if action == "remove" && profile_id == "default" {
        return Err("browser_profile_builtin".into());
    }
    let before = crate::connection_preferences::read_client_preferences_at(path)?;
    if !native_profiles(&before).iter().any(|p| p == profile) {
        return Err("browser_profile_changed_reload".into());
    }
    let mut plan = vec![];
    let mut capable = false;
    let mut scopes = vec![];
    for prior in cached["sessions"]
        .as_array()
        .ok_or("invalid_settings_scope")?
    {
        if prior["error"] != "" {
            return Err("browser_profile_environment_unavailable".into());
        }
        let e = environments
            .iter()
            .find(|e| e.id == text(prior, "environment"))
            .ok_or("browser_profile_scope_changed")?;
        let (c, fresh) = inspect(e)?;
        if fresh["originKey"] != prior["originKey"]
            || fresh["serverBrowser"] != prior["serverBrowser"]
        {
            return Err("browser_profile_scope_changed".into());
        }
        let supports = fresh["serverBrowser"] == true;
        if supports {
            capable = true;
            if fresh["canClear"] != true {
                return Err("permission_denied".into());
            }
        }
        scopes.push(crate::browser_cleanup::Scope {
            environment: e.id.clone(),
            origin_key: origin(e),
        });
        plan.push((e.id.clone(), c, supports));
    }
    if !capable && !local {
        return Err("browser_profile_cleanup_unavailable".into());
    }
    if views()
        .lock()
        .map_err(|_| "local_error")?
        .remove(text(input, "viewId"))
        .is_none()
    {
        return Err("browser_profile_view_expired".into());
    }
    // Keep the local reservation through every remote acknowledgement and metadata CAS.
    let mut lease = if local {
        Some(runtime.unwrap().begin(profile_id, path, &scopes)?)
    } else {
        None
    };
    let mut results = vec![];
    for (id, c, supports) in plan {
        let local_result = lease.as_mut().map(|l| l.clear(&id));
        let (server_state, server_error) = if !supports {
            ("skipped", "")
        } else {
            match c.rpc_with_timeout(
                "preview.clearProfile",
                json!({"profileId":profile_id}),
                Duration::from_secs(60),
            ) {
                Ok(_) => ("complete", ""),
                Err(e) if e == "write_not_authorized" => ("failed", "permission_denied"),
                Err(e) if e == "rpc_rejected" => ("failed", "browser_profile_clear_failed"),
                Err(_) => ("uncertain", "browser_profile_outcome_unknown"),
            }
        };
        let (local_state, local_error) = match &local_result {
            None => ("skipped", ""),
            Some(Ok(())) => ("complete", ""),
            Some(Err(e))
                if matches!(
                    e.as_str(),
                    "browser_profile_busy"
                        | "browser_profiles_busy"
                        | "browser_profile_storage_unsafe"
                        | "browser_profile_scope_changed"
                ) =>
            {
                ("failed", e.as_str())
            }
            Some(Err(_)) => ("uncertain", "browser_profile_outcome_unknown"),
        };
        let (state, error) = if local_state == "uncertain" || server_state == "uncertain" {
            ("uncertain", "browser_profile_outcome_unknown")
        } else if local_state == "failed" {
            (local_state, local_error)
        } else if server_state == "failed" {
            (server_state, server_error)
        } else if local_state == "complete" || server_state == "complete" {
            ("complete", "")
        } else {
            ("skipped", "")
        };
        results.push(json!({"environment":id,"state":state,"error":error,"local":{"state":local_state,"error":local_error},"server":{"state":server_state,"error":server_error}}));
    }
    if results
        .iter()
        .any(|r| matches!(text(r, "state"), "failed" | "uncertain"))
    {
        return Ok(
            json!({"action":action,"profileId":profile_id,"state":if results.iter().any(|r|r["state"]=="uncertain"){"uncertain"}else{"failed"},"results":results,"error":"browser_profile_clear_incomplete"}),
        );
    }
    let updated = (|| -> Result<Value> {
        let current = crate::connection_preferences::read_client_preferences_at(path)?;
        if action == "clear" {
            return Ok(current);
        }
        // Rebase only metadata removal onto the newest settings, preserving unrelated
        // edits made during cleanup. Changed profile identity/name requires review.
        let current_profiles = native_profiles(&current);
        if let Some(p) = current_profiles.iter().find(|p| p["id"] == profile_id) {
            if p != profile {
                return Err("browser_profile_changed_reload".into());
            }
        }
        let profiles = current["browserProfiles"]
            .as_array()
            .ok_or("invalid_client_settings")?
            .iter()
            .filter(|p| p["id"] != profile_id)
            .cloned()
            .collect::<Vec<_>>();
        let mut patch = json!({"browserProfiles":profiles});
        if current["browserDefaultProfileId"] == profile_id {
            patch["browserDefaultProfileId"] = json!("default")
        }
        crate::connection_preferences::update_client_preferences_at(
            path,
            text(&current, "revision"),
            &patch,
        )
    })();
    Ok(match updated {
        Ok(data) => {
            json!({"action":action,"profileId":profile_id,"state":"complete","results":results,"data":data})
        }
        Err(error) => {
            json!({"action":action,"profileId":profile_id,"state":"uncertain","results":results,"error":error})
        }
    })
}
pub fn execute(input: &Value) -> Result<Value> {
    execute_with(input, None)
}
pub fn execute_with(
    input: &Value,
    runtime: Option<&dyn crate::browser_cleanup::Runtime>,
) -> Result<Value> {
    let path = crate::t3_home().join("userdata/client-settings.json");
    let environments = crate::environments()?;
    match text(input, "op") {
        "browser-profiles-read" => read_with_at(&environments, input, &path, runtime),
        "browser-profiles-action" => apply_with_at(&environments, input, &path, runtime),
        _ => Err("invalid_browser_profile_action".into()),
    }
}
