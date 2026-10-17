//! Authorized native-session coordinator. Website pixels remain in isolated
//! Qt guests; T3 credentials, permission checks and RPCs stay in this process.
use crate::{browser_guest, json, preview_transport, text, Environment, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
type Resolver = Arc<dyn Fn(&str) -> Result<Environment> + Send + Sync>;
type Output = Arc<dyn Fn(Value) + Send + Sync>;
type Key = (String, String, String);
struct Grant {
    scope: Value,
    process: String,
    request: String,
    allowed: AtomicBool,
    validated: Mutex<Instant>,
    zoom: Mutex<f64>,
}
type Grants = Arc<Mutex<HashMap<Key, Arc<Grant>>>>;
fn key(e: &str, t: &str, tab: &str) -> Key {
    (e.into(), t.into(), tab.into())
}
fn safe_error(error: &str) -> &str {
    match error {
        "permission_denied"
        | "preview_scope_changed"
        | "preview_server_restarted"
        | "preview_tab_closed"
        | "server_browser_surface_required"
        | "browser_runtime_unavailable"
        | "browser_profile_busy"
        | "browser_guest_limit"
        | "invalid_browser_input"
        | "browser_view_changed" => error,
        _ => "browser_unavailable",
    }
}
fn error(output: &Output, request: &str, value: &str) {
    output(json!({"kind":"error","requestId":request,"error":safe_error(value)}));
}
#[derive(Clone)]
struct Reporter {
    pending: Arc<Mutex<HashMap<Key, Option<(Arc<Grant>, Value)>>>>,
    resolve: Resolver,
    output: Output,
}
impl Reporter {
    fn offer(&self, k: Key, grant: Arc<Grant>, status: Value) {
        let mut pending = self.pending.lock().unwrap();
        if let Some(value) = pending.get_mut(&k) {
            *value = Some((grant, status));
            return;
        }
        if pending.len() >= 128 {
            return;
        }
        pending.insert(k.clone(), Some((grant, status)));
        drop(pending);
        let this = self.clone();
        thread::spawn(move || loop {
            let next = {
                let mut all = this.pending.lock().unwrap();
                let value = all.get_mut(&k).and_then(Option::take);
                if value.is_none() {
                    all.remove(&k);
                }
                value
            };
            let Some((grant, status)) = next else { return };
            if !grant.allowed.load(Ordering::SeqCst) {
                continue;
            }
            let result = (this.resolve)(&k.0)
                .and_then(|e| preview_transport::report_status(&e, &grant.scope, &status));
            if let Err(e) = result {
                grant.allowed.store(false, Ordering::SeqCst);
                error(&this.output, &grant.request, &e);
            }
        });
    }
}
enum Message {
    Show(Value, u64),
    Input(Value, u64),
    Control(Value, u64),
    Hide,
    Observe(Value),
    Action(Value, Result<Value>),
    Shutdown(mpsc::Sender<()>),
    CleanupBegin {
        id: String,
        profile: String,
        store: PathBuf,
        scopes: Vec<crate::browser_cleanup::Scope>,
        done: mpsc::Sender<Result<()>>,
    },
    CleanupClear {
        id: String,
        environment: String,
        done: mpsc::Sender<Result<()>>,
    },
    CleanupCookie {
        id: String,
        environment: String,
        cookie: crate::browser_cookies::ImportedCookie,
        done: mpsc::Sender<Result<bool>>,
    },
    CleanupFlush {
        id: String,
        environment: String,
        done: mpsc::Sender<Result<()>>,
    },
    CleanupEnd(String),
}
struct Cleanup {
    profile: String,
    scopes: HashMap<String, String>,
    locks: HashMap<String, crate::browser_runtime::ProfileLock>,
    cleared: Vec<String>,
    importing: std::collections::HashSet<String>,
}
struct CleanupLease {
    sender: mpsc::SyncSender<Message>,
    id: String,
}
impl crate::browser_cleanup::Lease for CleanupLease {
    fn write_cookie(
        &mut self,
        environment: &str,
        cookie: crate::browser_cookies::ImportedCookie,
    ) -> Result<bool> {
        let (done, wait) = mpsc::channel();
        self.sender
            .send(Message::CleanupCookie {
                id: self.id.clone(),
                environment: environment.into(),
                cookie,
                done,
            })
            .map_err(|_| "browser_cookie_import_unknown")?;
        wait.recv_timeout(Duration::from_secs(15))
            .map_err(|_| "browser_cookie_import_unknown")?
    }
    fn flush_import(&mut self, environment: &str) -> Result<()> {
        let (done, wait) = mpsc::channel();
        self.sender
            .send(Message::CleanupFlush {
                id: self.id.clone(),
                environment: environment.into(),
                done,
            })
            .map_err(|_| "browser_cookie_import_unknown")?;
        wait.recv_timeout(Duration::from_secs(15))
            .map_err(|_| "browser_cookie_import_unknown")?
    }
    fn clear(&mut self, environment: &str) -> Result<()> {
        let (done, wait) = mpsc::channel();
        self.sender
            .send(Message::CleanupClear {
                id: self.id.clone(),
                environment: environment.into(),
                done,
            })
            .map_err(|_| "browser_profile_outcome_unknown")?;
        wait.recv_timeout(Duration::from_secs(15))
            .map_err(|_| "browser_profile_outcome_unknown")?
    }
}
impl Drop for CleanupLease {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::CleanupEnd(self.id.clone()));
    }
}
#[derive(Clone)]
pub struct Manager {
    sender: mpsc::SyncSender<Message>,
    generation: Arc<AtomicU64>,
    desired: Arc<Mutex<Option<Value>>>,
    output: Output,
    ready: Arc<AtomicBool>,
    import_assets: [PathBuf; 2],
}
fn store_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}
impl crate::browser_cleanup::Runtime for Manager {
    fn can_import(&self) -> bool {
        self.available() && self.import_assets.iter().all(|p| p.is_file())
    }
    fn available(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
    fn begin(
        &self,
        profile: &str,
        client_store: &Path,
        scopes: &[crate::browser_cleanup::Scope],
    ) -> Result<Box<dyn crate::browser_cleanup::Lease>> {
        if !self.available() {
            return Err("browser_profile_runtime_unavailable".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let lease = CleanupLease {
            sender: self.sender.clone(),
            id: id.clone(),
        };
        let (done, wait) = mpsc::channel();
        self.sender
            .send(Message::CleanupBegin {
                id,
                profile: profile.into(),
                store: store_key(client_store),
                scopes: scopes.to_vec(),
                done,
            })
            .map_err(|_| "browser_profile_runtime_unavailable")?;
        wait.recv_timeout(Duration::from_secs(15))
            .map_err(|_| "browser_profile_outcome_unknown")??;
        Ok(Box::new(lease))
    }
}
impl Manager {
    pub fn start(
        config: browser_guest::Config,
        resolve: impl Fn(&str) -> Result<Environment> + Send + Sync + 'static,
        defaults: impl Fn() -> Result<Value> + Send + 'static,
        output: impl Fn(Value) + Send + Sync + 'static,
    ) -> Self {
        let resolve: Resolver = Arc::new(resolve);
        let output: Output = Arc::new(output);
        let grants: Grants = Arc::new(Mutex::new(HashMap::new()));
        let reporter = Reporter {
            pending: Arc::new(Mutex::new(HashMap::new())),
            resolve: resolve.clone(),
            output: output.clone(),
        };
        let import_assets = config.import_assets();
        let storage = config.storage_root();
        let settings = config.client_settings_path();
        let (sender, receiver) = mpsc::sync_channel(256);
        let ready = Arc::new(AtomicBool::new(true));
        let running = ready.clone();
        let generation = Arc::new(AtomicU64::new(0));
        let desired = Arc::new(Mutex::new(None::<Value>));
        let (current, wanted) = (generation.clone(), desired.clone());
        let (callback_grants, callback_output, callback_wanted) =
            (grants.clone(), output.clone(), desired.clone());
        let mut pool = browser_guest::Pool::new(config, move |data| {
            if data["kind"] == "status" {
                let s = &data["snapshot"];
                let k = key(
                    text(&data, "environment"),
                    text(s, "threadId"),
                    text(s, "tabId"),
                );
                let grant = callback_grants.lock().unwrap().get(&k).cloned();
                if let Some(grant) = grant.filter(|g| {
                    g.allowed.load(Ordering::SeqCst)
                        && g.process == text(&data, "processId")
                        && g.scope["originKey"] == data["originKey"]
                }) {
                    reporter.offer(k, grant, s.clone());
                }
            } else {
                let active = callback_wanted.lock().unwrap().clone();
                if active.as_ref().is_some_and(|a| {
                    a["requestId"] == data["requestId"]
                        && a["environment"] == data["environment"]
                        && a["thread"] == data["thread"]
                        && a["tabId"] == data["tabId"]
                }) {
                    callback_output(data);
                }
            }
        });
        let worker_output = output.clone();
        thread::spawn(move || {
            struct Ready(Arc<AtomicBool>);
            impl Drop for Ready {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::SeqCst);
                }
            }
            let _ready = Ready(running);
            let mut cleanups = HashMap::<String, Cleanup>::new();
            let mut active = None::<Value>;
            let mut epochs = HashMap::<String, (String, String)>::new();
            let mut revisions = HashMap::<(String, String), u64>::new();
            for message in receiver {
                if wanted.lock().unwrap().is_none() {
                    pool.hide();
                    active = None;
                }
                match message {
                    Message::Hide => {
                        pool.hide();
                        active = None;
                    }
                    Message::Shutdown(done) => {
                        for grant in grants.lock().unwrap().values() {
                            grant.allowed.store(false, Ordering::SeqCst);
                        }
                        pool.close_all();
                        let _ = done.send(());
                        break;
                    }
                    Message::CleanupBegin {
                        id,
                        profile,
                        store,
                        scopes,
                        done,
                    } => {
                        let result = (|| -> Result<()> {
                            crate::browser_runtime::partition("local", &profile)?;
                            if store != store_key(&settings) {
                                return Err("browser_profile_scope_changed".into());
                            }
                            if scopes.is_empty() || scopes.len() > 64 || cleanups.len() >= 16 {
                                return Err("browser_profiles_busy".into());
                            }
                            let mut selected = HashMap::new();
                            for scope in scopes {
                                let e = resolve(&scope.environment)?;
                                let origin = format!("{:x}", Sha256::digest(e.base_url.as_bytes()));
                                if origin != scope.origin_key
                                    || e.connection_error.is_some()
                                    || selected.insert(e.id.clone(), origin).is_some()
                                {
                                    return Err("browser_profile_scope_changed".into());
                                }
                            }
                            if cleanups.values().any(|c| {
                                c.profile == profile
                                    && c.scopes.keys().any(|e| selected.contains_key(e))
                            }) {
                                return Err("browser_profiles_busy".into());
                            }
                            let targets = grants
                                .lock()
                                .unwrap()
                                .iter()
                                .filter(|((e, _, _), g)| {
                                    selected.contains_key(e) && g.scope["profileId"] == profile
                                })
                                .map(|(k, _)| k.clone())
                                .collect::<Vec<_>>();
                            if active.as_ref().is_some_and(|a| {
                                targets.contains(&key(
                                    text(a, "environment"),
                                    text(a, "thread"),
                                    text(a, "tabId"),
                                ))
                            }) {
                                pool.hide();
                                active = None;
                            }
                            for k in targets {
                                if let Some(grant) = grants.lock().unwrap().get(&k) {
                                    // Keep the latched profile/zoom for a fresh authorized reattachment.
                                    grant.allowed.store(false, Ordering::SeqCst);
                                }
                            }
                            let environments = selected.keys().cloned().collect::<Vec<_>>();
                            cleanups.insert(
                                id.clone(),
                                Cleanup {
                                    profile: profile.clone(),
                                    scopes: selected,
                                    locks: HashMap::new(),
                                    cleared: vec![],
                                    importing: Default::default(),
                                },
                            );
                            worker_output(
                                json!({"kind":"cleanup-started","cleanupId":id,"profileId":profile,"environments":environments}),
                            );
                            Ok(())
                        })();
                        let _ = done.send(result);
                    }
                    Message::CleanupClear {
                        id,
                        environment,
                        done,
                    } => {
                        let result = (|| -> Result<()> {
                            let cleanup = cleanups
                                .get_mut(&id)
                                .ok_or("browser_profile_scope_changed")?;
                            let expected = cleanup
                                .scopes
                                .get(&environment)
                                .ok_or("browser_profile_scope_changed")?;
                            if cleanup.cleared.contains(&environment) {
                                return Err("browser_profile_view_expired".into());
                            }
                            let e = resolve(&environment)?;
                            if format!("{:x}", Sha256::digest(e.base_url.as_bytes())) != *expected {
                                return Err("browser_profile_scope_changed".into());
                            }
                            pool.close_profile(&environment, &cleanup.profile)?;
                            if !cleanup.locks.contains_key(&environment) {
                                let lock = crate::browser_runtime::lock_profile(
                                    &storage,
                                    &environment,
                                    &cleanup.profile,
                                )?;
                                cleanup.locks.insert(environment.clone(), lock);
                            }
                            crate::browser_runtime::clear_owned_storage(
                                &storage,
                                &environment,
                                &cleanup.profile,
                                cleanup.locks.get(&environment).unwrap(),
                            )?;
                            cleanup.cleared.push(environment);
                            Ok(())
                        })();
                        let _ = done.send(result);
                    }
                    Message::CleanupCookie {
                        id,
                        environment,
                        cookie,
                        done,
                    } => {
                        let result = (|| -> Result<bool> {
                            let cleanup = cleanups
                                .get_mut(&id)
                                .ok_or("browser_profile_scope_changed")?;
                            let expected = cleanup
                                .scopes
                                .get(&environment)
                                .ok_or("browser_profile_scope_changed")?;
                            if cleanup.cleared.contains(&environment)
                                || cleanup.locks.contains_key(&environment)
                            {
                                return Err("browser_profile_scope_changed".into());
                            }
                            let e = resolve(&environment)?;
                            if format!("{:x}", Sha256::digest(e.base_url.as_bytes())) != *expected
                                || e.connection_error.is_some()
                            {
                                return Err("browser_profile_scope_changed".into());
                            }
                            // Remove all live pages from this partition before the first write.
                            if !cleanup.importing.contains(&environment) {
                                pool.close_profile(&environment, &cleanup.profile)?;
                                cleanup.importing.insert(environment.clone());
                            }
                            pool.import_cookie(&environment, &cleanup.profile, expected, &cookie)
                        })();
                        let _ = done.send(result);
                    }
                    Message::CleanupFlush {
                        id,
                        environment,
                        done,
                    } => {
                        let result = (|| -> Result<()> {
                            let cleanup = cleanups
                                .get_mut(&id)
                                .ok_or("browser_profile_scope_changed")?;
                            let expected = cleanup
                                .scopes
                                .get(&environment)
                                .ok_or("browser_profile_scope_changed")?;
                            let e = resolve(&environment)?;
                            if format!("{:x}", Sha256::digest(e.base_url.as_bytes())) != *expected
                                || e.connection_error.is_some()
                            {
                                return Err("browser_profile_scope_changed".into());
                            }
                            if !cleanup.importing.contains(&environment) {
                                return Ok(());
                            }
                            pool.close_profile(&environment, &cleanup.profile)?;
                            if !cleanup.locks.contains_key(&environment) {
                                cleanup.locks.insert(
                                    environment.clone(),
                                    crate::browser_runtime::lock_profile(
                                        &storage,
                                        &environment,
                                        &cleanup.profile,
                                    )?,
                                );
                            }
                            Ok(())
                        })();
                        let _ = done.send(result);
                    }
                    Message::CleanupEnd(id) => {
                        if let Some(cleanup) = cleanups.remove(&id) {
                            for environment in &cleanup.importing {
                                let _ = pool.close_profile(environment, &cleanup.profile);
                            }
                            let environments = cleanup.scopes.keys().cloned().collect::<Vec<_>>();
                            drop(cleanup.locks);
                            worker_output(
                                json!({"kind":"cleanup-ended","cleanupId":id,"profileId":cleanup.profile,"environments":environments,"cleared":cleanup.cleared}),
                            );
                        }
                    }
                    Message::Show(input, stamp) => {
                        if current.load(Ordering::SeqCst) != stamp {
                            continue;
                        }
                        pool.hide();
                        active = None;
                        let request = text(&input, "requestId");
                        let result = (|| -> Result<Value> {
                            let e = resolve(text(&input, "environment"))?;
                            let mut view = preview_transport::surface(&e, &input)?;
                            if current.load(Ordering::SeqCst) != stamp {
                                return Err("browser_view_changed".into());
                            }
                            let preferences = defaults()?;
                            let snapshot = &mut view["snapshot"];
                            let k = key(&e.id, text(&input, "thread"), text(&input, "tabId"));
                            let old = grants.lock().unwrap().get(&k).cloned();
                            if snapshot.get("profileId").is_none() {
                                let fallback = old
                                    .as_ref()
                                    .filter(|g| {
                                        g.scope["originKey"] == input["originKey"]
                                            && g.scope["serverEpoch"] == input["serverEpoch"]
                                    })
                                    .map(|g| g.scope["profileId"].clone())
                                    .unwrap_or_else(|| {
                                        json!(crate::browser_preferences::default_profile(
                                            &preferences
                                        ))
                                    });
                                snapshot["profileId"] = if fallback.is_string() {
                                    fallback
                                } else {
                                    json!("default")
                                };
                            }
                            let profile = text(snapshot, "profileId").to_owned();
                            if cleanups
                                .values()
                                .any(|c| c.profile == profile && c.scopes.contains_key(&e.id))
                            {
                                return Err("browser_profile_busy".into());
                            }
                            view["profileId"] = json!(profile);
                            let (w, h) = dimensions(&view["snapshot"], &input)?;
                            let zoom = old
                                .as_ref()
                                .filter(|g| {
                                    g.scope["originKey"] == input["originKey"]
                                        && g.scope["serverEpoch"] == input["serverEpoch"]
                                })
                                .map(|g| json!(*g.zoom.lock().unwrap()))
                                .unwrap_or_else(|| {
                                    preferences
                                        .get("browserDefaultZoomFactor")
                                        .cloned()
                                        .unwrap_or(json!(1))
                                });
                            view["zoomFactor"] = zoom.clone();
                            let identity = (
                                text(&view, "originKey").to_owned(),
                                text(&view, "serverEpoch").to_owned(),
                            );
                            if epochs.get(&e.id).is_some_and(|v| v != &identity) {
                                invalidate(&mut pool, &grants, &e.id);
                                revisions.retain(|(id, _), _| id != &e.id);
                            }
                            epochs.insert(e.id.clone(), identity);
                            revisions
                                .entry((e.id.clone(), text(&input, "thread").to_owned()))
                                .and_modify(|r| {
                                    *r = (*r).max(view["revision"].as_u64().unwrap_or(0))
                                })
                                .or_insert(view["revision"].as_u64().unwrap_or(0));
                            pool.show(
                                &e.id,
                                text(&view, "originKey"),
                                &view["snapshot"],
                                request,
                                w,
                                h,
                                &zoom,
                            )?;
                            let process = pool
                                .process_id(&e.id, &profile)
                                .ok_or("browser_runtime_unavailable")?;
                            let grant = Arc::new(Grant {
                                scope: view.clone(),
                                process,
                                request: request.into(),
                                allowed: AtomicBool::new(true),
                                validated: Mutex::new(Instant::now()),
                                zoom: Mutex::new(zoom.as_f64().unwrap_or(1.0)),
                            });
                            if let Some(old) = grants.lock().unwrap().insert(k, grant) {
                                old.allowed.store(false, Ordering::SeqCst);
                            }
                            if current.load(Ordering::SeqCst) != stamp {
                                pool.hide();
                                return Err("browser_view_changed".into());
                            }
                            active = Some(input.clone());
                            Ok(view)
                        })();
                        if current.load(Ordering::SeqCst) == stamp {
                            match result {
                                Ok(view) => worker_output(
                                    json!({"kind":"shown","requestId":request,"environment":input["environment"],"thread":input["thread"],"tabId":input["tabId"],"originKey":view["originKey"],"canOperate":view["canOperate"],"profileId":view["profileId"],"zoomFactor":view["zoomFactor"]}),
                                ),
                                Err(e) => error(&worker_output, request, &e),
                            }
                        }
                    }
                    Message::Input(input, stamp) | Message::Control(input, stamp) => {
                        let control = input["op"] == "browser-control";
                        if current.load(Ordering::SeqCst) != stamp {
                            continue;
                        }
                        let Some(view) = active
                            .as_ref()
                            .filter(|v| v["requestId"] == input["requestId"])
                        else {
                            continue;
                        };
                        let k = key(
                            text(view, "environment"),
                            text(view, "thread"),
                            text(view, "tabId"),
                        );
                        let grant = grants.lock().unwrap().get(&k).cloned();
                        let Some(grant) = grant.filter(|g| g.allowed.load(Ordering::SeqCst)) else {
                            continue;
                        };
                        // Recheck active native authorization periodically, rather than perform
                        // HTTP/RPC preflight on every mouse move. Rejected status also revokes it.
                        let fresh = !control
                            && grant.validated.lock().unwrap().elapsed() < Duration::from_secs(5);
                        if !fresh {
                            let result = resolve(&k.0)
                                .and_then(|e| preview_transport::surface(&e, &grant.scope));
                            match result {
                                Ok(v)
                                    if v["snapshot"]
                                        .get("profileId")
                                        .is_none_or(|p| p == &grant.scope["profileId"]) =>
                                {
                                    *grant.validated.lock().unwrap() = Instant::now()
                                }
                                Ok(_) | Err(_) => {
                                    grant.allowed.store(false, Ordering::SeqCst);
                                    error(&worker_output, &grant.request, "permission_denied");
                                    continue;
                                }
                            }
                        }
                        let result = if control {
                            let action = text(&input, "action");
                            let profile = text(&grant.scope, "profileId");
                            let event = match action {
                                "back" | "forward" | "stop" => json!({"type":action}),
                                "hard-reload" => json!({"type":"refresh","bypassCache":true}),
                                "zoom-in" | "zoom-out" | "zoom-reset" => {
                                    let levels = crate::browser_preferences::contract()
                                        ["zoomLevels"]
                                        .as_array()
                                        .unwrap()
                                        .iter()
                                        .filter_map(Value::as_f64)
                                        .collect::<Vec<_>>();
                                    let old = *grant.zoom.lock().unwrap();
                                    let index = levels.iter().position(|z| *z == old).unwrap_or(7);
                                    let next = if action == "zoom-reset" {
                                        1.0
                                    } else if action == "zoom-in" {
                                        levels[(index + 1).min(levels.len() - 1)]
                                    } else {
                                        levels[index.saturating_sub(1)]
                                    };
                                    json!({"type":"zoom","value":next})
                                }
                                _ => {
                                    error(&worker_output, &grant.request, "invalid_browser_input");
                                    continue;
                                }
                            };
                            let result = pool.admitted(&k.0, profile, &k.1, &k.2, &event);
                            if result.is_ok() && event["type"] == "zoom" {
                                *grant.zoom.lock().unwrap() = event["value"].as_f64().unwrap();
                                worker_output(
                                    json!({"kind":"zoom","requestId":grant.request,"value":event["value"]}),
                                );
                            }
                            result
                        } else {
                            pool.input(text(&input, "requestId"), &input["event"])
                        };
                        if let Err(e) = result {
                            error(&worker_output, text(&input, "requestId"), &e);
                        }
                    }
                    Message::Observe(data) => {
                        let e = text(&data, "environment");
                        let t = text(&data, "thread");
                        if data["kind"] == "disconnected" {
                            if active
                                .as_ref()
                                .is_some_and(|v| v["environment"] == e && v["thread"] == t)
                            {
                                pool.hide();
                                active = None;
                            }
                            continue;
                        }
                        let is_list = data["kind"] == "list";
                        let result = &data["result"];
                        let identity = (
                            text(&data, "originKey").to_owned(),
                            text(result, "serverEpoch").to_owned(),
                        );
                        if identity.0.is_empty() || identity.1.is_empty() {
                            continue;
                        }
                        if !is_list && epochs.get(e) != Some(&identity) {
                            continue;
                        }
                        if is_list && epochs.get(e).is_some_and(|old| old != &identity) {
                            invalidate(&mut pool, &grants, e);
                            revisions.retain(|(id, _), _| id != e);
                        }
                        epochs.insert(e.into(), identity);
                        let revision = result["revision"].as_u64().unwrap_or(0);
                        let rk = (e.to_owned(), t.to_owned());
                        if revisions.get(&rk).is_some_and(|r| *r > revision) {
                            continue;
                        }
                        revisions.insert(rk, revision);
                        let closed = if is_list {
                            grants
                                .lock()
                                .unwrap()
                                .iter()
                                .filter(|((env, thread, tab), _)| {
                                    env == e
                                        && thread == t
                                        && !result["sessions"].as_array().is_some_and(|ss| {
                                            ss.iter().any(|s| {
                                                s["tabId"] == *tab && s["runtime"] != "server"
                                            })
                                        })
                                })
                                .map(|(k, _)| k.clone())
                                .collect::<Vec<_>>()
                        } else if result["type"] == "closed" {
                            vec![key(e, t, text(result, "tabId"))]
                        } else {
                            vec![]
                        };
                        for k in closed {
                            close(&mut pool, &grants, &k);
                        }
                    }
                    Message::Action(input, result) => {
                        let Ok(receipt) = result else { continue };
                        if receipt["state"] != "complete" {
                            continue;
                        }
                        let e = text(&input, "environment");
                        let t = text(&input, "thread");
                        let tab = text(&input, "tabId");
                        let targets = grants
                            .lock()
                            .unwrap()
                            .iter()
                            .filter(|((env, thread, id), g)| {
                                env == e
                                    && thread == t
                                    && (tab.is_empty() || id == tab)
                                    && g.scope["originKey"] == input["originKey"]
                                    && g.scope["serverEpoch"] == input["serverEpoch"]
                            })
                            .map(|(k, g)| (k.clone(), g.clone()))
                            .collect::<Vec<_>>();
                        for (k, grant) in targets {
                            let profile = text(&grant.scope, "profileId");
                            let event = match text(&input, "action") {
                                "close" => {
                                    close(&mut pool, &grants, &k);
                                    continue;
                                }
                                "navigate" => {
                                    json!({"type":"navigate","url":receipt["snapshot"]["navStatus"]["url"]})
                                }
                                "refresh" => json!({"type":"refresh"}),
                                _ => continue,
                            };
                            if let Err(err) = pool.admitted(e, profile, t, &k.2, &event) {
                                error(&worker_output, &grant.request, &err);
                            }
                        }
                    }
                }
            }
            for grant in grants.lock().unwrap().values() {
                grant.allowed.store(false, Ordering::SeqCst);
            }
        });
        Self {
            sender,
            generation,
            desired,
            output,
            ready,
            import_assets,
        }
    }
    pub fn request(&self, input: Value, visible: bool) {
        match text(&input, "op") {
            "browser-hide" => self.hide(),
            "browser-show" => {
                if !visible {
                    error(
                        &self.output,
                        text(&input, "requestId"),
                        "browser_view_changed",
                    );
                    return;
                }
                let id = text(&input, "requestId");
                if id.is_empty() || id.len() > 128 {
                    error(&self.output, id, "browser_view_changed");
                    return;
                }
                let stamp = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
                *self.desired.lock().unwrap() = Some(input.clone());
                if self
                    .sender
                    .try_send(Message::Show(input.clone(), stamp))
                    .is_err()
                {
                    *self.desired.lock().unwrap() = None;
                    error(&self.output, id, "browser_unavailable");
                }
            }
            "browser-input" if visible && input.to_string().len() <= 10000 => {
                let _ = self.sender.try_send(Message::Input(
                    input,
                    self.generation.load(Ordering::SeqCst),
                ));
            }
            "browser-control" if visible && input.to_string().len() <= 1000 => {
                let _ = self.sender.try_send(Message::Control(
                    input,
                    self.generation.load(Ordering::SeqCst),
                ));
            }
            _ => {}
        }
    }
    pub fn hide(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        *self.desired.lock().unwrap() = None;
        let _ = self.sender.try_send(Message::Hide);
    }
    /// Explicit worker shutdown waits for Qt profile flush. A crash is covered
    /// separately by the child parent-death signal and scoped recovery state.
    pub fn shutdown(&self) -> Result<()> {
        self.generation.fetch_add(1, Ordering::SeqCst);
        *self.desired.lock().unwrap() = None;
        let (done, wait) = mpsc::channel();
        self.sender
            .send(Message::Shutdown(done))
            .map_err(|_| "browser_unavailable")?;
        wait.recv_timeout(Duration::from_secs(10))
            .map_err(|_| "browser_shutdown_uncertain".into())
    }
    /// Trusted backend producers only; these are not public stdin operations.
    pub fn observe(&self, data: Value) {
        if self.sender.try_send(Message::Observe(data)).is_err() {
            self.hide();
        }
    }
    pub fn action(&self, input: Value, result: Result<Value>) {
        if self
            .sender
            .try_send(Message::Action(input, result))
            .is_err()
        {
            self.hide();
        }
    }
}
fn dimensions(snapshot: &Value, input: &Value) -> Result<(u64, u64)> {
    let v = &snapshot["viewport"];
    let (w, h) = if matches!(text(v, "_tag"), "preset" | "freeform") {
        (v["width"].clone(), v["height"].clone())
    } else {
        (
            input.get("width").cloned().unwrap_or(json!(900)),
            input.get("height").cloned().unwrap_or(json!(620)),
        )
    };
    let checked = crate::browser_preferences::validate(
        "browserDefaultViewport",
        &json!({"_tag":"freeform","width":w,"height":h}),
    )?;
    Ok((
        checked["width"].as_u64().unwrap(),
        checked["height"].as_u64().unwrap(),
    ))
}
fn invalidate(pool: &mut browser_guest::Pool, grants: &Grants, environment: &str) {
    let keys = grants
        .lock()
        .unwrap()
        .keys()
        .filter(|(e, _, _)| e == environment)
        .cloned()
        .collect::<Vec<_>>();
    for k in keys {
        if let Some(g) = grants.lock().unwrap().remove(&k) {
            g.allowed.store(false, Ordering::SeqCst);
        }
    }
    pool.close_environment(environment);
}
fn close(pool: &mut browser_guest::Pool, grants: &Grants, k: &Key) {
    let grant = grants.lock().unwrap().remove(k);
    if let Some(g) = grant {
        g.allowed.store(false, Ordering::SeqCst);
        let _ = pool.close_tab(&k.0, text(&g.scope, "profileId"), &k.1, &k.2);
    }
}
