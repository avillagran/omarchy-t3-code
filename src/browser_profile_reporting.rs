//! Global native profile-report targets and connection-owned report transport.
//! Config snapshots supply identity; report errors never schedule a replay.
use crate::{environments, json, protocol, rpc_sessions, text, Environment, Result};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Condvar, Mutex,
    },
    time::Duration,
};

#[derive(Default)]
struct Mailbox {
    stopped: bool,
    latest: Option<Value>,
}
type Target = Arc<Mutex<Option<(String, String)>>>;
struct Watch {
    stop: Arc<AtomicBool>,
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
}
impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let (lock, wake) = &*self.mailbox;
        if let Ok(mut state) = lock.lock() {
            state.stopped = true;
            state.latest = None;
            wake.notify_one();
        }
    }
}
/// One target/config watcher and one bounded latest-report mailbox per actual
/// configured environment. Neither is attached to a selected thread or panel.
pub struct Control {
    watches: HashMap<String, Watch>,
    notices: Arc<Notices>,
}
impl Control {
    pub fn new(events: mpsc::Sender<Value>) -> Self {
        Self {
            watches: HashMap::new(),
            notices: Arc::new(Notices {
                events,
                state: Mutex::new(NoticeState::default()),
            }),
        }
    }
    pub fn sync(&mut self, environments: &[Environment]) {
        self.watches.retain(|id, _| {
            let keep = environments.iter().any(|e| &e.id == id);
            if !keep {
                self.notices.remove(id);
            }
            keep
        });
        for environment in environments {
            if self.watches.contains_key(&environment.id) {
                continue;
            }
            let id = environment.id.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let mailbox = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
            let identity = uuid::Uuid::new_v4().to_string();
            let watch = Watch {
                stop: stop.clone(),
                mailbox: mailbox.clone(),
            };
            let target = Arc::new(Mutex::new(None));
            let observed = target.clone();
            self.notices.register(&id, &identity);
            let events = self.notices.clone();
            let target_id = id.clone();
            std::thread::spawn(move || watch_target(target_id, identity, stop, observed, events));
            std::thread::spawn(move || report_worker(id, mailbox, target));
            self.watches.insert(environment.id.clone(), watch);
        }
    }
    /// The GUI records native due-state before submission. Coalesce only queued
    /// reports to the latest real settings/config; never retry a failed request.
    pub fn take_targets(&self) -> Vec<Value> {
        self.notices.take()
    }
    pub fn submit(&self, input: &Value) {
        let Some(watch) = self.watches.get(text(input, "environment")) else {
            return;
        };
        let (lock, wake) = &*watch.mailbox;
        if let Ok(mut state) = lock.lock() {
            if !state.stopped {
                if let Ok(value) = bounded_input(input) {
                    state.latest = Some(value);
                }
                wake.notify_one();
            }
        }
    }
}
fn connection(id: &str) -> Result<protocol::Connection> {
    environments()?
        .into_iter()
        .find(|e| e.id == id)
        .ok_or("unknown_environment")?
        .connection(true)
}
fn operates(connection: &protocol::Connection) -> Result<bool> {
    let auth = connection.request_json("GET", "/api/auth/session", None, false)?;
    Ok(auth["authenticated"] == true
        && auth
            .get("permissions")
            .or_else(|| auth.get("scopes"))
            .and_then(Value::as_array)
            .is_some_and(|scopes| scopes.iter().any(|s| s == "preview:operate")))
}
fn current(id: &str, connection: &protocol::Connection, session: &protocol::RpcSession) -> bool {
    session.is_open()
        && rpc_sessions::is_current(id, true, session.id()).unwrap_or(false)
        && self::connection(id).is_ok_and(|c| connection.same_session_admission(&c))
}
#[derive(Default)]
struct NoticeState {
    generations: HashMap<String, String>,
    latest: HashMap<String, Value>,
    notified: bool,
}
struct Notices {
    events: mpsc::Sender<Value>,
    state: Mutex<NoticeState>,
}
impl Notices {
    fn register(&self, id: &str, generation: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.generations.insert(id.into(), generation.into());
        }
    }
    fn remove(&self, id: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.generations.remove(id);
            state.latest.insert(
                id.into(),
                json!({"environment":id,"removed":true,"operable":false}),
            );
            self.wake(&mut state);
        }
    }
    fn wake(&self, state: &mut NoticeState) {
        if !state.notified {
            state.notified = true;
            let _ = self
                .events
                .send(json!({"op":"browser-profile-targets-ready"}));
        }
    }
    fn take(&self) -> Vec<Value> {
        let Ok(mut state) = self.state.lock() else {
            return Vec::new();
        };
        state.notified = false;
        state.latest.drain().map(|(_, event)| event).collect()
    }
}
fn publish(events: &Notices, id: &str, watch: &str, session: &str, config: Option<&str>) {
    if let Ok(mut state) = events.state.lock() {
        if state
            .generations
            .get(id)
            .is_some_and(|current| current == watch)
        {
            state.latest.insert(
                id.into(),
                json!({"environment":id,"sessionId":session,
                "configId":config,"operable":config.is_some()}),
            );
            events.wake(&mut state);
        }
    }
}
fn bounded_input(input: &Value) -> Result<Value> {
    let session = text(input, "sessionId");
    let config = text(input, "configId");
    uuid::Uuid::parse_str(session).map_err(|_| "invalid_profile_report")?;
    uuid::Uuid::parse_str(config).map_err(|_| "invalid_profile_report")?;
    let value = &input["input"];
    Ok(json!({"sessionId":session,"configId":config,"input":{
        "profiles":crate::browser_preferences::validate("browserProfiles", &value["profiles"])?,
        "defaultProfileId":crate::browser_preferences::validate("browserDefaultProfileId", &value["defaultProfileId"])?,
        "recordingFrameRate":crate::browser_preferences::validate("browserRecordingFrameRate", &value["recordingFrameRate"])?}}))
}
/// Wire numbers become JS Numbers in the original projection. In particular,
/// equal integer/float encodings must not manufacture a new config identity.
fn js_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| js_equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, value)| b.get(key).is_some_and(|other| js_equal(value, other)))
        }
        _ => left == right,
    }
}
#[derive(Default)]
struct ConfigProjection {
    value: Option<Value>,
    // JS own properties whose value is undefined are distinct from missing
    // properties. These bits are internal identity state, never wire fields.
    undefined: u8,
}
/// Mirror the pinned config projection followed by presentation.withoutProviders.
/// Providers never change report identity; equal snapshots/settings retain it.
fn project_config(previous: &mut ConfigProjection, event: &Value) -> bool {
    if event["version"] != 1 {
        return false;
    }
    let kind = text(event, "type");
    let mut force = false;
    let mut undefined = previous.undefined;
    let mut next = match kind {
        "snapshot" if event["config"].is_object() => {
            let mut next = event["config"].clone();
            // Original carried undefined values are omitted by the snapshot
            // spread, removing their own keys and changing presentation identity.
            undefined = 0;
            for key in ["environmentThemes", "usageLimitSources"] {
                if let Some(value) = previous.value.as_ref().and_then(|v| v.get(key)) {
                    next[key] = value.clone();
                }
            }
            next
        }
        "keybindingsUpdated"
        | "settingsUpdated"
        | "environmentThemesUpdated"
        | "usageLimitSourcesUpdated" => {
            let Some(current) = previous.value.as_ref() else {
                return false;
            };
            let mut next = current.clone();
            match kind {
                "keybindingsUpdated" => {
                    next["keybindings"] = event["payload"]["keybindings"].clone();
                    next["issues"] = event["payload"]["issues"].clone();
                    force = true;
                }
                "settingsUpdated" => next["settings"] = event["payload"]["settings"].clone(),
                _ => {
                    let (key, field, bit) = if kind == "environmentThemesUpdated" {
                        ("environmentThemes", "themes", 1)
                    } else {
                        ("usageLimitSources", "sources", 2)
                    };
                    let values = &event["payload"][field];
                    if values.as_array().is_some_and(|values| !values.is_empty()) {
                        next[key] = values.clone();
                        undefined &= !bit;
                        force = true;
                    } else {
                        next.as_object_mut().expect("config object").remove(key);
                        undefined |= bit;
                    }
                }
            }
            next
        }
        // The actual presentation explicitly excludes providers.
        _ => return false,
    };
    next.as_object_mut()
        .expect("config object")
        .remove("providers");
    let changed = force
        || previous.undefined != undefined
        || !previous
            .value
            .as_ref()
            .is_some_and(|old| js_equal(old, &next));
    previous.value = Some(next);
    previous.undefined = undefined;
    changed
}

fn watch_target(
    id: String,
    identity: String,
    stop: Arc<AtomicBool>,
    target: Target,
    events: Arc<Notices>,
) {
    let mut retry = 1;
    let mut config = ConfigProjection::default();
    let mut config_id = String::new();
    let mut previous_session = String::new();
    while !stop.load(Ordering::Acquire) {
        let mut observed_session = String::new();
        let result = (|| -> Result<()> {
            let (connection, session) = rpc_sessions::admitted(&id, true)?;
            observed_session = session.id().to_owned();
            if previous_session != session.id() {
                if let Ok(mut current) = target.lock() {
                    *current = None;
                }
                publish(&events, &id, &identity, &previous_session, None);
                config = ConfigProjection::default();
                previous_session = session.id().to_owned();
            }
            if !operates(&connection)? {
                if let Ok(mut current) = target.lock() {
                    *current = None;
                }
                publish(&events, &id, &identity, session.id(), None);
                return Err("permission_denied".into());
            }
            protocol::api::streams::subscribe_server_config(
                &connection,
                &Default::default(),
                || stop.load(Ordering::Acquire) || !current(&id, &connection, &session),
                || false,
                |event| {
                    if stop.load(Ordering::Acquire) || !current(&id, &connection, &session) {
                        return Ok(());
                    }
                    if let Some(event) = event {
                        let changed = project_config(&mut config, &event);
                        let missing = target.lock().is_ok_and(|value| value.is_none());
                        if changed {
                            config_id = uuid::Uuid::new_v4().to_string();
                        }
                        if changed || missing && config.value.is_some() {
                            if config.value.as_ref().is_some_and(|value| {
                                value["environment"]["capabilities"]["serverBrowser"] == true
                            }) {
                                if let Ok(mut current) = target.lock() {
                                    *current = Some((session.id().to_owned(), config_id.clone()));
                                }
                                publish(&events, &id, &identity, session.id(), Some(&config_id));
                                retry = 1;
                            } else {
                                if let Ok(mut current) = target.lock() {
                                    *current = None;
                                }
                                publish(&events, &id, &identity, session.id(), None);
                            }
                        }
                    }
                    Ok(())
                },
            )
        })();
        // A failed config stream does not replay a failed report. Retain the
        // last config identity while its actual write socket is still current.
        let alive = rpc_sessions::existing(&id, true)
            .ok()
            .flatten()
            .is_some_and(|session| session.id() == observed_session);
        if stop.load(Ordering::Acquire) || !alive {
            if let Ok(mut current) = target.lock() {
                *current = None;
            }
            publish(&events, &id, &identity, &observed_session, None);
        }
        if stop.load(Ordering::Acquire) {
            break;
        }
        let _ = result;
        for _ in 0..retry * 10 {
            if stop.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        retry = (retry * 2).min(30);
    }
}

fn report_worker(id: String, mailbox: Arc<(Mutex<Mailbox>, Condvar)>, target: Target) {
    loop {
        let input = {
            let (lock, wake) = &*mailbox;
            let Ok(mut state) = lock.lock() else { return };
            while !state.stopped && state.latest.is_none() {
                let Ok(next) = wake.wait(state) else { return };
                state = next;
            }
            if state.stopped {
                return;
            }
            state.latest.take().expect("queued report")
        };
        // The expected physical socket was already admitted by the config
        // watcher. Never reopen a socket to deliver an obsolete report.
        let _ = report(&id, &input, &target);
    }
}
fn report(id: &str, input: &Value, target: &Target) -> Result<()> {
    let matches = || {
        target.lock().is_ok_and(|current| {
            current.as_ref().is_some_and(|(session, config)| {
                session == text(input, "sessionId") && config == text(input, "configId")
            })
        })
    };
    if !matches() {
        return Err("connection_changed".into());
    }
    let session = rpc_sessions::existing(id, true)?.ok_or("connection_changed")?;
    if session.id() != text(input, "sessionId") {
        return Err("connection_changed".into());
    }
    let connection = self::connection(id)?;
    if !current(id, &connection, &session) || !operates(&connection)? {
        return Err("permission_denied".into());
    }
    let value = &input["input"];
    let profiles = crate::browser_preferences::validate("browserProfiles", &value["profiles"])?;
    let default = crate::browser_preferences::validate(
        "browserDefaultProfileId",
        &value["defaultProfileId"],
    )?;
    let rate = crate::browser_preferences::validate(
        "browserRecordingFrameRate",
        &value["recordingFrameRate"],
    )?;
    if !matches() || !current(id, &connection, &session) {
        return Err("connection_changed".into());
    }
    // The server reporter token belongs to THIS persistent socket. Awaiting the
    // response retains only this request, not a disposable one-shot connection.
    session
        .start(
            "preview.reportProfiles",
            json!({"profiles":profiles,"defaultProfileId":default,
        "recordingFrameRate":rate}),
            Duration::from_secs(20),
        )?
        .wait()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_push_and_equal_snapshot_keep_config_identity() {
        let mut config = ConfigProjection::default();
        let snapshot = json!({"version":1,"type":"snapshot","config":{
            "environment":{"capabilities":{"serverBrowser":true}},"providers":[],"settings":{"n":1}}});
        assert!(project_config(&mut config, &snapshot));
        assert!(!project_config(&mut config, &snapshot));
        assert!(!project_config(
            &mut config,
            &json!({"version":1,"type":"providerStatuses","payload":{"providers":[{"id":"changed"}]}})
        ));
        assert!(!project_config(
            &mut config,
            &json!({"version":1,"type":"settingsUpdated","payload":{"settings":{"n":1}}})
        ));
        assert!(project_config(
            &mut config,
            &json!({"version":1,"type":"settingsUpdated","payload":{"settings":{"n":2}}})
        ));
        assert!(project_config(
            &mut config,
            &json!({"version":1,"type":"keybindingsUpdated","payload":{"keybindings":[],"issues":[]}})
        ));
        assert!(project_config(
            &mut config,
            &json!({"version":1,"type":"keybindingsUpdated","payload":{"keybindings":[],"issues":[]}})
        ));
    }

    #[test]
    fn whole_native_projection_reference_preserves_undefined_keys_and_duration_identity() {
        let reference: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/browser-profile-config-projection-4314.json"
        ))
        .unwrap();
        assert_eq!(reference["wholeProjection"], true);
        assert_eq!(reference["wholeWithoutProviders"], true);
        assert_eq!(reference["effect"], "4.1.2");
        let mut projection = ConfigProjection::default();
        let rows = reference["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 20);
        for row in rows {
            let event: Value = serde_json::from_str(row["eventJson"].as_str().unwrap()).unwrap();
            assert_eq!(
                project_config(&mut projection, &event),
                row["changed"].as_bool().unwrap(),
                "{}",
                row["name"]
            );
            let mut undefined = Vec::new();
            if projection.undefined & 1 != 0 {
                undefined.push("environmentThemes");
            }
            if projection.undefined & 2 != 0 {
                undefined.push("usageLimitSources");
            }
            assert_eq!(json!(undefined), row["ownUndefined"], "{}", row["name"]);
        }
    }

    #[test]
    fn coalesced_notices_reject_retired_watcher_and_keep_latest_target() {
        let (tx, rx) = mpsc::channel();
        let notices = Notices {
            events: tx,
            state: Mutex::new(NoticeState::default()),
        };
        notices.register("a", "old");
        publish(&notices, "a", "old", "socket", Some("first"));
        publish(&notices, "a", "old", "socket", Some("second"));
        assert!(rx.try_recv().is_ok());
        assert!(rx.try_recv().is_err());
        notices.remove("a");
        notices.register("a", "new");
        publish(&notices, "a", "new", "next-socket", Some("third"));
        publish(&notices, "a", "old", "socket", None);
        let current = notices.take();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0]["configId"], "third");
        publish(&notices, "a", "new", "next-socket", None);
        assert!(rx.try_recv().is_ok());
        assert_eq!(notices.take()[0]["operable"], false);
    }
}
