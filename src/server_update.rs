//! Keeps connected T3 servers on the newest release of their channel. One
//! worker, driven by the metadata poll like the compactor; the version rules
//! and the native `t3 update` run live in the library's server_updates.
use omarchy_t3_client::{server_updates as updates, *};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::{mpsc, Mutex},
};
const CHECK_MS: i64 = 60 * 60 * 1000;
static STATE: Mutex<Option<Value>> = Mutex::new(None);
static RUNNING: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static RECHECK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone)]
pub struct Control(mpsc::SyncSender<Value>);
impl Control {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || run(rx));
        Self(tx)
    }
    pub fn observe(&self, snapshot: &Value) {
        let _ = self.0.try_send(snapshot.clone());
    }
}
fn ledger_path() -> std::path::PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("server-updates.json")
}
/// "auto" (when idle), "notify" or "off". Integration fixtures never update by default.
fn mode() -> String {
    let settings = crate::ui_state::load();
    let fallback = if std::env::var_os("OMARCHY_T3_FIXTURE_ENVIRONMENTS").is_some() { "off" } else { "auto" };
    settings["t3ServerUpdates"].as_str().unwrap_or(fallback).to_string()
}
fn publish(change: impl FnOnce(&mut Value)) {
    let Ok(mut state) = STATE.lock() else { return };
    let mut next = state.clone().unwrap_or_else(|| json!({"mode":"off","environments":{}}));
    change(&mut next);
    if state.as_ref() != Some(&next) {
        crate::emit("server-updates", next.clone());
        *state = Some(next);
    }
}
/// The user asked for this environment's update now, whatever the mode.
pub fn update_now(environment: &str) -> Result<Value> {
    let target = STATE.lock().ok().and_then(|s| s.as_ref()?["environments"][environment]["latest"].as_str().map(String::from)).ok_or("t3_update_not_available")?;
    perform(environment, &target)?;
    Ok(STATE.lock().ok().and_then(|s| s.clone()).unwrap_or(Value::Null))
}
fn perform(environment: &str, target: &str) -> Result<()> {
    {
        let mut running = RUNNING.lock().map_err(|_| "local_error")?;
        if !running.get_or_insert_with(HashSet::new).insert(environment.to_string()) {
            return Err("t3_update_running".into());
        }
    }
    publish(|s| {
        s["environments"][environment]["state"] = json!("running");
        s["environments"][environment]["error"] = Value::Null;
    });
    let now = chrono::Utc::now().timestamp_millis();
    let result = updates::run(environment, target);
    let path = ledger_path();
    let mut ledger = read_json(&path).ok().filter(Value::is_object).unwrap_or_else(|| json!({}));
    ledger[environment] = json!({"target":target,"at":now,"status":if result.is_ok(){"updated"}else{"failed"},"error":result.as_ref().err()});
    let _ = save_json(&path, &ledger);
    publish(|s| {
        let e = &mut s["environments"][environment];
        e["state"] = json!(if result.is_ok() { "updated" } else { "failed" });
        e["error"] = json!(result.as_ref().err());
        e["attemptedAt"] = json!(now);
    });
    if let Ok(mut running) = RUNNING.lock() {
        running.get_or_insert_with(HashSet::new).remove(environment);
    }
    RECHECK.store(true, std::sync::atomic::Ordering::SeqCst);
    result.map(|_| ())
}
fn run(rx: mpsc::Receiver<Value>) {
    let mut checked_at = 0i64;
    let mut known: Vec<String> = vec![];
    for snapshot in rx {
        let mode = mode();
        if mode == "off" {
            publish(|s| *s = json!({"mode":"off","environments":{}}));
            checked_at = 0;
            continue;
        }
        let now = chrono::Utc::now().timestamp_millis();
        let mut ready: Vec<String> = snapshot["environments"].as_array().into_iter().flatten().filter(|e| e["state"] == "ready").map(|e| text(e, "id").to_string()).collect();
        ready.sort();
        let recheck = RECHECK.swap(false, std::sync::atomic::Ordering::SeqCst);
        if recheck || ready != known || now - checked_at >= CHECK_MS {
            checked_at = now;
            known = ready.clone();
            let mut latest: HashMap<&str, Result<String>> = HashMap::new();
            let mut rows = serde_json::Map::new();
            for e in environments().unwrap_or_default().into_iter().filter(|e| ready.contains(&e.id)) {
                let Some(current) = write::rpc(&e, "server.getConfig", json!({})).ok().and_then(|c| c["environment"]["serverVersion"].as_str().map(String::from)) else { continue };
                let channel = updates::channel(&current);
                let newest = latest.entry(channel).or_insert_with(|| updates::latest(channel));
                let mut row = updates::advisory(&current, newest.as_ref().ok().map(String::as_str));
                row["name"] = json!(e.name);
                match updates::route(&e.id) {
                    Ok(_) => row["canUpdate"] = json!(true),
                    Err(reason) => {
                        row["canUpdate"] = json!(false);
                        row["reason"] = json!(reason);
                    }
                }
                if let Err(error) = newest {
                    row["error"] = json!(error);
                    // An unreachable release index is asked again in ten minutes, not an hour.
                    checked_at = now - CHECK_MS + 10 * 60 * 1000;
                }
                rows.insert(e.id.clone(), row);
            }
            publish(|s| {
                let previous = s["environments"].clone();
                for (id, row) in rows.iter_mut() {
                    // A finished or failed attempt stays visible until the next one.
                    for key in ["state", "attemptedAt"] {
                        if !previous[id][key].is_null() {
                            row[key] = previous[id][key].clone();
                        }
                    }
                    if row["error"].is_null() && previous[id]["state"] == "failed" {
                        row["error"] = previous[id]["error"].clone();
                    }
                }
                *s = json!({"mode":mode,"checkedAt":now,"environments":rows});
            });
        } else {
            publish(|s| s["mode"] = json!(mode));
        }
        if mode != "auto" {
            continue;
        }
        let ledger = read_json(&ledger_path()).unwrap_or(Value::Null);
        let rows = STATE.lock().ok().and_then(|s| s.as_ref().map(|s| s["environments"].clone())).unwrap_or(Value::Null);
        let next = rows.as_object().into_iter().flatten().find(|(id, row)| row["state"] != "running" && updates::due(&mode, row, &ledger[id.as_str()], updates::quiet(&snapshot, id, now), now));
        if let Some((id, row)) = next {
            let _ = perform(id, text(row, "latest"));
        }
    }
}
