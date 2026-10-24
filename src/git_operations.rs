//! Shared native Git/script transport, trusted read cache and durable non-replaying journal.
use crate::{
    git_controls, json, read_json, save_json, text, xdg, Client, Environment, Result,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    collections::HashMap,
    fs::{File, OpenOptions},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
static VIEWS: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
fn cache() -> &'static Mutex<HashMap<String, Value>> {
    VIEWS.get_or_init(Default::default)
}
fn scopes(e: &Environment) -> Result<Value> {
    let session = Client::new()?.get(e, "/api/auth/session", false)?;
    if session["authenticated"] != true {
        return Err("unauthorized".into());
    }
    Ok(session
        .get("permissions")
        .or_else(|| session.get("scopes"))
        .cloned()
        .unwrap_or(json!([])))
}
fn has(scopes: &Value, scope: &str) -> bool {
    scopes
        .as_array()
        .is_some_and(|s| s.iter().any(|v| v == scope))
}
fn authorized(e: &Environment, operation: &str) -> Result<()> {
    let s = scopes(e)?;
    let okay = if operation == "script" {
        has(&s, "terminal:operate") && has(&s, "terminal:read")
    } else {
        has(&s, "source-control:write")
    };
    if okay {
        Ok(())
    } else {
        Err("permission_denied".into())
    }
}
fn base(scope: &Value) -> Result<PathBuf> {
    let env = scope["environment"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("git_scope_changed")?;
    let cwd = scope["cwd"]
        .as_str()
        .filter(|s| s.starts_with('/'))
        .ok_or("git_scope_changed")?;
    Ok(xdg("XDG_STATE_HOME", ".local/state")
        .join("outbox/git")
        .join(format!("{:x}", Sha256::digest(format!("{env}\0{cwd}")))))
}
struct CheckoutLock(File);
impl Drop for CheckoutLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
fn lock(scope: &Value) -> Result<CheckoutLock> {
    let p = base(scope)?.with_extension("lock");
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|_| "local_error")?;
    let f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(p)
        .map_err(|_| "local_error")?;
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("git_operation_busy".into());
    }
    Ok(CheckoutLock(f))
}
fn record_path(scope: &Value) -> Result<PathBuf> {
    Ok(base(scope)?.with_extension("json"))
}
fn action_path(id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id).map_err(|_| "invalid_request_id")?;
    Ok(xdg("XDG_STATE_HOME", ".local/state")
        .join("outbox/git-actions")
        .join(format!("{id}.json")))
}
fn save(scope: &Value, record: &Value) -> Result<()> {
    save_json(&action_path(text(record, "actionId"))?, record)?;
    save_json(&record_path(scope)?, record)
}
fn record(scope: &Value) -> Result<Value> {
    let p = record_path(scope)?;
    if p.exists() {
        read_json(&p)
    } else {
        Ok(json!({}))
    }
}
/// Public receipt deliberately omits script command text, file selections and hook output.
fn receipt(r: &Value) -> Value {
    json!({"state":r["state"],"actionId":r["actionId"],"operation":r["operation"],"scope":r["scope"],"phase":r["phase"],"error":r["error"],"result":r["result"],"terminalId":r["terminalId"],"terminalError":r["terminalError"]})
}
pub fn status(e: &Environment, input: &Value) -> Result<Value> {
    if input["scope"]["environment"] != e.id {
        return Err("git_scope_changed".into());
    }
    let scope = &input["scope"];
    let mut r = record(scope)?;
    // A persisted pending entry without a live checkout lock came from a lost worker.
    if r["state"] == "pending" {
        if let Ok(_guard) = lock(scope) {
            r["state"] = json!("unknown");
            r["error"] = json!("disconnected");
            save(scope, &r)?;
        }
    }
    Ok(receipt(&r))
}
pub fn read(e: &Environment, input: &Value) -> Result<Value> {
    uuid::Uuid::parse_str(text(input, "requestId")).map_err(|_| "invalid_request_id")?;
    let mut view = git_controls::execute(e, input)?;
    let permissions = scopes(e)?;
    view["permissions"] = json!({"filesystemRead":has(&permissions,"filesystem:read"),"sourceControlWrite":has(&permissions,"source-control:write"),"terminalWrite":has(&permissions,"terminal:operate")&&has(&permissions,"terminal:read")&&crate::terminal::launcher_available()});
    view["pullRequestReference"] = pull_request_reference(&view["scope"], &view["status"]["pr"]);
    view["operation"] = status(e, &json!({"scope":view["scope"]}))?;
    let mut views = cache().lock().map_err(|_| "local_error")?;
    if views.len() >= 32 {
        views.clear();
    }
    views.insert(text(input, "requestId").into(), view.clone());
    Ok(view)
}
/// No generic RPC from UI. Every action is built from the cached Rust/native snapshot
/// and a second authoritative read; durable pending is saved before the first mutation.
pub fn action(e: &Environment, input: &Value) -> Result<Value> {
    let id = text(input, "requestId");
    uuid::Uuid::parse_str(id).map_err(|_| "invalid_request_id")?;
    let intent = &input["intent"];
    let scope = &intent["scope"];
    if scope["environment"] != e.id
        || input["thread"] != scope["thread"]
        || intent["actionId"] != id
    {
        return Err("git_scope_changed".into());
    }
    let operation = text(intent, "operation");
    authorized(e, operation)?;
    let _guard = lock(scope)?;
    let archived = action_path(id)?;
    if archived.exists() {
        let mut previous = read_json(&archived)?;
        if previous["intent"] != *intent {
            return Err("retry_payload_changed".into());
        }
        if previous["state"] == "pending" {
            previous["state"] = json!("unknown");
            previous["error"] = json!("disconnected");
            save_json(&archived, &previous)?;
        }
        return Ok(receipt(&previous));
    }
    let previous = record(scope)?;
    // All duplicate deliveries return the stored outcome. Neither pending nor uncertain
    // scripts/pulls/streaming Git actions are replayed, even with the same action ID.
    if previous["actionId"] == id {
        if previous["intent"] != *intent {
            return Err("retry_payload_changed".into());
        }
        return Ok(receipt(&previous));
    }
    if matches!(previous["state"].as_str(), Some("pending" | "unknown")) {
        return Err("git_operation_unknown".into());
    }
    let current = cache()
        .lock()
        .map_err(|_| "local_error")?
        .get(text(scope, "requestId"))
        .cloned()
        .ok_or("git_stale_result")?;
    let fresh = git_controls::execute(e, scope)?;
    let plan = git_controls::build_verified_action(&current, &fresh, intent)?;
    let mut r = json!({"scope":scope,"intent":intent,"actionId":id,"operation":operation,"plan":plan,"state":"pending","phase":"","error":"","result":null,"terminalId":null});
    // Save exact intent and all native steps under the same checkout lock before sending.
    save(scope, &r)?;
    let c = e.connection(true)?;
    let outcome = (|| -> Result<()> {
        for step in plan["steps"].as_array().ok_or("invalid_git_plan")? {
            let tag = text(step, "tag");
            let payload = &step["payload"];
            if tag == "git.runStackedAction" {
                let finished = Cell::new(false);
                let streamed = c.subscribe(
                    tag,
                    payload.clone(),
                    || finished.get(),
                    |event| {
                        if let Some(event) = event {
                            if event["actionId"] != id
                                || event["cwd"] != scope["cwd"]
                                || event["action"] != operation
                            {
                                return Err("git_scope_changed".into());
                            }
                            match text(&event, "kind") {
                                "phase_started" => {
                                    r["phase"] = event["phase"].clone();
                                    save(scope, &r)?;
                                }
                                "action_finished" => {
                                    validate_result(operation, &event["result"])?;
                                    r["state"] = json!("succeeded");
                                    r["result"] = event["result"].clone();
                                    save(scope, &r)?;
                                    finished.set(true);
                                }
                                "action_failed" => {
                                    r["state"] = json!("failed");
                                    r["error"] = json!(text(&event, "message")
                                        .chars()
                                        .take(4096)
                                        .collect::<String>());
                                    save(scope, &r)?;
                                    finished.set(true);
                                }
                                _ => {}
                            }
                        }
                        Ok(())
                    },
                );
                if !finished.get() {
                    return Err(streamed.err().unwrap_or("stream_closed".into()));
                }
            } else {
                let result = c.rpc(tag, payload.clone())?;
                if tag == "vcs.pull" {
                    if !matches!(
                        result["status"].as_str(),
                        Some("pulled" | "skipped_up_to_date")
                    ) {
                        return Err("invalid_git_result".into());
                    }
                    r["result"] = result;
                    r["state"] = json!("succeeded");
                }
                if tag == "terminal.open" {
                    r["terminalId"] = payload["terminalId"].clone();
                    save(scope, &r)?;
                }
                if tag == "terminal.write" {
                    r["state"] = json!("submitted");
                }
            }
        }
        Ok(())
    })();
    if let Err(error) = outcome {
        if !matches!(
            r["state"].as_str(),
            Some("succeeded" | "failed" | "submitted")
        ) {
            r["state"] = json!("unknown");
            r["error"] = json!(error);
        }
    }
    save(scope, &r)?;
    if operation == "script" && r["terminalId"].is_string() {
        if let Err(error) =
            crate::terminal::launch_session(e, text(scope, "thread"), text(&r, "terminalId"))
        {
            r["terminalError"] = json!(error);
            save(scope, &r)?;
        }
    }
    Ok(receipt(&r))
}
fn validate_result(operation: &str, result: &Value) -> Result<()> {
    if serde_json::to_vec(result)
        .map_err(|_| "invalid_git_result")?
        .len()
        > 1024 * 1024
    {
        return Err("git_result_limit".into());
    }
    if result["action"] != operation
        || !matches!(
            result["branch"]["status"].as_str(),
            Some("created" | "skipped_not_requested")
        )
        || !matches!(
            result["commit"]["status"].as_str(),
            Some("created" | "skipped_no_changes" | "skipped_not_requested")
        )
        || !matches!(
            result["push"]["status"].as_str(),
            Some("pushed" | "skipped_up_to_date" | "skipped_not_requested")
        )
        || !matches!(
            result["pr"]["status"].as_str(),
            Some("created" | "opened_existing" | "skipped_not_requested")
        )
    {
        return Err("invalid_git_result".into());
    }
    Ok(())
}
/// Explicit reconciliation releases the checkout guard; it NEVER replays an action.
pub fn resolve(e: &Environment, input: &Value) -> Result<Value> {
    let scope = &input["scope"];
    if scope["environment"] != e.id || input["confirmed"] != true {
        return Err("git_confirmation_required".into());
    }
    let _guard = lock(scope)?;
    let mut r = record(scope)?;
    authorized(e, text(&r, "operation"))?;
    if r["actionId"] != input["actionId"]
        || !matches!(r["state"].as_str(), Some("unknown" | "pending"))
    {
        return Err("git_scope_changed".into());
    }
    r["state"] = json!("reviewed");
    r["error"] = json!("");
    save(scope, &r)?;
    Ok(receipt(&r))
}
pub fn open_terminal(e: &Environment, input: &Value) -> Result<Value> {
    let scope = &input["scope"];
    if scope["environment"] != e.id {
        return Err("git_scope_changed".into());
    }
    let r = record(scope)?;
    if r["actionId"] != input["actionId"] || r["scope"]["thread"] != scope["thread"] {
        return Err("git_scope_changed".into());
    }
    authorized(e, "script")?;
    crate::terminal::launch_session(e, text(scope, "thread"), text(&r, "terminalId"))
}

/// Canonical native PR metadata into the existing scoped PR view reference.
/// Unsupported URL forms stay unavailable instead of guessing a repository.
pub fn pull_request_reference(scope: &Value, pr: &Value) -> Value {
    let Some(number) = pr["number"].as_u64().filter(|n| *n > 0) else {
        return Value::Null;
    };
    let Ok(url) = reqwest::Url::parse(text(pr, "url")) else {
        return Value::Null;
    };
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Value::Null;
    }
    let Some(host) = url.host_str() else {
        return Value::Null;
    };
    let path = url.path().trim_end_matches('/');
    for marker in ["/pull/", "/pulls/", "/-/merge_requests/", "/pull-requests/"] {
        if let Some((repository, id)) = path.rsplit_once(marker) {
            let repository = repository.trim_start_matches('/');
            if id.parse::<u64>().ok() == Some(number)
                && repository.split('/').count() >= 2
                && repository
                    .split('/')
                    .all(|p| !p.is_empty() && !matches!(p, "." | "..") && !p.contains('%'))
            {
                return json!({"environmentId":scope["environment"],"projectId":scope["projectId"],"host":host,"repository":repository,"number":number,"url":pr["url"],"snapshot":pr});
            }
        }
    }
    Value::Null
}
