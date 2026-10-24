//! Local T3 client preferences. Never serialize the decrypted catalog to the UI.
//! The caller owns encrypted catalog persistence under the catalog management lock.
use crate::{read_catalog, t3_home, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, os::fd::AsRawFd, os::unix::fs::OpenOptionsExt, path::{Path, PathBuf}};

fn revision(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn settings_path() -> PathBuf { t3_home().join("userdata/client-settings.json") }
fn bytes(path: &Path) -> Result<Vec<u8>> {
    match fs::read(path) {
        Ok(contents) if contents.len() <= crate::MAX_BYTES as usize => Ok(contents),
        Ok(_) => Err("client_settings_too_large".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err("client_settings_read_failed".into()),
    }
}
fn document(raw: &[u8]) -> Result<Value> {
    if raw.is_empty() { return Ok(json!({})); }
    let value: Value = serde_json::from_slice(raw).map_err(|_| "invalid_client_settings")?;
    if !value.is_object() { return Err("invalid_client_settings".into()); }
    // Native accepts both legacy {settings: ...} and the current flat document.
    if value.get("settings").is_some_and(|v| !v.is_object()) { return Err("invalid_client_settings".into()); }
    Ok(value)
}
fn fields(value: &Value) -> &Value { value.get("settings").unwrap_or(value) }
fn fields_mut(value: &mut Value) -> &mut Value {
    if value.get("settings").is_some() { &mut value["settings"] } else { value }
}
fn preferences(value: &Value, rev: String) -> Value {
    let settings = fields(value);
    let mut out=json!({"revision":rev,"loadBalancingEnabled":settings["loadBalancingEnabled"] == true,
        "planModeEnabled":settings["planModeEnabled"]==true,
        // T3 followUpBehavior: what a message sent during a run does (queue by default).
        "followUpBehavior":if settings["followUpBehavior"]=="steer"{"steer"}else{"queue"},
        "mathRenderingEnabled":settings["mathRenderingEnabled"]==true,
        "confirmThreadDelete":settings["confirmThreadDelete"]!=false,
        "diffLayout":if settings["diffLayout"]=="split"{"split"}else{"stacked"},
        "diffIgnoreWhitespace":settings["diffIgnoreWhitespace"]!=false,
        "wordWrap":settings["wordWrap"]!=false,"diffFilesCollapsed":settings["diffFilesCollapsed"]!=false,
        "loadBalancingWeights":settings["loadBalancingWeights"].as_object().cloned().unwrap_or_default()});
    if let Some(browser)=crate::browser_preferences::projection(settings).as_object(){for (key,value) in browser {out[key]=value.clone();}}
    out
}
/// Read safe client defaults and a revision of the canonical client-settings file.
/// Raw client settings fields, for runtime choices such as the search engine.
pub fn client_settings() -> Result<Value> {
    Ok(fields(&document(&bytes(&settings_path())?)?).clone())
}
pub fn read_client_preferences() -> Result<Value> {
    read_client_preferences_at(&settings_path())
}
pub fn read_client_preferences_at(path: &Path) -> Result<Value> {
    let raw = bytes(path)?;
    Ok(preferences(&document(&raw)?, revision(&raw)))
}
/// CAS under an ot3 lock. Native Electron does not take this lock: compare again
/// just before rename and fail on a detected external edit rather than overwrite it.
pub fn update_client_preferences_at(path: &Path, expected: &str, patch: &Value) -> Result<Value> {
    let parent = path.parent().ok_or("client_settings_write_failed")?;
    fs::create_dir_all(parent).map_err(|_| "client_settings_write_failed")?;
    let lock = fs::OpenOptions::new().create(true).read(true).write(true).mode(0o600)
        .custom_flags(libc::O_NOFOLLOW).open(parent.join(".ot3-client-settings.lock"))
        .map_err(|_| "client_settings_write_failed")?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 { return Err("client_settings_write_failed".into()); }
    let destination = if path.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        fs::canonicalize(path).map_err(|_| "client_settings_read_failed")?
    } else { path.to_path_buf() };
    let raw = bytes(&destination)?;
    if revision(&raw) != expected { return Err("client_settings_changed_reload".into()); }
    let mut value = document(&raw)?;
    if !patch.is_object() || patch.as_object().is_some_and(|o| o.is_empty() || o.keys().any(|k| !crate::browser_preferences::KEYS.contains(&k.as_str()) && k != "diffLayout" && k != "planModeEnabled" && k != "followUpBehavior" && k != "mathRenderingEnabled" && k != "loadBalancingEnabled" && k != "environmentId" && k != "weight")) {
        return Err("invalid_client_preference".into());
    }
    let target = fields_mut(&mut value);
    for key in crate::browser_preferences::KEYS {if let Some(next)=patch.get(*key){target[*key]=crate::browser_preferences::validate(key,next)?;}}
    if let Some(layout)=patch.get("diffLayout"){
        if !matches!(layout.as_str(),Some("stacked"|"split")){return Err("invalid_client_preference".into());}
        target["diffLayout"]=layout.clone();
    }
    if let Some(enabled)=patch.get("mathRenderingEnabled"){
        target["mathRenderingEnabled"]=json!(enabled.as_bool().ok_or("invalid_client_preference")?);
    }
    if let Some(behavior)=patch.get("followUpBehavior"){
        if !matches!(behavior.as_str(),Some("queue"|"steer")){return Err("invalid_client_preference".into());}
        target["followUpBehavior"]=behavior.clone();
    }
    if let Some(enabled)=patch.get("planModeEnabled"){
        target["planModeEnabled"]=json!(enabled.as_bool().ok_or("invalid_client_preference")?);
    }
    if let Some(enabled) = patch.get("loadBalancingEnabled") {
        target["loadBalancingEnabled"] = json!(enabled.as_bool().ok_or("invalid_client_preference")?);
    }
    if patch.get("weight").is_some() || patch.get("environmentId").is_some() {
        let id = patch["environmentId"].as_str().filter(|s| !s.trim().is_empty() && s.trim() == *s && s.len() <= 2048).ok_or("invalid_client_preference")?;
        let weight = patch["weight"].as_u64().filter(|w| matches!(w, 0 | 25 | 50 | 100)).ok_or("invalid_client_preference")?;
        if !target["loadBalancingWeights"].is_object() && !target["loadBalancingWeights"].is_null() { return Err("invalid_client_settings".into()); }
        if !target["loadBalancingWeights"].is_object() { target["loadBalancingWeights"] = json!({}); }
        target["loadBalancingWeights"][id] = json!(weight);
    }
    let output = serde_json::to_vec(&value).map_err(|_| "client_settings_write_failed")?;
    let temp = destination.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result: Result<()> = (|| -> Result<()> {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp).map_err(|_| "client_settings_write_failed")?;
        f.write_all(&output).and_then(|_| f.write_all(b"\n")).and_then(|_| f.sync_all()).map_err(|_| "client_settings_write_failed")?;
        if revision(&bytes(&destination)?) != expected { return Err("client_settings_changed_reload".into()); }
        fs::rename(&temp, &destination).map_err(|_| "client_settings_write_failed")?;
        fs::File::open(destination.parent().ok_or("client_settings_write_failed")?).and_then(|d| d.sync_all()).map_err(|_| "client_settings_write_failed")?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp); }
    result?;
    let current = bytes(&destination)?;
    Ok(preferences(&document(&current)?, revision(&current)))
}
pub fn update_client_preferences(expected: &str, patch: &Value) -> Result<Value> {
    update_client_preferences_at(&settings_path(), expected, patch)
}

fn url_key(base: &str, scheme: &str) -> Option<String> {
    let url = crate::Url::parse(base).ok()?;
    if !scheme.split('|').any(|s| url.scheme() == s) || !url.username().is_empty() || url.password().is_some() || url.host_str().is_none() { return None; }
    // URL::href in JS includes the slash for an empty root path; only trailing
    // slashes are stripped from the full href by the native trust algorithm.
    Some(url.as_str().trim_end_matches('/').to_string())
}
fn route_key(target: &Value, profiles: &[Value]) -> Option<String> {
    let tag = target["_tag"].as_str()?;
    let env = target["environmentId"].as_str()?.to_string();
    let profile = profiles.iter().find(|p| p["connectionId"] == target["connectionId"] && p["environmentId"] == env);
    let key = match tag {
        "RelayConnectionTarget" => json!([tag, env]),
        "SshConnectionTarget" => {
            let p = profile.filter(|p| p["_tag"] == "SshConnectionProfile")?;
            let ssh = &p["target"];
            json!([tag, env, ssh["alias"], ssh["hostname"], ssh["username"], ssh["port"]])
        }
        "PrimaryConnectionTarget" | "BearerConnectionTarget" => {
            let p = if tag == "BearerConnectionTarget" { profile.filter(|p| p["_tag"] == "BearerConnectionProfile")? } else { target };
            json!([tag, env, url_key(p["httpBaseUrl"].as_str()?, "http|https")?, url_key(p["wsBaseUrl"].as_str()?, "ws|wss")?])
        }
        _ => return None,
    };
    serde_json::to_string(&key).ok()
}
/// Key of all saved routes. Learned bearer routes do not change trust.
pub fn github_connection_key(catalog: &Value, environment_id: &str, primary: Option<&Value>) -> Option<String> {
    let targets = catalog["targets"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let profiles = catalog["profiles"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut routes: Vec<&Value> = primary.filter(|p| p["environmentId"] == environment_id).into_iter().collect();
    routes.extend(targets.iter().filter(|t| t["environmentId"] == environment_id));
    let mut keys = Vec::new();
    for target in routes {
        let learned = profiles.iter().find(|p| p["connectionId"] == target["connectionId"] && p["environmentId"] == environment_id)
            .is_some_and(|p| p["_tag"] == "BearerConnectionProfile" && p["learned"] == true);
        if !learned { keys.push(route_key(target, profiles)?); }
    }
    match keys.len() {
        0 => None,
        1 => keys.pop(),
        _ => { keys.sort(); serde_json::to_string(&keys).ok() }
    }
}
/// UI projection: no credentials, profiles, DPoP tokens or decrypted catalog.
/// `primary` must be the exact PrimaryConnectionTarget from native bootstrap,
/// not a synthetic `local` id. `enabled` is the set of switched-on ids.
pub fn github_preferences(catalog: &Value, primary: Option<&Value>, enabled: &[String]) -> Value {
    let mut ids = Vec::<String>::new();
    if let Some(p) = primary.and_then(|p| p["environmentId"].as_str()) { ids.push(p.to_string()); }
    if let Some(targets) = catalog["targets"].as_array() {
        for target in targets {
            if let Some(id) = target["environmentId"].as_str() {
                if !ids.iter().any(|known| known == id) { ids.push(id.to_string()); }
            }
        }
    }
    let disabled = catalog["disabledEnvironmentIds"].as_array();
    let entries: Vec<Value> = ids.into_iter().filter(|id| enabled.contains(id) && !disabled.is_some_and(|d| d.contains(&json!(id)))).map(|id| {
        let key = github_connection_key(catalog, &id, primary);
        let permission = key.as_ref().and_then(|key| catalog["githubRoutingPermissions"].as_array()?.iter().find(|p| p["connectionKey"] == *key).and_then(|p| p["permission"].as_str())).unwrap_or("off");
        json!({"environmentId":id,"permission":permission,"available":key.is_some()})
    }).collect();
    json!({"environments":entries})
}
/// Pure transform. The parent must hold the SAME catalog lock as connection
/// management, compare ciphertext revision, then encrypt/persist the returned
/// catalog with its existing transaction. Never pass the result into QML.
pub fn set_github_permission(catalog: &Value, primary: Option<&Value>, environment_id: &str, expected_key: &str, permission: &str) -> Result<Value> {
    if !matches!(permission, "off" | "read" | "read-write") { return Err("invalid_github_permission".into()); }
    if catalog["disabledEnvironmentIds"].as_array().is_some_and(|ids| ids.contains(&json!(environment_id))) { return Err("environment_disabled".into()); }
    let key = github_connection_key(catalog, environment_id, primary).ok_or("untrusted_connection_endpoint")?;
    if key != expected_key { return Err("catalog_changed_reload".into()); }
    let mut next = catalog.clone();
    let rows = next["githubRoutingPermissions"].as_array().cloned().unwrap_or_default();
    let mut rows: Vec<Value> = rows.into_iter().filter(|row| row["environmentId"] != environment_id).collect();
    if permission != "off" { rows.push(json!({"environmentId":environment_id,"connectionKey":key,"permission":permission})); }
    next["githubRoutingPermissions"] = json!(rows);
    Ok(next)
}

/// T3's free-CPU/memory scoring, for caller-filtered connected project/provider
/// candidates only. Caller must not move explicit targets or attachment drafts.
pub fn choose_load_balanced_environment(candidates: &[Value], now_ms: i64) -> Option<String> {
    let mut best = 0.0;
    let mut selected = None;
    for candidate in candidates {
        let Some(weight) = candidate["weight"].as_f64() else { continue };
        let r = &candidate["resources"];
        let Some(cpu) = r["cpuUtilization"].as_f64() else { continue };
        let Some(count) = r["cpuCount"].as_f64() else { continue };
        let Some(total) = r["totalMemoryBytes"].as_f64() else { continue };
        let Some(available) = r["availableMemoryBytes"].as_f64() else { continue };
        let Some(time) = candidate["receivedAt"].as_i64().or_else(|| r["sampledAt"].as_i64()) else { continue };
        if !weight.is_finite() || weight <= 0.0 || cpu >= 0.95 || cpu < 0.0 || count <= 0.0 || total <= 0.0 || available / total <= 0.05 || now_ms.saturating_sub(time) > 15_000 || time > now_ms.saturating_add(5_000) { continue; }
        let score = weight * count * (1.0 - cpu) * available / total;
        if score > best {
            best = score;
            selected = candidate["environmentId"].as_str().map(str::to_string);
        }
    }
    selected
}

/// Apply the native per-client switch/weights before scoring. Candidates must
/// already represent connected machines hosting the same logical project and
/// an installed, authenticated, enabled matching provider.
pub fn choose_shared_project_environment(settings: &Value, candidates: &[Value], now_ms: i64) -> Option<String> {
    if settings["loadBalancingEnabled"] != true || candidates.len() < 2 { return None; }
    let weighted: Vec<Value> = candidates.iter().map(|candidate| {
        let mut value = candidate.clone();
        let id = candidate["environmentId"].as_str().unwrap_or("");
        value["weight"] = settings["loadBalancingWeights"][id].as_u64().map_or(json!(50), |weight| json!(weight));
        value
    }).collect();
    choose_load_balanced_environment(&weighted, now_ms)
}

/// Obtains the native encrypted catalog without returning it to the client.
pub fn read_github_preferences(primary: Option<&Value>, enabled: &[String]) -> Result<Value> {
    Ok(github_preferences(&read_catalog()?, primary, enabled))
}
