use omarchy_t3_client::{browser_profiles as profiles, protocol, Environment};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
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
    capable: Arc<AtomicBool>,
    allowed: Arc<AtomicBool>,
    mode: Arc<Mutex<String>>,
    edit: Arc<Mutex<Option<(PathBuf, Value)>>>,
    calls: Arc<Mutex<Vec<Value>>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(id: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let capable = Arc::new(AtomicBool::new(true));
        let allowed = Arc::new(AtomicBool::new(true));
        let mode = Arc::new(Mutex::new("success".into()));
        let edit = Arc::new(Mutex::new(None::<(PathBuf, Value)>));
        let calls = Arc::new(Mutex::new(vec![]));
        let (s, c, a, m, e, l) = (
            stop.clone(),
            capable.clone(),
            allowed.clone(),
            mode.clone(),
            edit.clone(),
            calls.clone(),
        );
        let worker = thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(err) => panic!("{err}"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut peek = [0; 512];
                let n = socket.peek(&mut peek).unwrap();
                if String::from_utf8_lossy(&peek[..n]).starts_with("GET /")
                    && !String::from_utf8_lossy(&peek[..n]).starts_with("GET /ws")
                {
                    let mut bytes = vec![];
                    let mut b = [0];
                    while !bytes.ends_with(b"\r\n\r\n") {
                        socket.read_exact(&mut b).unwrap();
                        bytes.push(b[0]);
                    }
                    let mut scopes = vec!["orchestration:read"];
                    if a.load(Ordering::Relaxed) {
                        scopes.push("preview:operate");
                    }
                    let body = if String::from_utf8_lossy(&peek[..n])
                        .starts_with("GET /api/auth/session ")
                    {
                        json!({"authenticated":true,"permissions":scopes})
                    } else {
                        json!({"threads":[],"projects":[]})
                    }
                    .to_string();
                    write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                    continue;
                }
                let mut ws = tungstenite::accept(socket).unwrap();
                let request: Value =
                    serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
                l.lock().unwrap().push(request.clone());
                let value = match request["tag"].as_str().unwrap() {
                    "server.getConfig" => {
                        json!({"environment":{"capabilities":{"serverBrowser":c.load(Ordering::Relaxed)}},"settings":{"private":"PRIVATE"},"providers":[{"token":"PRIVATE"}]})
                    }
                    "preview.clearProfile" => {
                        assert!(a.load(Ordering::Relaxed));
                        assert!(c.load(Ordering::Relaxed));
                        assert_eq!(request["payload"].as_object().unwrap().len(), 1);
                        if let Some((path, value)) = e.lock().unwrap().take() {
                            fs::write(path, value.to_string()).unwrap();
                        }
                        if *m.lock().unwrap() == "lost" {
                            continue;
                        }
                        if *m.lock().unwrap() == "rejected" {
                            let _=ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Failure","cause":{"private":"PRIVATE"}}}).to_string().into()));
                            continue;
                        }
                        Value::Null
                    }
                    // Background subscriptions the worker opens on its own.
                    "orchestration.subscribeShell" | "subscribeServerConfig" => continue,
                    method => panic!("Unexpected RPC {method}"),
                };
                let _ = ws.send(tungstenite::Message::Text(
                    json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}})
                        .to_string()
                        .into(),
                ));
            }
        });
        let env = Environment {
            id: id.into(),
            name: id.into(),
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
            capable,
            allowed,
            mode,
            edit,
            calls,
            worker: Some(worker),
        }
    }
    fn clears(&self) -> Vec<Value> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v["tag"] == "preview.clearProfile")
            .cloned()
            .collect()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("ot3-profile-lifecycle-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("client-settings.json");
    fs::write(&path, settings().to_string()).unwrap();
    (root, path)
}
fn settings() -> Value {
    json!({"browserProfiles":[{"id":"work","name":"Work","kind":"persistent"}],"browserDefaultProfileId":"work","future":{"keep":true},"private":"PRIVATE"})
}
fn action(view: &Value, kind: &str) -> Value {
    json!({"action":kind,"profileId":"work","viewId":view["viewId"],"confirmed":true})
}
#[test]
fn native_cleanup_acknowledgements_precede_metadata_removal_and_preserve_concurrent_preferences() {
    let _test = TEST.lock().unwrap();
    let a = Server::new("one");
    let b = Server::new("two");
    let envs = [a.env.clone(), b.env.clone()];
    let (root, path) = fixture();
    let view = profiles::read_at(&envs, &json!({"environments":["one","two"]}), &path).unwrap();
    assert_eq!(view["available"], true);
    assert!(!view.to_string().contains("PRIVATE"));
    let mut changed = settings();
    changed["browserDefaultZoomFactor"] = json!(1.75);
    *a.edit.lock().unwrap() = Some((path.clone(), changed));
    let result = profiles::apply_at(&envs, &action(&view, "remove"), &path).unwrap();
    assert_eq!(result["state"], "complete");
    assert_eq!(result["results"].as_array().unwrap().len(), 2);
    assert!(!result.to_string().contains("PRIVATE"));
    for server in [&a, &b] {
        assert_eq!(server.clears().len(), 1);
        assert_eq!(server.clears()[0]["payload"], json!({"profileId":"work"}));
    }
    let stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored["browserProfiles"], json!([]));
    assert_eq!(stored["browserDefaultProfileId"], "default");
    assert_eq!(stored["browserDefaultZoomFactor"], 1.75);
    assert_eq!(stored["future"]["keep"], true);
    assert_eq!(stored["private"], "PRIVATE");
    assert_eq!(
        profiles::apply_at(&envs, &action(&view, "remove"), &path).unwrap_err(),
        "browser_profile_view_expired"
    );
    assert_eq!(a.clears().len(), 1);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn lost_or_failed_cleanup_preserves_profile_and_never_automatically_replays() {
    let _test = TEST.lock().unwrap();
    for mode in ["lost", "rejected"] {
        let a = Server::new("one");
        let b = Server::new("two");
        *b.mode.lock().unwrap() = mode.into();
        let envs = [a.env.clone(), b.env.clone()];
        let (root, path) = fixture();
        let before = fs::read(&path).unwrap();
        let view = profiles::read_at(&envs, &json!({"environments":["one","two"]}), &path).unwrap();
        let request = action(&view, "remove");
        let result = profiles::apply_at(&envs, &request, &path).unwrap();
        assert_eq!(
            result["state"],
            if mode == "lost" {
                "uncertain"
            } else {
                "failed"
            }
        );
        assert_eq!(result["results"][0]["state"], "complete");
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!result.to_string().contains("PRIVATE"));
        assert_eq!(
            profiles::apply_at(&envs, &request, &path).unwrap_err(),
            "browser_profile_view_expired"
        );
        assert_eq!(a.clears().len(), 1);
        assert_eq!(b.clears().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn fresh_scope_roles_profile_identity_and_builtin_guards_fail_before_dispatch() {
    let _test = TEST.lock().unwrap();
    let a = Server::new("one");
    let b = Server::new("two");
    b.capable.store(false, Ordering::Relaxed);
    let envs = [a.env.clone(), b.env.clone()];
    let (root, path) = fixture();
    let view = profiles::read_at(&envs, &json!({"environments":["one","two"]}), &path).unwrap();
    assert_eq!(
        view["available"], true,
        "native server-only cleanup needs at least one capable environment"
    );
    let other = root.join("another-client.json");
    fs::write(&other, settings().to_string()).unwrap();
    assert_eq!(
        profiles::apply_at(&envs, &action(&view, "remove"), &other).unwrap_err(),
        "browser_profile_scope_changed"
    );
    let mut request = action(&view, "remove");
    request["confirmed"] = json!(false);
    assert_eq!(
        profiles::apply_at(&envs, &request, &path).unwrap_err(),
        "confirmation_required"
    );
    request = action(&view, "remove");
    request["profileId"] = json!("default");
    assert_eq!(
        profiles::apply_at(&envs, &request, &path).unwrap_err(),
        "browser_profile_builtin"
    );
    a.allowed.store(false, Ordering::Relaxed);
    assert_eq!(
        profiles::apply_at(&envs, &action(&view, "remove"), &path).unwrap_err(),
        "permission_denied"
    );
    a.allowed.store(true, Ordering::Relaxed);
    let mut moved = envs.clone();
    moved[0].base_url = "http://127.0.0.1:1".into();
    assert_eq!(
        profiles::apply_at(&moved, &action(&view, "remove"), &path).unwrap_err(),
        "browser_profile_scope_changed"
    );
    let mut changed = settings();
    changed["browserProfiles"][0]["name"] = json!("Renamed");
    fs::write(&path, changed.to_string()).unwrap();
    assert_eq!(
        profiles::apply_at(&envs, &action(&view, "remove"), &path).unwrap_err(),
        "browser_profile_changed_reload"
    );
    assert!(a.clears().is_empty());
    fs::write(&path, settings().to_string()).unwrap();
    let result = profiles::apply_at(&envs, &action(&view, "clear"), &path).unwrap();
    assert_eq!(result["state"], "complete");
    assert_eq!(result["results"][1]["state"], "skipped");
    assert!(b.clears().is_empty());
    let stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored, settings(), "clear retains profile metadata");
    let view = profiles::read_at(&envs, &json!({"environments":["one","two"]}), &path).unwrap();
    a.capable.store(false, Ordering::Relaxed);
    assert_eq!(
        profiles::apply_at(&envs, &action(&view, "remove"), &path).unwrap_err(),
        "browser_profile_scope_changed"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn rename_during_native_cleanup_preserves_metadata_and_reports_uncertain_completion() {
    let _test = TEST.lock().unwrap();
    let a = Server::new("one");
    let (root, path) = fixture();
    let view =
        profiles::read_at(&[a.env.clone()], &json!({"environments":["one"]}), &path).unwrap();
    let mut changed = settings();
    changed["browserProfiles"][0]["name"] = json!("Changed while clearing");
    *a.edit.lock().unwrap() = Some((path.clone(), changed.clone()));
    let result = profiles::apply_at(&[a.env.clone()], &action(&view, "remove"), &path).unwrap();
    assert_eq!(result["state"], "uncertain");
    assert_eq!(result["error"], "browser_profile_changed_reload");
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        changed
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn actual_worker_routes_global_profile_cleanup_to_native_api_and_updates_only_isolated_client_file()
{
    use std::{
        io::{BufRead, BufReader},
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        sync::mpsc,
        time::Instant,
    };
    let _test = TEST.lock().unwrap();
    let server = Server::new("one");
    let (root, _) = fixture();
    let native = root.join("native/userdata");
    fs::create_dir_all(&native).unwrap();
    let path = native.join("client-settings.json");
    fs::write(&path, settings().to_string()).unwrap();
    let token = root.join("token");
    fs::write(&token, "synthetic").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let fixture = root.join("environments.json");
    fs::write(&fixture,json!({"environments":[{"id":"one","name":"One","base_url":server.env.base_url,"token_file":token,"write_token_file":token}]}).to_string()).unwrap();
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Child(
        Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
            .arg("watch")
            .env("HOME", root.join("home"))
            .env("T3CODE_HOME", root.join("native"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("OMARCHY_T3_FIXTURE_ENVIRONMENTS", fixture)
            .env_remove("KIMI_CODING_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if tx.send(v).is_err() {
                    return;
                }
            }
        }
    });
    let mut stdin = child.0.stdin.take().unwrap();
    let send = |stdin: &mut std::process::ChildStdin, v: Value| {
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap()
    };
    let wait = |id: &str| -> Value {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let v = rx
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .expect("native profile IPC deadline");
            if v["type"] == "action" && v["data"]["requestId"] == id {
                return v["data"].clone();
            }
        }
    };
    send(
        &mut stdin,
        json!({"op":"browser-profiles-read","requestId":"read","environments":["one"]}),
    );
    let view = wait("read");
    assert!(view["error"].is_null(), "{view}");
    assert_eq!(view["result"]["available"], true);
    assert!(!view.to_string().contains("PRIVATE"));
    let request = json!({"op":"browser-profiles-action","requestId":"remove","action":"remove","viewId":view["result"]["viewId"],"profileId":"work","confirmed":true});
    send(&mut stdin, request.clone());
    let done = wait("remove");
    assert!(done["error"].is_null(), "{done}");
    assert_eq!(done["result"]["state"], "complete");
    assert_eq!(done["result"]["data"]["browserProfiles"], json!([]));
    let stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored["browserProfiles"], json!([]));
    assert_eq!(stored["browserDefaultProfileId"], "default");
    assert_eq!(stored["private"], "PRIVATE");
    assert_eq!(stored["future"]["keep"], true);
    assert_eq!(server.clears().len(), 1);
    let mut replay = request;
    replay["requestId"] = json!("replay");
    send(&mut stdin, replay);
    let rejected = wait("replay");
    assert_eq!(rejected["error"], "browser_profile_view_expired");
    assert_eq!(server.clears().len(), 1);
    drop(child);
    reader.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_qt_cleanup_includes_non_server_environments_and_retains_metadata_on_partial_failure() {
    use omarchy_t3_client::{
        browser_cleanup::{Lease, Runtime, Scope},
        Result,
    };
    use std::path::Path;
    struct Local {
        events: Arc<Mutex<Vec<String>>>,
        fail: bool,
    }
    struct Held {
        events: Arc<Mutex<Vec<String>>>,
        fail: bool,
        path: PathBuf,
    }
    impl Runtime for Local {
        fn available(&self) -> bool {
            true
        }
        fn begin(&self, profile: &str, path: &Path, scopes: &[Scope]) -> Result<Box<dyn Lease>> {
            assert_eq!(profile, "work");
            assert_eq!(scopes.len(), 2);
            assert!(scopes.iter().all(|s| s.origin_key.len() == 64));
            self.events.lock().unwrap().push("begin".into());
            Ok(Box::new(Held {
                events: self.events.clone(),
                fail: self.fail,
                path: path.into(),
            }))
        }
    }
    impl Lease for Held {
        fn clear(&mut self, environment: &str) -> Result<()> {
            let settings: Value = serde_json::from_slice(&fs::read(&self.path).unwrap()).unwrap();
            assert_eq!(
                settings["browserProfiles"][0]["id"], "work",
                "metadata must remain while local cleanup runs"
            );
            self.events.lock().unwrap().push(environment.into());
            if self.fail && environment == "two" {
                Err("browser_profile_flush_unknown".into())
            } else {
                Ok(())
            }
        }
    }
    impl Drop for Held {
        fn drop(&mut self) {
            self.events.lock().unwrap().push("released".into());
        }
    }
    let _test = TEST.lock().unwrap();
    for fail in [false, true] {
        let a = Server::new("one");
        let b = Server::new("two");
        b.capable.store(false, Ordering::Relaxed);
        b.allowed.store(false, Ordering::Relaxed);
        let envs = [a.env.clone(), b.env.clone()];
        let (root, path) = fixture();
        let local = Local {
            events: Arc::new(Mutex::new(vec![])),
            fail,
        };
        let view = profiles::read_with_at(
            &envs,
            &json!({"environments":["one","two"]}),
            &path,
            Some(&local),
        )
        .unwrap();
        assert_eq!(view["available"], true);
        let result =
            profiles::apply_with_at(&envs, &action(&view, "remove"), &path, Some(&local)).unwrap();
        assert_eq!(result["state"], if fail { "uncertain" } else { "complete" });
        assert_eq!(
            result["results"][1]["local"]["state"],
            if fail { "uncertain" } else { "complete" }
        );
        assert_eq!(result["results"][1]["server"]["state"], "skipped");
        assert_eq!(
            *local.events.lock().unwrap(),
            vec!["begin", "one", "two", "released"]
        );
        assert_eq!(a.clears().len(), 1);
        assert_eq!(b.clears().len(), 0);
        let stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            stored["browserProfiles"].as_array().unwrap().len(),
            if fail { 1 } else { 0 }
        );
        assert_eq!(
            profiles::apply_with_at(&envs, &action(&view, "remove"), &path, Some(&local))
                .unwrap_err(),
            "browser_profile_view_expired"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn coordinator_clears_cold_local_partition_and_requires_matching_store_and_origin() {
    use omarchy_t3_client::{
        browser_cleanup::{Runtime, Scope},
        browser_guest, browser_runtime, browser_sessions,
    };
    use sha2::{Digest, Sha256};
    let _test = TEST.lock().unwrap();
    let server = Server::new("one");
    server.capable.store(false, Ordering::Relaxed);
    server.allowed.store(false, Ordering::Relaxed);
    let (root, _) = fixture();
    let config = browser_guest::Config::isolated(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/omarchy-t3-client"),
        &root,
    )
    .unwrap();
    let path = config.client_settings_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, settings().to_string()).unwrap();
    let storage = config.storage_root();
    let identity = browser_runtime::partition("one", "work").unwrap();
    let partition = storage.join(
        identity["partition"]
            .as_str()
            .unwrap()
            .trim_start_matches("persist:"),
    );
    fs::create_dir_all(&partition).unwrap();
    fs::write(partition.join("Cookies"), "synthetic cookie").unwrap();
    let other = storage.join("unrelated");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("keep"), "keep").unwrap();
    let env = server.env.clone();
    let resolved = env.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let manager = browser_sessions::Manager::start(
        config,
        move |id| {
            if id == resolved.id {
                Ok(resolved.clone())
            } else {
                Err("unknown_environment".into())
            }
        },
        || Ok(json!({})),
        move |v| {
            let _ = tx.send(v);
        },
    );
    let scopes = [Scope {
        environment: "one".into(),
        origin_key: format!("{:x}", Sha256::digest(env.base_url.as_bytes())),
    }];
    assert!(
        matches!(manager.begin("work",&root.join("wrong-store"),&scopes),Err(e) if e=="browser_profile_scope_changed")
    );
    let bad = [Scope {
        environment: "one".into(),
        origin_key: "wrong".into(),
    }];
    assert!(
        matches!(manager.begin("work",&path,&bad),Err(e) if e=="browser_profile_scope_changed")
    );
    let view = profiles::read_with_at(
        &[env.clone()],
        &json!({"environments":["one"]}),
        &path,
        Some(&manager),
    )
    .unwrap();
    assert_eq!(
        view["available"], true,
        "desktop-only environment supports local clearing without server preview role"
    );
    let result =
        profiles::apply_with_at(&[env], &action(&view, "remove"), &path, Some(&manager)).unwrap();
    assert_eq!(result["state"], "complete");
    assert!(!partition.exists());
    assert!(other.join("keep").exists());
    let started = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(started["kind"], "cleanup-started");
    let ended = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(ended["kind"], "cleanup-ended");
    assert_eq!(ended["cleared"], json!(["one"]));
    assert_eq!(server.clears().len(), 0);
    manager.shutdown().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn import_reservation_flushes_holds_exclusive_partition_and_rolls_back_new_target_storage() {
    use omarchy_t3_client::{
        browser_cleanup::{Runtime, Scope},
        browser_cookies::ImportedCookie,
        browser_guest, browser_runtime, browser_sessions,
    };
    use sha2::{Digest, Sha256};
    let _test = TEST.lock().unwrap();
    let server = Server::new("one");
    let (root, _) = fixture();
    let config = browser_guest::Config::isolated(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/omarchy-t3-client"),
        &root,
    )
    .unwrap();
    let path = config.client_settings_path();
    let storage = config.storage_root();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, settings().to_string()).unwrap();
    let before = fs::read(&path).unwrap();
    let env = server.env.clone();
    let origin = format!("{:x}", Sha256::digest(env.base_url.as_bytes()));
    let resolved = env.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let manager = browser_sessions::Manager::start(
        config,
        move |id| {
            if id == resolved.id {
                Ok(resolved.clone())
            } else {
                Err("unknown_environment".into())
            }
        },
        || Ok(json!({})),
        move |v| {
            let _ = tx.send(v);
        },
    );
    let scopes = [Scope {
        environment: "one".into(),
        origin_key: origin,
    }];
    let mut lease = manager.begin("import-new", &path, &scopes).unwrap();
    let cookie = || ImportedCookie {
        url: format!("{}/", env.base_url),
        name: "lease-cookie".into(),
        value: "LEASE_PRIVATE".into(),
        domain: None,
        path: "/".into(),
        secure: false,
        http_only: true,
        expiration_date: Some(
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 3600) as f64,
        ),
        same_site: "strict",
    };
    assert_eq!(
        lease
            .write_cookie("wrong-environment", cookie())
            .unwrap_err(),
        "browser_profile_scope_changed"
    );
    assert!(lease.write_cookie("one", cookie()).unwrap());
    assert!(
        matches!(browser_runtime::lock_profile(&storage,"one","import-new"),Err(e) if e=="browser_profile_busy"),
        "Qt renderer holds process lock during import"
    );
    lease.flush_import("one").unwrap();
    assert!(
        matches!(browser_runtime::lock_profile(&storage,"one","import-new"),Err(e) if e=="browser_profile_busy"),
        "reservation retains lock after Qt flush through metadata registration"
    );
    assert_eq!(
        lease.write_cookie("one", cookie()).unwrap_err(),
        "browser_profile_scope_changed",
        "no extra writes after final flush"
    );
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "destination layer does not create metadata before caller acknowledges success"
    );
    let identity = browser_runtime::partition("one", "import-new").unwrap();
    let partition = storage.join(
        identity["partition"]
            .as_str()
            .unwrap()
            .trim_start_matches("persist:"),
    );
    assert!(partition.join("storage/Cookies").exists());
    lease.clear("one").unwrap();
    assert!(
        !partition.exists(),
        "registration failure can clear new target with same held lease/lock"
    );
    drop(lease);
    let mut ended = false;
    while let Ok(v) = rx.recv_timeout(Duration::from_secs(2)) {
        assert!(!v.to_string().contains("LEASE_PRIVATE"));
        if v["kind"] == "cleanup-ended" {
            assert_eq!(v["cleared"], json!(["one"]));
            ended = true;
            break;
        }
    }
    assert!(ended);
    assert!(browser_runtime::lock_profile(&storage, "one", "import-new").is_ok());
    manager.shutdown().unwrap();
    fs::remove_dir_all(root).unwrap();
}
