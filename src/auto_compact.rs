//! Desktop lifecycle adapter. One worker, driven by the existing metadata poll.
//! The portable policy is in t3-protocol; this module owns private claims and RPC.
use omarchy_t3_client::*;
use protocol::compaction as policy;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct Control(mpsc::SyncSender<Value>);
impl Control {
    pub fn new(refresh: mpsc::Sender<Value>) -> Self {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = run(rx, refresh);
            if let Err(error) = result {
                crate::emit(
                    "compaction",
                    json!({"key":"@engine","status":"error","error":error}),
                );
            }
        });
        Self(tx)
    }
    pub fn observe(&self, snapshot: &Value) {
        let _ = self.0.try_send(snapshot.clone());
    }
}
fn emit(key: &str, v: &Value) {
    let mut out = v.clone();
    out["key"] = json!(key);
    crate::emit("compaction", out)
}
fn bounded(client: &Client, e: &Environment, id: &str) -> Result<Value> {
    if !valid_id(id) {
        return Err("invalid_id".into());
    }
    client.bounded_thread(e, id)
}
fn supported(
    e: &Environment,
    instance: &str,
    cache: &mut HashMap<String, (Instant, Value)>,
) -> Result<bool> {
    if cache
        .get(&e.id)
        .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(300))
    {
        let c = write::rpc(e, "server.getConfig", json!({}))?;
        let providers=c["providers"].as_array().into_iter().flatten().map(|p|json!({"instanceId":p["instanceId"],"enabled":p["enabled"],"commands":p["slashCommands"]})).collect::<Vec<_>>();
        cache.insert(e.id.clone(), (Instant::now(), json!(providers)));
    }
    Ok(cache[&e.id].1.as_array().into_iter().flatten().any(|p| {
        p["instanceId"] == instance
            && p["enabled"] == true
            && p["commands"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|c| c["name"] == "compact")
    }))
}
fn run(rx: mpsc::Receiver<Value>, refresh: mpsc::Sender<Value>) -> Result<()> {
    let dir = xdg("XDG_STATE_HOME", ".local/state").join("compaction");
    fs::create_dir_all(&dir).map_err(|_| "local_error")?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|_| "local_error")?;
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(dir.join("worker.lock"))
        .map_err(|_| "local_error")?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("compaction_worker_already_running".into());
    }
    let file = dir.join("ledger.json");
    let mut ledger = if file.exists() {
        read_json(&file)?
    } else {
        json!({})
    };
    if !ledger.is_object() {
        return Err("invalid_compaction_ledger".into());
    }
    let mut interrupted = false;
    for record in ledger.as_object_mut().unwrap().values_mut() {
        if record["status"] == "claimed" {
            record["status"] = json!("uncertain");
            record["error"] = json!("compaction_outcome_unknown");
            interrupted = true;
        }
    }
    if interrupted {
        save_json(&file, &ledger)?;
    }
    let client = Client::new()?;
    let mut checked: HashMap<String, String> = HashMap::new();
    let mut catalogs = HashMap::new();
    let mut statuses: HashMap<String, Value> = HashMap::new();
    let mut settings_before = Value::Null;
    for snapshot in rx {
        let settings = crate::ui_state::load();
        // General integration fixtures must never acquire side effects by default.
        let enabled = policy::enabled(&settings)
            && (!std::env::var_os("OMARCHY_T3_FIXTURE_ENVIRONMENTS").is_some()
                || settings["autoCompactEnabled"] == true);
        let settings_signature = json!([
            settings["autoCompactEnabled"],
            settings["autoCompactFallbackMinutes"],
            settings["autoCompactMinTokens"],
            settings["autoCompactProviderMinutes"],
            settings["autoCompactExcludedThreads"]
        ]);
        if settings_before != settings_signature {
            checked.clear();
            settings_before = settings_signature;
        }
        emit(
            "@engine",
            &json!({"status":if enabled{"enabled"}else{"disabled"}}),
        );
        if !enabled {
            continue;
        }
        let now = chrono::Utc::now().timestamp_millis();
        let mut reads = 0;
        for thread in snapshot["threads"].as_array().into_iter().flatten() {
            let key = text(thread, "key");
            let env = text(thread, "environmentId");
            let id = text(thread, "id");
            let excluded = settings["autoCompactExcludedThreads"]
                .as_array()
                .is_some_and(|a| a.contains(&json!(key)));
            let activity = thread["compactActivityAt"].as_i64().unwrap_or(0);
            let stamp = format!(
                "{}:{}:{}:{}",
                activity,
                text(thread, "compactRunId"),
                thread["modelSelection"],
                thread["updatedAt"]
            );
            let old = ledger[key].clone();
            let due =
                old["status"] == "scheduled" && old["dueAt"].as_i64().is_some_and(|d| now >= d);
            if thread["settled"] == true {
                // Retire a pending deadline without losing a consumed episode's claim.
                if old["status"] == "scheduled" {
                    ledger[key]["status"] = json!("unavailable");
                    save_json(&file, &ledger)?;
                }
                checked.remove(key);
                let v = json!({"status":"unavailable"});
                if statuses.get(key) != Some(&v) {
                    emit(key, &v);
                    statuses.insert(key.into(), v);
                }
                continue;
            }
            if excluded
                || thread["working"] == true
                || thread["canSnooze"] == false
                || thread["relationshipToParent"] == "subagent"
            {
                let v = json!({"status":if excluded{"disabled"}else{"busy"}});
                if statuses.get(key) != Some(&v) {
                    emit(key, &v);
                    statuses.insert(key.into(), v);
                }
                continue;
            }
            if activity <= 0 || now < activity + 30_000 || now - activity > 86_400_000 {
                continue;
            }
            if !due && checked.get(key) == Some(&stamp) {
                continue;
            }
            if reads >= 4 {
                break;
            }
            reads += 1;
            let mut settings = settings.clone();
            settings["autoCompactEnvironment"] = json!(env);
            let result = (|| -> Result<Value> {
                let e = environments()?
                    .into_iter()
                    .find(|e| e.id == env)
                    .ok_or("unknown_environment")?;
                let envelope = bounded(&client, &e, id)?;
                let mut plan = policy::candidate(
                    &envelope["projection"],
                    &settings,
                    chrono::Utc::now().timestamp_millis(),
                );
                if policy::same_episode(&old, &plan) && policy::consumed(&old) {
                    return Ok(old.clone());
                }
                if matches!(text(&plan, "status"), "scheduled" | "due")
                    && !supported(&e, text(&plan, "provider"), &mut catalogs)?
                {
                    plan["status"] = json!("unsupported");
                }
                if plan["status"] == "due" {
                    // Re-read immediately before claiming. Native state may have advanced while reading the catalog.
                    let fresh = bounded(&client, &e, id)?;
                    let checked_plan = policy::candidate(
                        &fresh["projection"],
                        &settings,
                        chrono::Utc::now().timestamp_millis(),
                    );
                    if !policy::same_episode(&plan, &checked_plan)
                        || checked_plan["status"] != "due"
                        || plan["reportAt"] != checked_plan["reportAt"]
                    {
                        return Ok(checked_plan);
                    }
                    let mut current = crate::ui_state::load();
                    current["autoCompactEnvironment"] = json!(env);
                    if !policy::enabled(&current)
                        || current["autoCompactExcludedThreads"]
                            .as_array()
                            .is_some_and(|a| a.contains(&json!(key)))
                    {
                        return Ok(json!({"status":"disabled"}));
                    }
                    let current_plan = policy::candidate(
                        &fresh["projection"],
                        &current,
                        chrono::Utc::now().timestamp_millis(),
                    );
                    if current_plan["status"] != "due" {
                        return Ok(current_plan);
                    }
                    plan = current_plan;
                    let request = uuid::Uuid::new_v4().to_string();
                    plan["status"] = json!("claimed");
                    plan["requestId"] = json!(request);
                    ledger[key] = plan.clone();
                    save_json(&file, &ledger)?;
                    emit(key, &plan);
                    // A claim is durable before dispatch. Unknown RPC outcomes are never retried automatically.
                    let command = policy::command(id, &request, &plan["modelSelection"]);
                    match write::rpc(&e, "orchestration.dispatchCommand", command.clone()) {
                        Ok(_) => {
                            plan["status"] = json!("accepted");
                            // T3 queues start_immediately if another client won the race. Never leave
                            // our maintenance run queued to compact later against a cold cache.
                            match bounded(&client, &e, id) {
                                Ok(after) if after["projection"]["runs"].is_array() => {
                                    if let Some(run) = after["projection"]["runs"]
                                        .as_array()
                                        .into_iter()
                                        .flatten()
                                        .find(|r| {
                                            r["userMessageId"] == command["messageId"]
                                                && r["status"] == "queued"
                                        })
                                    {
                                        let cancel = json!({"type":"queued-run.cancel","commandId":format!("omarchy-compact-cancel-{request}"),"threadId":id,"runId":run["id"]});
                                        if write::rpc(&e, "orchestration.dispatchCommand", cancel)
                                            .is_ok()
                                        {
                                            plan["status"] = json!("cancelled")
                                        } else {
                                            plan["status"] = json!("uncertain");
                                            plan["error"] =
                                                json!("compaction_queue_outcome_unknown");
                                        }
                                    }
                                }
                                _ => {
                                    plan["status"] = json!("uncertain");
                                    plan["error"] = json!("compaction_queue_outcome_unknown");
                                }
                            }
                        }
                        Err(error) => {
                            plan["status"] = json!(if error == "rpc_outcome_unknown" {
                                "uncertain"
                            } else {
                                "failed"
                            });
                            plan["error"] = json!(error);
                        }
                    }
                    plan["attemptedAt"] = json!(chrono::Utc::now().timestamp_millis());
                    let _ = refresh.send(json!({"op":"refresh"}));
                }
                Ok(plan)
            })();
            match result {
                Ok(mut plan) => {
                    checked.insert(key.into(), stamp);
                    if old["status"] == "scheduled" && !plan["episode"].is_string() {
                        let mut held = old.clone();
                        held["status"] = plan["status"].clone();
                        plan = held;
                    }
                    if plan["episode"].is_string() && ledger[key] != plan {
                        ledger[key] = plan.clone();
                        if ledger.as_object().unwrap().len() > 1024 {
                            return Err("compaction_ledger_full".into());
                        }
                        save_json(&file, &ledger)?;
                    }
                    if statuses.get(key) != Some(&plan) {
                        emit(key, &plan);
                        statuses.insert(key.into(), plan);
                    }
                }
                Err(error) => {
                    emit(key, &json!({"status":"error","error":error}));
                }
            }
        }
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(at) = ledger
            .as_object()
            .unwrap()
            .values()
            .filter(|v| v["status"] == "scheduled")
            .filter_map(|v| v["dueAt"].as_i64())
            .filter(|d| *d > now)
            .min()
        {
            let _ = refresh.send(json!({"op":"compaction-deadline","at":at}));
        }
    }
    Ok(())
}
