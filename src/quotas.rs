//! Dynamic native subscription streams plus compatible local credential sources.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Default)]
struct State {
    environments: HashMap<String, Value>,
    local: Vec<Value>,
}
fn publish(state: &State) {
    let mut environments = state.environments.values().cloned().collect::<Vec<_>>();
    environments.sort_by(|a, b| text(a, "id").cmp(text(b, "id")));
    crate::emit(
        "quotas",
        json!({"environments":environments,"localSources":state.local,"updatedAt":chrono::Utc::now().timestamp()}),
    );
}
struct Watch {
    signature: String,
    cancel: Arc<AtomicBool>,
    refresh: Arc<AtomicBool>,
}
#[derive(Default)]
pub struct Control {
    state: Arc<Mutex<State>>,
    watches: HashMap<String, Watch>,
}
impl Control {
    pub fn sync(&mut self, environments: Vec<Environment>) {
        let mut state = self.state.lock().unwrap();
        let ids = environments
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>();
        let before = self.watches.len();
        self.watches.retain(|id, w| {
            if ids.contains(&id.as_str()) {
                true
            } else {
                w.cancel.store(true, Ordering::Relaxed);
                state.environments.remove(id);
                false
            }
        });
        let mut changed = before != self.watches.len();
        for e in environments {
            let signature = format!("{}\n{}\n{:?}", e.name, e.base_url, e.connection_error);
            if self
                .watches
                .get(&e.id)
                .is_some_and(|w| w.signature == signature)
            {
                continue;
            }
            if let Some(old) = self.watches.remove(&e.id) {
                old.cancel.store(true, Ordering::Relaxed);
            }
            let cancel = Arc::new(AtomicBool::new(false));
            let refresh = Arc::new(AtomicBool::new(false));
            self.watches.insert(
                e.id.clone(),
                Watch {
                    signature,
                    cancel: cancel.clone(),
                    refresh: refresh.clone(),
                },
            );
            state.environments.insert(e.id.clone(),json!({"id":e.id,"name":e.name,"connected":false,"providers":[],"usageLimitSources":[]}));
            let shared = self.state.clone();
            std::thread::spawn(move || watch(e, shared, cancel, refresh));
            changed = true;
        }
        if changed {
            publish(&state);
        }
    }
    pub fn local(&self, sources: Vec<Value>, complete: bool) {
        let mut state = self.state.lock().unwrap();
        let previous = state
            .local
            .iter()
            .flat_map(|s| s["accounts"].as_array().into_iter().flatten())
            .map(|a| (text(a, "id").to_owned(), a["usageLimits"].clone()))
            .collect::<HashMap<_, _>>();
        let mut sources = quota_sources::sources(&json!(sources));
        if !complete {
            for source in &mut sources {
                if let Some(accounts) = source["accounts"].as_array_mut() {
                    let known = accounts
                        .iter()
                        .map(|a| text(a, "id").to_owned())
                        .collect::<Vec<_>>();
                    for old in state
                        .local
                        .iter()
                        .flat_map(|s| s["accounts"].as_array().into_iter().flatten())
                    {
                        if !known.iter().any(|id| id == text(old, "id")) {
                            accounts.push(old.clone());
                        }
                    }
                }
            }
        }
        for source in &mut sources {
            let mut failed = false;
            if let Some(accounts) = source["accounts"].as_array_mut() {
                for account in accounts {
                    if account["usageLimits"]["unavailable"]["reason"] == "probeFailed" {
                        failed = true;
                        if let Some(old) = previous.get(text(account, "id")).filter(|old| {
                            old["unavailable"].is_null()
                                && old["windows"].as_array().is_some_and(|w| !w.is_empty())
                        }) {
                            account["usageLimits"] = old.clone();
                        }
                    }
                }
            }
            if failed {
                source["error"] =
                    json!("Could not refresh local accounts; readings may be outdated.");
            }
        }
        state.local = sources;
        publish(&state);
    }
    pub fn refresh(&self) {
        for w in self.watches.values() {
            w.refresh.store(true, Ordering::Relaxed);
        }
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        for w in self.watches.values() {
            w.cancel.store(true, Ordering::Relaxed);
        }
    }
}
fn watch(
    initial: Environment,
    shared: Arc<Mutex<State>>,
    cancel: Arc<AtomicBool>,
    refresh: Arc<AtomicBool>,
) {
    let mut retry = 1;
    let refresh_after = Arc::new(Mutex::new(Instant::now()));
    let refreshing = Arc::new(AtomicBool::new(false));
    while !cancel.load(Ordering::Relaxed) {
        // Resolve routes/credentials again after reconnect, never persist raw config.
        let e = environments()
            .ok()
            .and_then(|all| all.into_iter().find(|e| e.id == initial.id))
            .unwrap_or_else(|| initial.clone());
        let result = quota_sources::subscribe(
            &e,
            || cancel.load(Ordering::Relaxed),
            |event| {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                if let Some(event) = event {
                    let mut state = shared.lock().unwrap();
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    if let Some(view) = state.environments.get_mut(&e.id) {
                        let id = view["id"].clone();
                        let name = view["name"].clone();
                        if quota_sources::apply(view, &event) {
                            view["id"] = id;
                            view["name"] = name;
                            view["connected"] = json!(true);
                            view.as_object_mut().unwrap().remove("error");
                            publish(&state);
                            retry = 1;
                        }
                    }
                }
                // The same five-minute budget as T3, also after failures. Native
                // refresh has no refreshModels flag and cannot request a model session.
                if (refresh.swap(false, Ordering::Relaxed)
                    || Instant::now() >= *refresh_after.lock().unwrap())
                    && !refreshing.swap(true, Ordering::Relaxed)
                {
                    *refresh_after.lock().unwrap() = Instant::now() + Duration::from_secs(300);
                    let refresh_after = refresh_after.clone();
                    let e = e.clone();
                    let cancel = cancel.clone();
                    let shared = shared.clone();
                    let refreshing = refreshing.clone();
                    std::thread::spawn(move || {
                        let result = e.connection(false).and_then(|c| {
                            quota_sources::authorize(&c)?;
                            c.rpc("server.refreshProviders", json!({}))
                        });
                        let mut state = shared.lock().unwrap();
                        if !cancel.load(Ordering::Relaxed) {
                            if let Some(view) = state.environments.get_mut(&e.id) {
                                view["refreshFailed"] = json!(result.is_err());
                            }
                            publish(&state);
                        }
                        *refresh_after.lock().unwrap() = Instant::now() + Duration::from_secs(300);
                        refreshing.store(false, Ordering::Relaxed);
                    });
                }
                Ok(())
            },
        );
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        {
            let mut state = shared.lock().unwrap();
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            if let Some(view) = state.environments.get_mut(&e.id) {
                view["connected"] = json!(false);
                let denied = result
                    .as_ref()
                    .err()
                    .is_some_and(|e| matches!(e.as_str(), "permission_denied" | "unauthorized"));
                view["error"] = json!(if denied {
                    "permission_denied"
                } else {
                    "disconnected"
                });
                if denied {
                    view["providers"] = json!([]);
                    view["usageLimitSources"] = json!([]);
                }
                publish(&state);
            }
        }
        for _ in 0..retry * 10 {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        retry = (retry * 2).min(30);
    }
}
/// Bounded explicit CLI read, including the opt-in hub replay (not getConfig alone).
pub fn collect(_client: &Client) -> Value {
    let environments = environments().unwrap_or_default();
    let views = std::thread::scope(|scope| {
        let jobs=environments.into_iter().map(|e|scope.spawn(move|| {
            let start=Instant::now();let mut state=json!({"id":e.id,"name":e.name,"providers":[],"usageLimitSources":[],"connected":false});
            let result=quota_sources::subscribe(&e,||start.elapsed()>Duration::from_secs(2),|event| {
                if let Some(event)=event {let id=state["id"].clone();let name=state["name"].clone();quota_sources::apply(&mut state,&event);state["id"]=id;state["name"]=name;}
                Ok(())
            });
            if result.is_err(){state["error"]=json!("disconnected");state["connected"]=json!(false);}
            state
        })).collect::<Vec<_>>();
        jobs.into_iter()
            .filter_map(|job| job.join().ok())
            .collect::<Vec<_>>()
    });
    json!({"environments":views,"localSources":crate::local_quotas::collect(&Client::new().unwrap()),"updatedAt":chrono::Utc::now().timestamp()})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(id: &str, failed: bool) -> Value {
        let mut usage = json!({"checkedAt":"2026-10-18T12:00:00Z","windows":[{"id":"weekly","label":"Weekly","kind":"weekly","usedPercent":25}]});
        if failed {
            usage = json!({"checkedAt":"2026-10-18T12:05:00Z","windows":[],"unavailable":{"reason":"probeFailed"}});
        }
        json!({"id":"source","label":"Local","accounts":[{"id":id,"driver":"codex","usageLimits":usage}]})
    }
    #[test]
    fn failures_keep_last_good_timestamp_and_partial_refreshes_keep_unread_accounts() {
        let control = Control::default();
        control.local(vec![source("one", false)], true);
        control.local(vec![source("two", false)], false);
        assert_eq!(
            control.state.lock().unwrap().local[0]["accounts"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        control.local(vec![source("one", true)], true);
        let state = control.state.lock().unwrap();
        assert_eq!(state.local[0]["accounts"].as_array().unwrap().len(), 1);
        assert_eq!(
            state.local[0]["accounts"][0]["usageLimits"]["checkedAt"],
            "2026-10-18T12:00:00Z"
        );
        assert!(state.local[0].get("error").is_some());
        drop(state);
        control.local(vec![], true);
        assert!(control.state.lock().unwrap().local.is_empty());
    }
}
