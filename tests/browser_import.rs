use omarchy_t3_client::{
    browser_cleanup::{Lease, Runtime, Scope},
    browser_cookies::ImportedCookie,
    browser_import as import, protocol, Environment, Result,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
static TEST: Mutex<()> = Mutex::new(());
struct Server {
    env: Environment,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    proof_requests: Arc<Mutex<Vec<String>>>,
}
impl Server {
    fn new() -> Self {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        socket.set_nonblocking(true).unwrap();
        let base = format!("http://{}", socket.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let running = stop.clone();
        let proof_requests = Arc::new(Mutex::new(vec![]));
        let proofs = proof_requests.clone();
        let worker = thread::spawn(move || {
            while !running.load(Ordering::Relaxed) {
                let (mut stream, _) = match socket.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut peek = [0u8; 1024];
                let n = stream.peek(&mut peek).unwrap();
                if !String::from_utf8_lossy(&peek[..n]).contains("Upgrade: websocket")
                    && !String::from_utf8_lossy(&peek[..n])
                        .to_lowercase()
                        .contains("upgrade: websocket")
                {
                    let mut header = vec![];
                    let mut b = [0];
                    while !header.ends_with(b"\r\n\r\n") {
                        stream.read_exact(&mut b).unwrap();
                        header.push(b[0]);
                    }
                    if String::from_utf8_lossy(&header).starts_with("GET /proof ") {
                        proofs
                            .lock()
                            .unwrap()
                            .push(String::from_utf8_lossy(&header).into_owned());
                    }
                    let body = json!({"authenticated":true,"permissions":["orchestration:read"]})
                        .to_string();
                    write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                    continue;
                }
                let mut ws = tungstenite::accept(stream).unwrap();
                let request: Value =
                    serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
                assert_eq!(request["tag"], "server.getConfig");
                let value = json!({"environment":{"capabilities":{"serverBrowser":false}},"settings":{"secret":"SERVER_PRIVATE"}});
                let _ = ws.send(tungstenite::Message::Text(
                    json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}})
                        .to_string()
                        .into(),
                ));
            }
        });
        let env = Environment {
            id: "fixture".into(),
            name: "Fixture".into(),
            base_url: base.clone(),
            token_file: "/unused".into(),
            write_token_file: None,
            bearer: None,
            connection_error: None,
            connection: Some(Arc::new(
                protocol::Connection::new(
                    &base,
                    None,
                    protocol::Credential::bearer("synthetic").unwrap(),
                )
                .unwrap(),
            )),
        };
        Self {
            env,
            stop,
            worker: Some(worker),
            proof_requests,
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}
struct Data {
    root: PathBuf,
    home: PathBuf,
    path: PathBuf,
}
impl Data {
    fn new(cookies: usize) -> Self {
        let root = std::env::temp_dir().join(format!("ot3-import-{}", uuid::Uuid::new_v4()));
        let home = root.join("home");
        let path = root.join("native/userdata/client-settings.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, settings().to_string()).unwrap();
        let profile = home.join(".mozilla/firefox/profile");
        fs::create_dir_all(&profile).unwrap();
        let mut sql="pragma user_version=16;create table moz_cookies(host,name,value,path,expiry,isSecure,isHttpOnly,sameSite,originAttributes);".to_owned();
        let expiry = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600)
            * 1000;
        for n in 0..cookies {
            sql+=&format!("insert into moz_cookies values('example.test','cookie{n}','COOKIE_PRIVATE','/',{expiry},1,1,2,'');")
        }
        assert!(std::process::Command::new("sqlite3")
            .arg(profile.join("cookies.sqlite"))
            .arg(sql)
            .status()
            .unwrap()
            .success());
        Self { root, home, path }
    }
}
impl Drop for Data {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn settings() -> Value {
    json!({"browserProfiles":[{"id":"work","name":"Firefox","kind":"persistent"}],"browserDefaultProfileId":"work","future":{"preserved":true},"secret":"CLIENT_PRIVATE"})
}
fn request(view: &Value, kind: &str) -> Value {
    json!({"environment":"fixture","viewId":view["viewId"],"sourceId":"firefox","sourceProfileDirectory":"profile","target":{"kind":kind,"profileId":if kind=="new"{format!("profile-{}",uuid::Uuid::new_v4())}else{"work".into()}},"confirmed":true})
}
#[derive(Clone)]
struct Destination {
    events: Arc<Mutex<Vec<String>>>,
    path: PathBuf,
    mode: &'static str,
    patch: Option<Value>,
}
impl Destination {
    fn new(d: &Data) -> Self {
        Self {
            events: Arc::new(Mutex::new(vec![])),
            path: d.path.clone(),
            mode: "success",
            patch: None,
        }
    }
}
struct Held(Destination, usize);
impl Runtime for Destination {
    fn available(&self) -> bool {
        true
    }
    fn begin(&self, _profile: &str, path: &Path, scopes: &[Scope]) -> Result<Box<dyn Lease>> {
        assert_eq!(path, self.path);
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].environment, "fixture");
        self.events.lock().unwrap().push("begin".into());
        Ok(Box::new(Held(self.clone(), 0)))
    }
}
impl Lease for Held {
    fn clear(&mut self, _e: &str) -> Result<()> {
        self.0.events.lock().unwrap().push("rollback".into());
        if self.0.mode == "rollback-fail" {
            Err("unknown".into())
        } else {
            Ok(())
        }
    }
    fn write_cookie(&mut self, _e: &str, c: ImportedCookie) -> Result<bool> {
        assert_eq!(c.value, "COOKIE_PRIVATE");
        if self.0.mode == "unavailable" {
            return Err("browser_runtime_unavailable".into());
        }
        self.1 += 1;
        self.0.events.lock().unwrap().push("write".into());
        if self.0.mode == "unknown" && self.1 == 2 {
            return Err("browser_cookie_import_unknown".into());
        }
        Ok(self.0.mode != "reject")
    }
    fn flush_import(&mut self, _e: &str) -> Result<()> {
        self.0.events.lock().unwrap().push("flush".into());
        if self.0.mode == "redirect" {
            use std::os::unix::fs::symlink;
            fs::rename(&self.0.path, self.0.path.with_extension("old.json")).unwrap();
            let target = self.0.path.with_extension("redirect.json");
            fs::write(&target, settings().to_string()).unwrap();
            symlink(&target, &self.0.path).unwrap();
        }
        if let Some(patch) = &self.0.patch {
            fs::write(&self.0.path, patch.to_string()).unwrap();
        }
        if self.0.mode == "flush-fail" {
            Err("unknown".into())
        } else {
            Ok(())
        }
    }
}
impl Drop for Held {
    fn drop(&mut self) {
        self.0.events.lock().unwrap().push("end".into());
    }
}
fn view(s: &Server, d: &Data, r: &dyn Runtime) -> Value {
    import::read_at(
        &[s.env.clone()],
        &json!({"environment":"fixture"}),
        &d.home,
        &d.path,
        r,
    )
    .unwrap()
}
fn apply(s: &Server, d: &Data, input: &Value, r: &dyn Runtime) -> Result<Value> {
    import::apply_at(
        &[s.env.clone()],
        input,
        &d.home,
        &d.path,
        &d.root.join("no-keyring-helper"),
        r,
    )
}
#[test]
fn original_new_profile_registration_follows_writes_flush_and_preserves_latest_preferences() {
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    let mut dest = Destination::new(&d);
    let mut latest = settings();
    latest["browserDefaultZoomFactor"] = json!(1.75);
    dest.patch = Some(latest);
    let v = view(&s, &d, &dest);
    assert!(!v.to_string().contains("PRIVATE"));
    let req = request(&v, "new");
    let result = apply(&s, &d, &req, &dest).unwrap();
    assert_eq!(result["kind"], "imported");
    assert_eq!(result["imported"], 1);
    assert_eq!(result["targetName"], "Firefox 2");
    assert_eq!(result["data"]["browserDefaultZoomFactor"], 1.75);
    assert!(!result.to_string().contains("PRIVATE"));
    let stored: Value = serde_json::from_slice(&fs::read(&d.path).unwrap()).unwrap();
    assert_eq!(stored["browserProfiles"].as_array().unwrap().len(), 2);
    assert_eq!(stored["browserDefaultProfileId"], "work");
    assert_eq!(stored["future"]["preserved"], true);
    assert_eq!(stored["secret"], "CLIENT_PRIVATE");
    assert_eq!(
        *dest.events.lock().unwrap(),
        vec!["begin", "write", "flush", "end"]
    );
    assert_eq!(
        apply(&s, &d, &req, &dest).unwrap_err(),
        "browser_import_view_expired"
    );
}
#[test]
fn empty_or_fully_skipped_import_does_not_register_a_new_profile() {
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    for (cookies, mode) in [(0, "success"), (1, "reject")] {
        let d = Data::new(cookies);
        let mut dest = Destination::new(&d);
        dest.mode = mode;
        let before = fs::read(&d.path).unwrap();
        let v = view(&s, &d, &dest);
        let result = apply(&s, &d, &request(&v, "new"), &dest).unwrap();
        assert_eq!(result["kind"], "imported");
        assert_eq!(result["imported"], 0);
        assert_eq!(result["skipped"], cookies);
        assert_eq!(fs::read(&d.path).unwrap(), before);
    }
}
#[test]
fn partial_unknown_or_flush_failure_never_registers_or_replays_and_clears_new_target() {
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    for mode in ["unknown", "flush-fail"] {
        let d = Data::new(2);
        let mut dest = Destination::new(&d);
        dest.mode = mode;
        let before = fs::read(&d.path).unwrap();
        let v = view(&s, &d, &dest);
        let req = request(&v, "new");
        let result = apply(&s, &d, &req, &dest).unwrap();
        assert_eq!(result["state"], "uncertain");
        assert_eq!(result["rolledBack"], true);
        assert_eq!(fs::read(&d.path).unwrap(), before);
        assert!(dest.events.lock().unwrap().contains(&"rollback".into()));
        assert_eq!(
            apply(&s, &d, &req, &dest).unwrap_err(),
            "browser_import_view_expired"
        );
    }
}
#[test]
fn concurrent_profile_limit_rolls_back_new_storage_without_losing_other_changes() {
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    let mut dest = Destination::new(&d);
    let mut latest = settings();
    latest["browserProfiles"] = json!((0..24)
        .map(|i| json!({"id":format!("other-{i}"),"name":format!("Other {i}"),"kind":"persistent"}))
        .collect::<Vec<_>>());
    dest.patch = Some(latest.clone());
    let v = view(&s, &d, &dest);
    let result = apply(&s, &d, &request(&v, "new"), &dest).unwrap();
    assert_eq!(result["state"], "failed");
    assert_eq!(result["reason"], "profileLimitReached");
    assert_eq!(result["rolledBack"], true);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&d.path).unwrap()).unwrap(),
        latest
    );
}
#[test]
fn preflight_confirmation_scope_identity_and_source_membership_fail_before_any_write() {
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    let dest = Destination::new(&d);
    let v = view(&s, &d, &dest);
    let mut req = request(&v, "existing");
    req["confirmed"] = json!(false);
    assert_eq!(
        apply(&s, &d, &req, &dest).unwrap_err(),
        "confirmation_required"
    );
    req["confirmed"] = json!(true);
    req["environment"] = json!("foreign");
    assert_eq!(
        apply(&s, &d, &req, &dest).unwrap_err(),
        "browser_profile_scope_changed"
    );
    req["environment"] = json!("fixture");
    req["sourceProfileDirectory"] = json!("../../outside");
    assert_eq!(
        apply(&s, &d, &req, &dest).unwrap_err(),
        "unknownSourceProfile"
    );
    req["sourceProfileDirectory"] = json!("profile");
    let mut changed = settings();
    changed["browserProfiles"][0]["name"] = json!("Renamed");
    fs::write(&d.path, changed.to_string()).unwrap();
    assert_eq!(
        apply(&s, &d, &req, &dest).unwrap_err(),
        "browser_profile_changed_reload"
    );
    assert!(dest.events.lock().unwrap().is_empty());
}
#[test]
fn real_qt_destination_imports_synthetic_firefox_and_registers_only_after_verified_flush() {
    use omarchy_t3_client::{browser_guest, browser_sessions};
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    let config = browser_guest::Config::isolated(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/omarchy-t3-client"),
        &d.root,
    )
    .unwrap();
    let env = s.env.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let manager = browser_sessions::Manager::start(
        config,
        move |id| {
            if id == env.id {
                Ok(env.clone())
            } else {
                Err("unknown_environment".into())
            }
        },
        || Ok(json!({})),
        move |v| {
            let _ = tx.send(v);
        },
    );
    let v = view(&s, &d, &manager);
    let req = request(&v, "new");
    let result = apply(&s, &d, &req, &manager).unwrap();
    assert_eq!(result["state"], "complete", "{result}");
    assert_eq!(result["imported"], 1);
    assert_eq!(result["targetName"], "Firefox 2");
    assert!(!result.to_string().contains("PRIVATE"));
    manager.shutdown().unwrap();
    assert!(!rx
        .try_iter()
        .any(|v| v.to_string().contains("COOKIE_PRIVATE")));
}

#[test]
fn actual_watch_routes_confirmed_import_to_qt_and_does_not_replay_consumed_view() {
    use std::{
        io::{BufRead, BufReader},
        os::unix::fs::{symlink, PermissionsExt},
        process::{Command, Stdio},
        time::Instant,
    };
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bin = d.root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let executable = bin.join("omarchy-t3-client");
    fs::copy(env!("CARGO_BIN_EXE_omarchy-t3-client"), &executable).unwrap();
    symlink(
        repo.join("bin/libt3-browser.so"),
        bin.join("libt3-browser.so"),
    )
    .unwrap();
    symlink(repo.join("ui"), d.root.join("ui")).unwrap();
    let token = d.root.join("token");
    fs::write(&token, "synthetic").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let fixture = d.root.join("environments.json");
    fs::write(&fixture,json!({"environments":[{"id":"fixture","name":"Fixture","base_url":s.env.base_url,"token_file":token,"write_token_file":token}]}).to_string()).unwrap();
    for name in ["runtime", "cache", "config", "state", "data"] {
        fs::create_dir_all(d.root.join(name)).unwrap();
        fs::set_permissions(d.root.join(name), fs::Permissions::from_mode(0o700)).unwrap();
    }
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Child(
        Command::new(executable)
            .arg("watch")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &d.home)
            .env("T3CODE_HOME", d.root.join("native"))
            .env("XDG_CONFIG_HOME", d.root.join("config"))
            .env("XDG_STATE_HOME", d.root.join("state"))
            .env("XDG_DATA_HOME", d.root.join("data"))
            .env("XDG_CACHE_HOME", d.root.join("cache"))
            .env("XDG_RUNTIME_DIR", d.root.join("runtime"))
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", d.root.join("no-bus").display()),
            )
            .env("OMARCHY_T3_FIXTURE_ENVIRONMENTS", fixture)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if tx.send(v).is_err() {
                    break;
                }
            }
        }
    });
    let send = |child: &mut Child, input: &Value| {
        writeln!(child.0.stdin.as_mut().unwrap(), "{input}").unwrap();
        child.0.stdin.as_mut().unwrap().flush().unwrap();
    };
    let wait = |id: &str| {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let v = rx
                .recv_timeout(end.saturating_duration_since(Instant::now()))
                .expect("actual native import IPC receipt");
            assert!(
                !v.to_string().contains("COOKIE_PRIVATE"),
                "cookie values cannot enter watch output"
            );
            if v["type"] == "action" && v["data"]["requestId"] == id {
                return v["data"].clone();
            }
        }
    };
    send(
        &mut child,
        &json!({"op":"browser-import-read","environment":"fixture","requestId":"read-import"}),
    );
    let v = wait("read-import");
    assert!(v["error"].is_null(), "{v}");
    let mut req = request(&v["result"], "new");
    req["op"] = json!("browser-import-action");
    req["requestId"] = json!("import");
    send(&mut child, &req);
    let done = wait("import");
    assert!(done["error"].is_null(), "{done}");
    assert_eq!(done["result"]["state"], "complete", "{done}");
    assert_eq!(done["result"]["imported"], 1);
    let stored: Value = serde_json::from_slice(&fs::read(&d.path).unwrap()).unwrap();
    assert_eq!(stored["browserProfiles"].as_array().unwrap().len(), 2);
    req["requestId"] = json!("replay");
    send(&mut child, &req);
    assert_eq!(wait("replay")["error"], "browser_import_view_expired");
    drop(child);
    reader.join().unwrap();
}

#[test]
fn empty_native_import_neither_creates_qt_storage_nor_registers_metadata() {
    use omarchy_t3_client::{browser_guest, browser_runtime, browser_sessions};
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(0);
    let config = browser_guest::Config::isolated(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/omarchy-t3-client"),
        &d.root,
    )
    .unwrap();
    let env = s.env.clone();
    let manager = browser_sessions::Manager::start(
        config,
        move |_| Ok(env.clone()),
        || Ok(json!({})),
        |_| {},
    );
    let before = fs::read(&d.path).unwrap();
    let v = view(&s, &d, &manager);
    let req = request(&v, "new");
    let result = apply(&s, &d, &req, &manager).unwrap();
    assert_eq!(result["state"], "complete");
    assert_eq!(result["imported"], 0);
    let identity =
        browser_runtime::partition("fixture", req["target"]["profileId"].as_str().unwrap())
            .unwrap();
    assert!(!d
        .path
        .parent()
        .unwrap()
        .join("ot3-browser-profiles")
        .join(
            identity["partition"]
                .as_str()
                .unwrap()
                .trim_start_matches("persist:")
        )
        .exists());
    assert_eq!(fs::read(&d.path).unwrap(), before);
    manager.shutdown().unwrap();
}

#[test]
fn existing_qt_profile_keeps_other_cookies_and_does_not_rewrite_metadata() {
    use omarchy_t3_client::{browser_guest, browser_runtime, browser_sessions};
    use sha2::{Digest, Sha256};
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    let config = browser_guest::Config::isolated(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/omarchy-t3-client"),
        &d.root,
    )
    .unwrap();
    let env = s.env.clone();
    let manager = browser_sessions::Manager::start(
        config,
        move |_| Ok(env.clone()),
        || Ok(json!({})),
        |_| {},
    );
    let scopes = [Scope {
        environment: "fixture".into(),
        origin_key: format!("{:x}", Sha256::digest(s.env.base_url.as_bytes())),
    }];
    let mut seed = manager.begin("work", &d.path, &scopes).unwrap();
    assert!(seed
        .write_cookie(
            "fixture",
            ImportedCookie {
                url: "https://example.test/".into(),
                name: "already".into(),
                value: "OTHER_PRIVATE".into(),
                domain: None,
                path: "/".into(),
                secure: true,
                http_only: true,
                expiration_date: Some(
                    (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        + 3600) as f64
                ),
                same_site: "strict"
            }
        )
        .unwrap());
    seed.flush_import("fixture").unwrap();
    drop(seed);
    let before = fs::read(&d.path).unwrap();
    let v = view(&s, &d, &manager);
    let result = apply(&s, &d, &request(&v, "existing"), &manager).unwrap();
    assert_eq!(result["state"], "complete", "{result}");
    assert_eq!(result["targetName"], "Firefox");
    assert_eq!(fs::read(&d.path).unwrap(), before);
    let identity = browser_runtime::partition("fixture", "work").unwrap();
    let db = d
        .path
        .parent()
        .unwrap()
        .join("ot3-browser-profiles")
        .join(
            identity["partition"]
                .as_str()
                .unwrap()
                .trim_start_matches("persist:"),
        )
        .join("storage/Cookies");
    let counts = std::process::Command::new("sqlite3")
        .arg(db)
        .arg("select count(*) from cookies where name in ('cookie0','already');")
        .output()
        .unwrap();
    assert!(counts.status.success());
    assert_eq!(
        String::from_utf8_lossy(&counts.stdout).trim(),
        "2",
        "existing cookies merge with imported cookies"
    );
    manager.shutdown().unwrap();
}

#[test]
fn missing_renderer_blocks_import_before_source_or_key_access_but_keeps_local_cleanup() {
    use omarchy_t3_client::{browser_guest, browser_profiles, browser_sessions};
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    let config =
        browser_guest::Config::isolated(d.root.join("missing/bin/client"), &d.root).unwrap();
    let env = s.env.clone();
    let manager = browser_sessions::Manager::start(
        config,
        move |_| Ok(env.clone()),
        || Ok(json!({})),
        |_| {},
    );
    assert!(manager.available());
    assert!(!manager.can_import());
    assert_eq!(
        import::read_at(
            &[s.env.clone()],
            &json!({"environment":"fixture"}),
            &d.home,
            &d.path,
            &manager
        )
        .unwrap_err(),
        "sessionUnavailable"
    );
    let v = browser_profiles::read_with_at(
        &[s.env.clone()],
        &json!({"environments":["fixture"]}),
        &d.path,
        Some(&manager),
    )
    .unwrap();
    assert_eq!(v["available"], true);
    let result = browser_profiles::apply_with_at(
        &[s.env.clone()],
        &json!({"viewId":v["viewId"],"action":"clear","profileId":"work","confirmed":true}),
        &d.path,
        Some(&manager),
    )
    .unwrap();
    assert_eq!(result["state"], "complete");
    manager.shutdown().unwrap();
}

#[test]
fn known_destination_failure_before_dispatch_is_retryable_without_an_unknown_outcome() {
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    let mut dest = Destination::new(&d);
    dest.mode = "unavailable";
    let v = view(&s, &d, &dest);
    let result = apply(&s, &d, &request(&v, "new"), &dest).unwrap();
    assert_eq!(result["state"], "failed");
    assert_eq!(result["reason"], "sessionUnavailable");
    assert_eq!(result["imported"], 0);
}

#[test]
fn client_store_redirection_during_import_cannot_register_in_another_store() {
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    let mut dest = Destination::new(&d);
    dest.mode = "redirect";
    let v = view(&s, &d, &dest);
    let result = apply(&s, &d, &request(&v, "new"), &dest).unwrap();
    assert_eq!(result["state"], "failed");
    assert_eq!(result["reason"], "profileNotSaved");
    assert_eq!(result["rolledBack"], true);
    let current: Value = serde_json::from_slice(&fs::read(&d.path).unwrap()).unwrap();
    assert_eq!(
        current,
        settings(),
        "redirected client store receives no imported profile metadata"
    );
}

#[test]
fn imported_firefox_cookie_is_usable_by_real_qt_http_after_registration() {
    use omarchy_t3_client::{browser_guest, browser_runtime, browser_sessions};
    use sha2::{Digest, Sha256};
    use std::time::Instant;
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    assert!(std::process::Command::new("sqlite3")
        .arg(d.home.join(".mozilla/firefox/profile/cookies.sqlite"))
        .arg("update moz_cookies set host='127.0.0.1',isSecure=0;")
        .status()
        .unwrap()
        .success());
    let config = browser_guest::Config::isolated(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/omarchy-t3-client"),
        &d.root,
    )
    .unwrap();
    let env = s.env.clone();
    let manager = browser_sessions::Manager::start(
        config.clone(),
        move |_| Ok(env.clone()),
        || Ok(json!({})),
        |_| {},
    );
    let v = view(&s, &d, &manager);
    let req = request(&v, "new");
    let result = apply(&s, &d, &req, &manager).unwrap();
    assert_eq!(result["state"], "complete", "{result}");
    manager.shutdown().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut pool = browser_guest::Pool::new(config, move |v| {
        let _ = tx.send(v);
    });
    let page = json!({"threadId":"thread","tabId":"proof","navStatus":{"_tag":"Success","url":format!("{}/proof",s.env.base_url),"title":"Proof"},"canGoBack":false,"canGoForward":false,"updatedAt":"","profileId":req["target"]["profileId"],"runtime":"desktop"});
    let origin = format!("{:x}", Sha256::digest(s.env.base_url.as_bytes()));
    pool.show("fixture", &origin, &page, "proof", 640, 400, &json!(1))
        .unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    while s.proof_requests.lock().unwrap().is_empty() && Instant::now() < end {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(s.proof_requests.lock().unwrap().iter().any(|r|r.contains("cookie0=COOKIE_PRIVATE")),"real HTTP uses source cookie after source read, native write, verified flush and metadata registration");
    assert!(!rx
        .try_iter()
        .any(|v| v.to_string().contains("COOKIE_PRIVATE")));
    let identity =
        browser_runtime::partition("fixture", req["target"]["profileId"].as_str().unwrap())
            .unwrap();
    assert!(d
        .path
        .parent()
        .unwrap()
        .join("ot3-browser-profiles")
        .join(
            identity["partition"]
                .as_str()
                .unwrap()
                .trim_start_matches("persist:")
        )
        .exists());
    drop(pool);
}

#[test]
fn expired_source_cookies_are_skipped_without_touching_target_or_metadata() {
    let _test = TEST.lock().unwrap();
    let s = Server::new();
    let d = Data::new(1);
    assert!(std::process::Command::new("sqlite3")
        .arg(d.home.join(".mozilla/firefox/profile/cookies.sqlite"))
        .arg("update moz_cookies set expiry=1000;")
        .status()
        .unwrap()
        .success());
    let dest = Destination::new(&d);
    let before = fs::read(&d.path).unwrap();
    let v = view(&s, &d, &dest);
    let result = apply(&s, &d, &request(&v, "new"), &dest).unwrap();
    assert_eq!(result["state"], "complete");
    assert_eq!(result["imported"], 0);
    assert_eq!(result["skipped"], 1);
    assert_eq!(result["skippedDomains"], json!(["example.test"]));
    assert!(!dest.events.lock().unwrap().contains(&"write".into()));
    assert_eq!(fs::read(&d.path).unwrap(), before);
}
