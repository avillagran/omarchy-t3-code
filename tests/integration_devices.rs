use omarchy_t3_client::{integration_devices as devices, protocol, Environment};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
fn host(id: &str, target: &str) -> Value {
    json!({"id":id,"label":format!("Host {id}"),"target":target})
}
fn summary() -> Value {
    json!({"id":"local","kind":"local","label":"Local machine","platforms":[{"platform":"android","available":false,"reason":"SDK not installed"}],"hubInstalled":false,"agentDeviceInstalled":false,"privateToken":"PRIVATE","tools":{"hub":{"requiredVersion":"1.2.3","installedVersions":[],"runningVersion":null,"private":"PRIVATE"},"agent":{"requiredVersion":"2.3.4","installedVersions":[],"runningVersion":null}}})
}
fn state() -> Value {
    json!({"supportsHostRetry":true,"supportsToolUpdate":true,"supportsToolInspection":true,"hosts":[summary()],"hostStatus":"disabled","hostStatuses":{},"devices":[],"sessions":[],"onboardingCompleted":false,"agentAccessEnabled":false,"hubBasePath":"/api/device-hub","revision":0,"private":"PRIVATE"})
}
#[test]
fn bounded_native_projection_and_host_destination_mapping_match_original_contract() {
    let projected = devices::present_state(&state()).unwrap();
    assert!(!projected.to_string().contains("PRIVATE"));
    assert_eq!(
        projected["hosts"][0]["tools"]["hub"]["requiredVersion"],
        "1.2.3"
    );
    // Native sessions are references, unlike full inventory/booting summaries.
    let mut populated = state();
    let device = json!({"hostId":"local","id":"emulator-5554","platform":"android","name":"Phone","version":"15","booted":true,"physical":false,"private":"PRIVATE"});
    populated["devices"] = json!([device]);
    populated["sessions"] = json!([{"threadId":"thread-1","hostId":"local","deviceId":"emulator-5554","platform":"android","openedAt":"2026-10-17T00:00:00Z","private":"PRIVATE"}]);
    let mut booting = device.clone();
    booting["threadId"] = json!("thread-2");
    populated["bootingDevices"] = json!([booting]);
    let projected = devices::present_state(&populated).unwrap();
    assert_eq!(projected["devices"][0]["id"], "emulator-5554");
    assert_eq!(projected["sessions"][0]["deviceId"], "emulator-5554");
    assert_eq!(projected["bootingDevices"][0]["threadId"], "thread-2");
    assert!(!projected.to_string().contains("PRIVATE"));
    let mut broken = state();
    broken["hosts"][0]["tools"]["hub"]["runningVersion"] = json!({"secret":"PRIVATE"});
    assert!(devices::present_state(&broken).is_err());
    let original = host("representative", "user@old");
    let renamed = host("representative", "user@new");
    let local = host("remote-local-id", "user@old");
    let unrelated = host("other", "user@unrelated");
    let changed = devices::update_hosts(
        &[local.clone(), unrelated.clone()],
        &renamed,
        false,
        &original,
    )
    .unwrap();
    assert_eq!(changed[0]["id"], "remote-local-id");
    assert_eq!(changed[0]["target"], "user@new");
    assert_eq!(changed[1], unrelated);
    // Native ?? is lazy: an original destination match wins even if the new
    // destination happens to be ambiguous elsewhere on that environment.
    let duplicate = host("duplicate", "user@new");
    let duplicate2 = host("duplicate2", "user@new");
    assert!(devices::update_hosts(
        &[local.clone(), duplicate.clone(), duplicate2.clone()],
        &renamed,
        false,
        &original
    )
    .is_ok());
    assert_eq!(
        devices::update_hosts(&[duplicate, duplicate2], &renamed, false, &original).unwrap_err(),
        "ambiguous_device_host"
    );
    let removed = devices::update_hosts(
        &[local.clone(), unrelated.clone()],
        &original,
        true,
        &original,
    )
    .unwrap();
    assert_eq!(removed, json!([unrelated]));
    let retry = devices::update_hosts(&[changed[0].clone()], &renamed, false, &original).unwrap();
    assert_eq!(retry, json!([changed[0]]));
    for invalid in [
        host("local", "user@host"),
        host("bad/id", "user@host"),
        host("a", "-option"),
        host("a", "user @host"),
    ] {
        assert!(devices::host(&invalid).is_err());
    }
    let with_port = json!({"id":"a","label":"Mac mini","target":"alias","port":2222,"identityFile":"~/.ssh/id_key","password":"PRIVATE"});
    assert_eq!(
        devices::host(&with_port)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        5
    );
}
struct Server {
    environment: Environment,
    stop: Arc<AtomicBool>,
    writable: Arc<AtomicBool>,
    operate: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    settings: Arc<Mutex<Value>>,
    state: Arc<Mutex<Value>>,
    calls: Arc<Mutex<Vec<Value>>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let writable = Arc::new(AtomicBool::new(true));
        let operate = Arc::new(AtomicBool::new(true));
        let lost = Arc::new(AtomicBool::new(false));
        let settings = Arc::new(Mutex::new(
            json!({"enableDeviceSupport":false,"enableAgentDeviceAccess":false,"deviceOnboardingCompleted":false,"deviceHosts":[],"providerSecret":"PRIVATE"}),
        ));
        let state = Arc::new(Mutex::new(state()));
        let calls = Arc::new(Mutex::new(vec![]));
        let (ss, ww, oo, ll, cc, dd, log) = (
            stop.clone(),
            writable.clone(),
            operate.clone(),
            lost.clone(),
            settings.clone(),
            state.clone(),
            calls.clone(),
        );
        let worker = thread::spawn(move || {
            let mut workers = vec![];
            while !ss.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                let (ww, oo, ll, cc, dd, log) = (
                    ww.clone(),
                    oo.clone(),
                    ll.clone(),
                    cc.clone(),
                    dd.clone(),
                    log.clone(),
                );
                let stopped = ss.clone();
                workers.push(thread::spawn(move||{
     socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();let mut peek=[0;512];let n=socket.peek(&mut peek).unwrap();
     if String::from_utf8_lossy(&peek[..n]).starts_with("GET /")&&!String::from_utf8_lossy(&peek[..n]).starts_with("GET /ws"){
      let mut header=vec![];let mut b=[0];while !header.ends_with(b"\r\n\r\n"){socket.read_exact(&mut b).unwrap();header.push(b[0]);}
      let mut permissions=vec!["orchestration:read"];if ww.load(Ordering::Relaxed){permissions.push("settings:write");}if oo.load(Ordering::Relaxed){permissions.push("orchestration:operate");}
      let body=if String::from_utf8_lossy(&peek[..n]).starts_with("GET /api/auth/session "){json!({"authenticated":true,"permissions":permissions})}else{json!({"threads":[],"projects":[]})}.to_string();write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();return
     }
     let mut ws=tungstenite::accept(socket).unwrap();let request:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();log.lock().unwrap().push(request.clone());let payload=&request["payload"];
     let result=match request["tag"].as_str().unwrap(){
      "server.getSettings"=>cc.lock().unwrap().clone(),
      "server.getConfig"=>json!({"settings":cc.lock().unwrap().clone(),"providers":[],"issues":[],"environment":{"capabilities":{}}}),
      "subscribeDeviceState"=>{
       assert_eq!(payload,&json!({}));let current=dd.lock().unwrap().clone();ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":[current]}).to_string().into())).unwrap();
       ws.get_mut().set_read_timeout(Some(Duration::from_millis(30))).unwrap();let mut previous=current;
       loop {
        let next=dd.lock().unwrap().clone();if next!=previous {if ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":[next]}).to_string().into())).is_err(){return}previous=next;}
        match ws.read(){
         Ok(tungstenite::Message::Text(s))=>{let v:Value=serde_json::from_str(&s).unwrap();if v["_tag"]=="Interrupt"{return}if v["_tag"]=="Ping"{let _=ws.send(tungstenite::Message::Text(json!({"_tag":"Pong"}).to_string().into()));}},
         Ok(tungstenite::Message::Ping(bytes))=>{let _=ws.send(tungstenite::Message::Pong(bytes));},
         Err(tungstenite::Error::Io(e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},
         Ok(tungstenite::Message::Close(_))|Err(_)=>return,
         _=>{}
        }
        if stopped.load(Ordering::Relaxed){return}
       }
      },
      "device.configure"=>{
       assert!(ww.load(Ordering::Relaxed));let mut raw=cc.lock().unwrap();let mut state=dd.lock().unwrap();
       for (key,setting) in [("enabled","enableDeviceSupport"),("agentAccessEnabled","enableAgentDeviceAccess"),("onboardingCompleted","deviceOnboardingCompleted")]{if let Some(v)=payload.get(key){raw[setting]=v.clone();}}
       state["hostStatus"]=json!(if raw["enableDeviceSupport"]==true{"idle"}else{"disabled"});state["agentAccessEnabled"]=raw["enableAgentDeviceAccess"].clone();state["onboardingCompleted"]=raw["deviceOnboardingCompleted"].clone();
       let blocked=raw["fixtureBlock"]==true;
       if blocked {raw["fixtureStarted"]=json!(true);state["hostStatus"]=json!("installing");}
       drop(state);drop(raw);
       let start=std::time::Instant::now();
       while blocked&&cc.lock().unwrap()["fixtureBlock"]==true&&!stopped.load(Ordering::Relaxed)&&start.elapsed()<Duration::from_secs(5){thread::sleep(Duration::from_millis(5));}
       if blocked {dd.lock().unwrap()["hostStatus"]=json!("ready");}
       if ll.load(Ordering::Relaxed){return}dd.lock().unwrap().clone()
      },
      "device.list"=>{if payload.get("updateTool").is_some()||payload.get("retryHostId").is_some(){assert!(oo.load(Ordering::Relaxed));}dd.lock().unwrap().clone()},
      "device.testHost"=>{assert!(ww.load(Ordering::Relaxed));let mut h=summary();h["id"]=payload["id"].clone();h["kind"]=json!("ssh");h},
      "server.updateSettings"=>{assert!(ww.load(Ordering::Relaxed));let mut s=cc.lock().unwrap();s["deviceHosts"]=payload["patch"]["deviceHosts"].clone();s.clone()},
      // Background subscriptions the worker opens on its own; this fixture does not model them.
      "orchestration.subscribeShell"|"subscribeServerConfig"=>return,
      method=>panic!("unexpected native method {method}")
     };
     let _=ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}}).to_string().into()));
    }));
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        let environment = Environment {
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
            environment,
            stop,
            writable,
            operate,
            lost,
            settings,
            state,
            calls,
            thread: Some(worker),
        }
    }
    fn mutations(&self) -> Vec<Value> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|v| {
                matches!(
                    v["tag"].as_str(),
                    Some(
                        "device.configure"
                            | "device.list"
                            | "device.testHost"
                            | "server.updateSettings"
                    )
                )
            })
            .cloned()
            .collect()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
#[test]
fn native_state_read_never_lists_installs_or_starts_tools_and_configure_uses_exact_native_payload()
{
    let s = Server::new();
    let mut view = devices::read(&s.environment).unwrap();
    assert!(!view.to_string().contains("PRIVATE"));
    assert!(s.mutations().is_empty());
    assert!(s
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|v| v["tag"] == "subscribeDeviceState"));
    s.writable.store(false, Ordering::Relaxed);
    let input = json!({"viewId":view["viewId"],"action":"hub","enabled":true});
    assert_eq!(
        devices::apply(&s.environment, &input).unwrap_err(),
        "permission_denied"
    );
    assert!(s.mutations().is_empty());
    s.writable.store(true, Ordering::Relaxed);
    let done = devices::apply(&s.environment, &input).unwrap();
    assert_eq!(done["state"], "complete");
    assert_eq!(done["data"]["settings"]["enableDeviceSupport"], true);
    assert_eq!(
        s.mutations()[0]["payload"],
        json!({"enabled":true,"onboardingCompleted":true})
    );
    assert_eq!(
        devices::apply(&s.environment, &input).unwrap_err(),
        "device_view_expired"
    );
    view = done["data"].clone();
    let done = devices::apply(
        &s.environment,
        &json!({"viewId":view["viewId"],"action":"agent","enabled":true}),
    )
    .unwrap();
    assert_eq!(done["state"], "complete");
    assert_eq!(
        s.mutations()[1]["payload"],
        json!({"agentAccessEnabled":true})
    );
    let done = devices::apply(
        &s.environment,
        &json!({"viewId":done["data"]["viewId"],"action":"hub","enabled":false}),
    )
    .unwrap();
    assert_eq!(
        s.mutations()[2]["payload"],
        json!({"enabled":false,"agentAccessEnabled":false})
    );
    assert_eq!(done["data"]["state"]["agentAccessEnabled"], false);
    view = devices::read(&s.environment).unwrap();
    s.settings.lock().unwrap()["deviceHosts"] = json!([host("external", "user@external")]);
    let before = s.mutations().len();
    assert_eq!(
        devices::apply(
            &s.environment,
            &json!({"viewId":view["viewId"],"action":"hub","enabled":true})
        )
        .unwrap_err(),
        "device_settings_changed_reload"
    );
    assert_eq!(s.mutations().len(), before);
    view = devices::read(&s.environment).unwrap();
    let mut moved = s.environment.clone();
    moved.base_url = "http://127.0.0.1:1".into();
    assert_eq!(
        devices::apply(
            &moved,
            &json!({"viewId":view["viewId"],"action":"hub","enabled":true})
        )
        .unwrap_err(),
        "device_scope_changed"
    );
    s.lost.store(true, Ordering::Relaxed);
    let input = json!({"viewId":view["viewId"],"action":"hub","enabled":true});
    let uncertain = devices::apply(&s.environment, &input).unwrap();
    assert_eq!(uncertain["state"], "uncertain");
    assert_eq!(
        devices::apply(&s.environment, &input).unwrap_err(),
        "device_view_expired"
    );
    assert_eq!(
        devices::read(&s.environment).unwrap()["settings"]["enableDeviceSupport"],
        true
    );
}
#[test]
fn capabilities_native_roles_and_hosts_guard_every_action_without_replacing_unrelated_hosts() {
    let s = Server::new();
    s.settings.lock().unwrap()["deviceHosts"] = json!([
        host("environment-id", "user@old"),
        host("keep", "user@keep")
    ]);
    let mut view = devices::read(&s.environment).unwrap();
    let done=devices::apply(&s.environment,&json!({"viewId":view["viewId"],"action":"host-upsert","host":host("representative-id","user@new"),"original":host("representative-id","user@old")})).unwrap();
    assert_eq!(done["state"], "complete");
    assert_eq!(
        s.mutations()[0]["payload"]["patch"]["deviceHosts"][0]["id"],
        "environment-id"
    );
    assert_eq!(
        s.mutations()[0]["payload"]["patch"]["deviceHosts"][1],
        host("keep", "user@keep")
    );
    view = done["data"].clone();
    s.operate.store(false, Ordering::Relaxed);
    assert_eq!(
        devices::apply(
            &s.environment,
            &json!({"viewId":view["viewId"],"action":"update-tool","tool":"hub"})
        )
        .unwrap_err(),
        "permission_denied"
    );
    let inspected = devices::apply(
        &s.environment,
        &json!({"viewId":view["viewId"],"action":"inspect","projectId":"project"}),
    )
    .unwrap();
    assert_eq!(inspected["state"], "complete");
    assert_eq!(s.mutations()[1]["payload"], json!({"inspectOnly":true}));
    view = inspected["data"].clone();
    s.state.lock().unwrap()["supportsToolInspection"] = json!(false);
    assert_eq!(
        devices::apply(
            &s.environment,
            &json!({"viewId":view["viewId"],"action":"inspect"})
        )
        .unwrap_err(),
        "device_inspection_unsupported"
    );
    s.state.lock().unwrap()["supportsToolInspection"] = json!(true);
    assert_eq!(
        devices::apply(
            &s.environment,
            &json!({"viewId":view["viewId"],"action":"hub","enabled":true,"projectId":"project"})
        )
        .unwrap_err(),
        "device_environment_only"
    );
    s.operate.store(true, Ordering::Relaxed);
    let done = devices::apply(
        &s.environment,
        &json!({"viewId":view["viewId"],"action":"update-tool","tool":"agent"}),
    )
    .unwrap();
    assert_eq!(done["state"], "complete");
    assert_eq!(s.mutations()[2]["payload"], json!({"updateTool":"agent"}));
    let done = devices::apply(
        &s.environment,
        &json!({"viewId":done["data"]["viewId"],"action":"retry-host","hostId":"local"}),
    )
    .unwrap();
    assert_eq!(done["state"], "complete");
    assert_eq!(s.mutations()[3]["payload"], json!({"retryHostId":"local"}));
    let tested=devices::apply(&s.environment,&json!({"viewId":done["data"]["viewId"],"action":"test-host","host":host("probe","user@host")})).unwrap();
    assert_eq!(tested["state"], "complete");
    assert_eq!(tested["host"]["kind"], "ssh");
    assert!(!tested.to_string().contains("PRIVATE"));
    s.state.lock().unwrap()["hostStatus"] = json!("installing");
    let before = s.mutations().len();
    assert_eq!(
        devices::apply(
            &s.environment,
            &json!({"viewId":tested["data"]["viewId"],"action":"hub","enabled":false})
        )
        .unwrap_err(),
        "device_busy"
    );
    assert_eq!(s.mutations().len(), before);
}

#[test]
fn slow_native_install_does_not_block_draft_durability_or_device_reads() {
    use std::{
        io::{BufRead, BufReader},
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        sync::mpsc,
        time::Instant,
    };
    let s = Server::new();
    let root = std::env::temp_dir().join(format!("ot3-devices-ipc-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let token = root.join("token");
    std::fs::write(&token, "synthetic").unwrap();
    std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
    let fixture = root.join("environments.json");
    std::fs::write(&fixture,json!({"environments":[{"id":"fixture","name":"Fixture","base_url":s.environment.base_url,"token_file":token,"write_token_file":token}]}).to_string()).unwrap();
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
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if tx.send(v).is_err() {
                    break;
                }
            }
        }
    });
    let mut input = child.0.stdin.take().unwrap();
    let send = |stdin: &mut std::process::ChildStdin, v: Value| {
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap()
    };
    let wait = |id: &str| -> Value {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let v = rx
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .expect("IPC reply deadline");
            if v["data"]["requestId"] == id {
                return v["data"].clone();
            }
        }
    };
    send(
        &mut input,
        json!({"op":"devices-read","environment":"fixture","requestId":"initial"}),
    );
    let read = wait("initial");
    assert!(read["error"].is_null(), "{read}");
    send(&mut input, json!({"op":"visible","value":true}));
    send(
        &mut input,
        json!({"op":"devices-watch","environments":["fixture","fixture"],"requestId":"watch"}),
    );
    let initial = wait("watch");
    assert_eq!(initial["state"]["hostStatus"], "disabled");
    assert_eq!(initial["originKey"], read["result"]["originKey"]);
    assert!(!initial.to_string().contains("PRIVATE"));
    s.settings.lock().unwrap()["fixtureBlock"] = json!(true);
    send(
        &mut input,
        json!({"op":"devices-action","environment":"fixture","requestId":"install","viewId":read["result"]["viewId"],"action":"hub","enabled":true}),
    );
    let until = Instant::now() + Duration::from_secs(3);
    while s.settings.lock().unwrap()["fixtureStarted"] != true {
        assert!(Instant::now() < until, "installation did not start");
        thread::sleep(Duration::from_millis(5));
    }
    send(
        &mut input,
        json!({"op":"draft-save","key":"fixture/prompt","revision":"during-install","draft":{"text":"Preserved while installing","attachments":[]}}),
    );
    send(
        &mut input,
        json!({"op":"devices-read","environment":"fixture","requestId":"during"}),
    );
    let until = Instant::now() + Duration::from_secs(3);
    let (mut saved, mut observed, mut progress) = (false, false, false);
    while !saved || !observed || !progress {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .expect("draft/read blocked behind install");
        if v["type"] == "draft-saved" {
            assert!(v["data"]["error"].is_null(), "{v}");
            assert_eq!(v["data"]["revision"], "during-install");
            saved = true
        }
        if v["data"]["requestId"] == "during" {
            assert!(v["data"]["error"].is_null(), "{v}");
            assert_eq!(v["data"]["result"]["state"]["hostStatus"], "installing");
            observed = true
        }
        if v["type"] == "devices-state"
            && v["data"]["requestId"] == "watch"
            && v["data"]["state"]["hostStatus"] == "installing"
        {
            assert_eq!(v["data"]["originKey"], initial["originKey"]);
            progress = true
        }
        assert_ne!(
            v["data"]["requestId"], "install",
            "installation finished before release"
        );
    }
    s.settings.lock().unwrap()["fixtureBlock"] = json!(false);
    let done = wait("install");
    assert!(done["error"].is_null(), "{done}");
    assert_eq!(done["result"]["state"], "complete");
    send(
        &mut input,
        json!({"op":"draft-load","key":"fixture/prompt"}),
    );
    loop {
        let v = rx.recv_timeout(Duration::from_secs(3)).unwrap();
        if v["type"] == "draft-loaded" {
            assert_eq!(v["data"]["draft"]["text"], "Preserved while installing");
            break;
        }
    }
    assert_eq!(
        s.mutations()
            .iter()
            .filter(|v| v["tag"] == "device.configure")
            .count(),
        1
    );
    send(&mut input, json!({"op":"visible","value":false}));
    thread::sleep(Duration::from_millis(150));
    while rx.try_recv().is_ok() {}
    s.state.lock().unwrap()["revision"] = json!(20);
    let until = Instant::now() + Duration::from_millis(180);
    while let Ok(v) = rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
        assert_ne!(
            v["type"], "devices-state",
            "hidden page retained subscription"
        );
    }
    assert!(!s
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|v| v["tag"] == "device.list"));
    drop(child);
    reader.join().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

/// Native T3 sends optional fields as null once device support is on
/// (captured from a real server after enabling the hub).
#[test]
fn enabled_state_with_null_optional_fields_is_accepted() {
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/device-state-enabled.json")).unwrap()).unwrap();
    assert!(raw["hostStatusDetail"].is_null() && raw["bootingDevices"].is_null());
    let state = devices::present_state(&raw).unwrap();
    assert_eq!(state["hubBasePath"], "/api/device-hub");
    assert!(state.get("hostStatusDetail").is_none() && state.get("bootingDevices").is_none());
    let mut host = raw["hosts"][0].clone();
    host["toolInspectionError"] = Value::Null;
    assert!(devices::present_host(&host).unwrap().get("toolInspectionError").is_none());
}
