//! One desktop worker; uses existing metadata polling and native session detach.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    sync::mpsc,
};
#[derive(Clone)]
pub struct Control(mpsc::SyncSender<Value>);
impl Control {
    pub fn new(refresh: mpsc::Sender<Value>) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Value>(1);
        std::thread::spawn(move || {
            let dir = xdg("XDG_STATE_HOME", ".local/state").join("idle-release");
            if fs::create_dir_all(&dir).is_err() {
                return;
            }
            let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
            let Ok(lock) = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(dir.join("worker.lock"))
            else {
                return;
            };
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return;
            }
            let ledger_path = dir.join("claims.json");
            let mut ledger = read_json(&ledger_path).unwrap_or(json!({}));
            if !ledger.is_object() {
                return;
            }
            let Ok(client) = Client::shared() else {
                return;
            };
            let mut attempted: HashSet<String> =
                ledger.as_object().unwrap().keys().cloned().collect();
            let mut cursor = 0;
            let mut audit_cache = crate::session_audit_cache::Cache::default();
            let mut settings_before = Value::Null;
            for snapshot in rx {
                let _memory_sweep = crate::memory_pressure::Sweep;
                let settings = crate::ui_state::load();
                if settings != settings_before {
                    audit_cache = crate::session_audit_cache::Cache::default();
                    settings_before = settings.clone();
                }
                let enabled = settings["idleReleaseEnabled"] != false
                    && (std::env::var_os("OMARCHY_T3_FIXTURE_ENVIRONMENTS").is_none()
                        || settings["idleReleaseEnabled"] == true);
                crate::emit(
                    "idle-release",
                    json!({"key":"@engine","status":if enabled{"enabled"}else{"disabled"}}),
                );
                if !enabled {
                    continue;
                }
                let mut reads = 0;
                let threads = snapshot["threads"].as_array().cloned().unwrap_or_default();
                audit_cache.retain(&threads);
                let start = cursor;
                cursor = if threads.is_empty() {
                    0
                } else {
                    (cursor + 2) % threads.len()
                };
                for t in threads.iter().cycle().skip(start).take(threads.len()) {
                    if t["working"] == true
                        || t["backgroundWaiting"] == true
                        || t["canSnooze"] == false
                    {
                        continue;
                    }
                    let env = text(t, "environmentId");
                    let id = text(t, "id");
                    let key = text(t, "key");
                    let now = chrono::Utc::now().timestamp_millis();
                    let at = t["compactActivityAt"].as_i64().unwrap_or(0);
                    let provider = text(&t["modelSelection"], "instanceId");
                    let Some(min) = session_memory::minutes(&settings, env, provider) else {
                        continue;
                    };
                    if at <= 0 || now < at.saturating_add(min as i64 * 60_000) {
                        continue;
                    }
                    if !audit_cache.due(t, std::time::Instant::now()) {
                        continue;
                    }
                    if reads >= 2 {
                        break;
                    }
                    reads += 1;
                    let result = (|| -> Result<Value> {
                        let e = environments()?
                            .into_iter()
                            .find(|e| e.id == env)
                            .ok_or("unknown_environment")?;
                        let plan = {
                            let first = client.bounded_thread(&e, id)?;
                            session_memory::candidate(&first["projection"], &settings, env, now)
                        };
                        if plan["status"] != "due" {
                            return Ok(plan);
                        }
                        if attempted.contains(text(&plan, "episode")) {
                            let mut previous = plan.clone();
                            previous["status"] = ledger[text(&plan, "episode")]["status"]
                                .as_str()
                                .filter(|s| *s != "claimed")
                                .map(|s| json!(s))
                                .unwrap_or(json!("uncertain"));
                            return Ok(previous);
                        }
                        // Revalidate the complete thread, queue, requests and background
                        // roster immediately before releasing the exact native session.
                        let checked = {
                            let fresh = client.bounded_thread(&e, id)?;
                            let current = crate::ui_state::load();
                            session_memory::candidate(
                                &fresh["projection"],
                                &current,
                                env,
                                chrono::Utc::now().timestamp_millis(),
                            )
                        };
                        if checked["status"] != "due" || checked["episode"] != plan["episode"] {
                            return Ok(checked);
                        }
                        // Claim before dispatch. A lost reply is never replayed as a new
                        // shutdown against a conversation the user may have resumed.
                        ledger[text(&checked, "episode")] = json!({"status":"claimed","at":now});
                        save_json(&ledger_path, &ledger)?;
                        attempted.insert(text(&checked, "episode").to_string());
                        let mut out = checked.clone();
                        match write::dispatch_command(&e, &session_memory::command(&checked)) {
                            Ok(_) => {
                                out["status"] = json!("released");
                                let _ = refresh.send(json!({"op":"refresh"}));
                            }
                            Err(error) => {
                                out["status"] = json!("failed");
                                out["error"] = json!(error);
                            }
                        }
                        ledger[text(&checked, "episode")] =
                            json!({"status":out["status"],"at":now});
                        save_json(&ledger_path, &ledger)?;
                        Ok(out)
                    })();
                    if result.is_ok() {
                        audit_cache.checked(t, std::time::Instant::now());
                    }
                    let mut status =
                        result.unwrap_or_else(|error| json!({"status":"failed","error":error}));
                    status["key"] = json!(key);
                    crate::emit("idle-release", status);
                }
            }
        });
        Self(tx)
    }
    pub fn observe(&self, snapshot: &Value) {
        let _ = self.0.try_send(snapshot.clone());
    }
}
