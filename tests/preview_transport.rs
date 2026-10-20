use omarchy_t3_client::{preview_transport as preview, protocol, Environment};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

fn snapshot(tab: &str) -> Value {
    json!({"threadId":"thread","tabId":tab,"navStatus":{"_tag":"Success","url":"https://example.test/","title":"Example","private":"PRIVATE"},"canGoBack":false,"canGoForward":true,"updatedAt":"2026-10-17T00:00:00Z","runtime":"desktop","profileId":"default","viewport":{"_tag":"fill"},"private":"PRIVATE"})
}
fn opened(epoch: &str, revision: u64, tab: &str) -> Value {
    json!({"type":"opened","threadId":"thread","tabId":tab,"snapshot":snapshot(tab),"createdAt":"2026-10-17T00:00:00Z","serverEpoch":epoch,"revision":revision,"private":"PRIVATE"})
}
#[test]
fn native_metadata_projection_preserves_runtime_reveal_and_failure_without_unknown_data() {
    let mut s = snapshot("a");
    s["runtime"] = json!("server");
    s["automationOwner"] = json!("session");
    s["reveal"] = json!(true);
    s["revealRequest"] = json!({"id":"reveal1","force":true,"private":"PRIVATE"});
    s["viewport"] = json!({"_tag":"preset","presetId":"pixel-8","width":412,"height":915});
    s["colorScheme"] = json!("dark");
    s["zoomFactor"] = json!(1.25);
    let out = preview::snapshot(&s).unwrap();
    assert_eq!(out["revealRequest"], json!({"id":"reveal1","force":true}));
    assert_eq!(out["viewport"]["presetId"], "pixel-8");
    assert!(!out.to_string().contains("PRIVATE"));
    let failed = json!({"type":"failed","threadId":"thread","tabId":"a","createdAt":"now","serverEpoch":"one","revision":1,"url":"https://example.test/file","title":"File","code":-2,"description":"Download","download":{"id":"id","fileName":"f.pdf","private":"PRIVATE"}});
    assert_eq!(
        preview::event(&failed).unwrap()["download"],
        json!({"id":"id","fileName":"f.pdf"})
    );
    let mut integral_json = failed.clone();
    integral_json["code"] = json!(-2.0);
    integral_json["revision"] = json!(1.0);
    assert_eq!(preview::event(&integral_json).unwrap()["revision"], 1);
    integral_json["revision"] = json!(1.5);
    assert!(preview::event(&integral_json).is_err());
    for patch in [
        json!({"runtime":"unknown"}),
        json!({"zoomFactor":1.2}),
        json!({"profileId":"bad\nprofile"}),
        json!({"viewport":{"_tag":"freeform","width":3840,"height":3840}}),
        json!({"title":"unused","navStatus":{"_tag":"Success","url":"https://example.test","title":"😀".repeat(257)}}),
    ] {
        let mut invalid = s.clone();
        for (k, v) in patch.as_object().unwrap() {
            invalid[k] = v.clone();
        }
        assert!(preview::snapshot(&invalid).is_err());
    }
    let mut mismatch = opened("one", 1, "a");
    mismatch["snapshot"]["threadId"] = json!("other");
    assert!(preview::event(&mismatch).is_err());
    assert!(preview::list(
        &json!({"serverEpoch":"one","revision":0,"sessions":[snapshot("a"),snapshot("a")]}),
        "thread"
    )
    .is_err());
    assert!(preview::list(
        &json!({"serverEpoch":"one","revision":0,"sessions":[snapshot("a")]}),
        "other"
    )
    .is_err());
    let mut invalid = opened("one", 0, "a");
    assert!(preview::event(&invalid).is_err());
    invalid["revision"] = json!(9_007_199_254_740_992u64);
    assert!(preview::event(&invalid).is_err());
}

struct Server {
    env: Environment,
    stop: Arc<AtomicBool>,
    allow: Arc<AtomicBool>,
    operate: Arc<AtomicBool>,
    calls: Arc<Mutex<Vec<Value>>>,
    interrupts: Arc<AtomicUsize>,
    mode: Arc<Mutex<String>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(events: Vec<Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let allow = Arc::new(AtomicBool::new(true));
        let operate = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(Mutex::new(vec![]));
        let lists = Arc::new(AtomicUsize::new(0));
        let interrupts = Arc::new(AtomicUsize::new(0));
        let mode = Arc::new(Mutex::new(String::new()));
        let mode_worker = mode.clone();
        let (ss, aa, oo, cc, ii) = (
            stop.clone(),
            allow.clone(),
            operate.clone(),
            calls.clone(),
            interrupts.clone(),
        );
        let worker = thread::spawn(move || {
            let mut workers = vec![];
            while !ss.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                let (ss, aa, oo, cc, lists, events, ii, mode) = (
                    ss.clone(),
                    aa.clone(),
                    oo.clone(),
                    cc.clone(),
                    lists.clone(),
                    events.clone(),
                    ii.clone(),
                    mode_worker.clone(),
                );
                workers.push(thread::spawn(move||{
          socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();let mut peek=[0;512];let n=socket.peek(&mut peek).unwrap();
          if String::from_utf8_lossy(&peek[..n]).starts_with("GET /") && !String::from_utf8_lossy(&peek[..n]).starts_with("GET /ws") {
            let mut header=vec![];let mut b=[0];while !header.ends_with(b"\r\n\r\n"){socket.read_exact(&mut b).unwrap();header.push(b[0]);}
            let mut scopes=vec![];if aa.load(Ordering::Relaxed){scopes.push("orchestration:read");}if oo.load(Ordering::Relaxed){scopes.push("preview:operate");}
            let http=String::from_utf8_lossy(&header).lines().next().unwrap_or("").to_owned();let page=http.starts_with("GET /page ")||http.starts_with("GET /next ")||http.starts_with("GET /clicked ");
            let body=if page{cc.lock().unwrap().push(json!({"http":http}));"<!doctype html><title>Fixture</title><style>body{margin:0;background:#203040}button{width:160px;height:80px}</style><button onclick=\"fetch('/clicked');document.body.style.background='green'\">Click</button>".to_string()}else if http.starts_with("GET /api/auth/session "){json!({"authenticated":true,"permissions":scopes,"private":"PRIVATE"}).to_string()}else{json!({"threads":[],"projects":[]}).to_string()};write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",if page{"text/html"}else{"application/json"},body.len(),body).unwrap();return;
          }
          let mut ws=tungstenite::accept(socket).unwrap();let request:Value=serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();cc.lock().unwrap().push(request.clone());
          let fixture_mode=mode.lock().unwrap().clone();
          let value=match request["tag"].as_str().unwrap(){
            "server.getConfig"=>json!({"environment":{"capabilities":{"serverBrowser":fixture_mode!="no-server"}},"settings":{"private":"PRIVATE"}}),
            "preview.list"=>{assert_eq!(request["payload"],json!({"threadId":"thread"}));let n=lists.fetch_add(1,Ordering::Relaxed);let mut s=snapshot("a");if fixture_mode.starts_with("http:"){s["navStatus"]["url"]=json!(fixture_mode);}if fixture_mode=="server-tab"{s["runtime"]=json!("server");}if fixture_mode=="profile-less"{s.as_object_mut().unwrap().remove("profileId");}let native=s;json!({"serverEpoch":if n==0||!fixture_mode.is_empty(){"one"}else{"two"},"revision":if n==0{4}else{0},"sessions":[native],"private":"PRIVATE"})},
            "preview.open"|"preview.navigate"|"preview.resize"|"preview.adjust"|"preview.refresh"|"preview.close"=>{
              assert!(oo.load(Ordering::Relaxed));if fixture_mode=="blocked"{while *mode.lock().unwrap()=="blocked"&&!ss.load(Ordering::Relaxed){thread::sleep(Duration::from_millis(5));}}if fixture_mode=="lost"{return;}if matches!(request["tag"].as_str(),Some("preview.refresh"|"preview.close")){Value::Null}else{let mut s=snapshot(if request["tag"]=="preview.open"{"created"}else{"a"});if request["tag"]=="preview.navigate"{s["navStatus"]["url"]=request["payload"]["url"].clone();}if fixture_mode=="malformed"{s["threadId"]=json!("other");}s}
            },
            "preview.reportStatus"=>{assert!(oo.load(Ordering::Relaxed));Value::Null},
            "subscribePreviewEvents"=>{
              assert_eq!(request["payload"],json!({}));if !events.is_empty(){ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":events}).to_string().into())).unwrap();}
              ws.get_mut().set_read_timeout(Some(Duration::from_millis(30))).unwrap();while !ss.load(Ordering::Relaxed){match ws.read(){Ok(tungstenite::Message::Text(t))=>{let v:Value=serde_json::from_str(&t).unwrap();if v["_tag"]=="Interrupt"{ii.fetch_add(1,Ordering::Relaxed);return}},Err(tungstenite::Error::Io(e)) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{},Err(_)|Ok(tungstenite::Message::Close(_))=>return,_=>{}}}return;
            },"orchestration.subscribeShell"|"subscribeServerConfig"=>return,other=>panic!("unexpected native RPC: {other}")};
          ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}}).to_string().into())).unwrap();
        }));
            }
            for w in workers {
                w.join().unwrap();
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
            allow,
            operate,
            calls,
            interrupts,
            mode,
            worker: Some(worker),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}

#[test]
fn read_only_scope_can_observe_but_never_starts_a_runtime_or_reports_profiles() {
    let s = Server::new(vec![]);
    let result = preview::read(&s.env, "thread").unwrap();
    assert_eq!(result["canOperate"], false);
    assert_eq!(result["serverBrowser"], true);
    assert_eq!(result["kind"], "list");
    assert!(!result.to_string().contains("PRIVATE"));
    assert_eq!(
        s.calls
            .lock()
            .unwrap()
            .iter()
            .map(|r| r["tag"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        vec!["server.getConfig", "preview.list"]
    );
    s.allow.store(false, Ordering::Relaxed);
    s.operate.store(true, Ordering::Relaxed);
    assert_eq!(
        preview::read(&s.env, "thread").unwrap_err(),
        "permission_denied"
    );
    assert_eq!(s.calls.lock().unwrap().len(), 2);
}
#[test]
fn subscribed_events_are_scoped_and_epoch_changes_request_a_fresh_native_list() {
    let mut foreign = opened("one", 99, "foreign");
    foreign["threadId"] = json!("other");
    foreign["snapshot"]["threadId"] = json!("other");
    let closed = json!({"type":"closed","threadId":"thread","tabId":"a","serverEpoch":"one","revision":5,"createdAt":"now"});
    let s = Server::new(vec![
        closed,
        foreign,
        opened("one", 3, "a"),
        opened("two", 1, "new"),
    ]);
    let done = AtomicBool::new(false);
    let mut messages = vec![];
    preview::observe(
        &s.env,
        "thread",
        || done.load(Ordering::Relaxed),
        |v| {
            messages.push(v);
            if messages.len() == 5 {
                done.store(true, Ordering::Relaxed);
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        messages
            .iter()
            .map(|m| m["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["list", "event", "event", "list", "event"]
    );
    assert_eq!(messages[0]["result"]["revision"], 4);
    assert_eq!(messages[1]["result"]["revision"], 5);
    assert_eq!(messages[2]["result"]["revision"], 3);
    assert_eq!(messages[3]["result"]["serverEpoch"], "two");
    assert_eq!(messages[3]["result"]["revision"], 0);
    assert!(messages.iter().all(|m| m["thread"] == "thread"
        && m["environment"] == "fixture"
        && m["canOperate"] == false));
    assert!(!json!(messages).to_string().contains("PRIVATE"));
    assert_eq!(
        s.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "preview.list")
            .count(),
        2
    );
}
#[test]
fn quiet_stream_loads_authoritative_list_and_cancels_without_waiting_for_an_event() {
    let s = Server::new(vec![]);
    let done = AtomicBool::new(false);
    let start = Instant::now();
    let mut count = 0;
    preview::observe(
        &s.env,
        "thread",
        || done.load(Ordering::Relaxed),
        |v| {
            assert_eq!(v["kind"], "list");
            count += 1;
            done.store(true, Ordering::Relaxed);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(count, 1);
    assert!(start.elapsed() < Duration::from_secs(2));
}

fn action_input(server: &Server, action: &str) -> Value {
    *server.mode.lock().unwrap() = "stable".into();
    let read = preview::read(&server.env, "thread").unwrap();
    json!({"action":action,"environment":"fixture","thread":"thread","tabId":"a","requestId":uuid::Uuid::new_v4().to_string(),"originKey":read["originKey"],"serverEpoch":"one"})
}
#[test]
fn native_lifecycle_payloads_and_fresh_scope_checks_preserve_exact_rpc_contracts() {
    use omarchy_t3_client::preview_actions as actions;
    let server = Server::new(vec![]);
    let mut input = action_input(&server, "navigate");
    input["url"] = json!("localhost:5173/path");
    input["providerToken"] = json!("PRIVATE");
    input["automationOwner"] = json!("forged");
    assert_eq!(
        actions::execute(&server.env, &input).unwrap_err(),
        "permission_denied"
    );
    server.operate.store(true, Ordering::Relaxed);
    let result = actions::execute(&server.env, &input).unwrap();
    assert_eq!(result["state"], "complete");
    assert!(!result.to_string().contains("PRIVATE"));
    let mutation = server
        .calls
        .lock()
        .unwrap()
        .iter()
        .find(|c| c["tag"] == "preview.navigate")
        .unwrap()
        .clone();
    assert_eq!(
        mutation["payload"],
        json!({"threadId":"thread","tabId":"a","url":"localhost:5173/path"})
    );
    assert_eq!(
        actions::execute(&server.env, &input).unwrap_err(),
        "preview_command_replayed"
    );
    for (field, value, error) in [
        ("originKey", json!("different"), "preview_scope_changed"),
        ("serverEpoch", json!("old"), "preview_server_restarted"),
        ("tabId", json!("closed"), "preview_tab_closed"),
    ] {
        let mut stale = input.clone();
        stale[field] = value;
        stale["requestId"] = json!(uuid::Uuid::new_v4().to_string());
        assert_eq!(actions::execute(&server.env, &stale).unwrap_err(), error);
    }
    for action in ["open", "resize", "adjust", "refresh", "close"] {
        let mut input = action_input(&server, action);
        input["viewport"] =
            json!({"_tag":"preset","presetId":"iphone-se","width":375,"height":667});
        input["colorScheme"] = json!("dark");
        let result = actions::execute(&server.env, &input).unwrap();
        assert_eq!(result["state"], "complete", "{action}");
        if matches!(action, "refresh" | "close") {
            assert!(result["snapshot"].is_null());
        }
    }
    let mut open = action_input(&server, "open");
    open["runtime"] = json!("server");
    *server.mode.lock().unwrap() = "no-server".into();
    assert_eq!(
        actions::execute(&server.env, &open).unwrap_err(),
        "server_browser_unavailable"
    );
    assert_eq!(
        actions::payload(
            &json!({"action":"open","thread":"thread","automationOwner":"forged","tabId":"ignored"})
        )
        .unwrap()
        .1,
        json!({"threadId":"thread"})
    );
    for invalid in [
        json!({"action":"open","thread":"thread","address":"a ".repeat(600)}),
        json!({"action":"adjust","thread":"thread","tabId":"a"}),
        json!({"action":"adjust","thread":"thread","tabId":"a","zoomFactor":1.2}),
        json!({"action":"resize","thread":"thread","tabId":"a","viewport":{"_tag":"freeform","width":3840,"height":3840}}),
        json!({"action":"evaluate","thread":"thread","tabId":"a"}),
    ] {
        assert!(actions::payload(&invalid).is_err());
    }
}
#[test]
fn lost_or_invalid_native_mutation_reply_is_uncertain_and_cannot_be_replayed() {
    use omarchy_t3_client::preview_actions as actions;
    let server = Server::new(vec![]);
    server.operate.store(true, Ordering::Relaxed);
    for mode in ["lost", "malformed"] {
        let input = action_input(&server, "open");
        *server.mode.lock().unwrap() = mode.into();
        let result = actions::execute(&server.env, &input).unwrap();
        assert_eq!(result["state"], "uncertain");
        assert_eq!(result["error"], "preview_command_uncertain");
        assert_eq!(
            actions::execute(&server.env, &input).unwrap_err(),
            "preview_command_replayed"
        );
    }
    assert_eq!(
        server
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c["tag"] == "preview.open")
            .count(),
        2
    );
}

#[test]
fn local_surface_and_status_use_current_native_identity_and_exact_operate_scope() {
    let server = Server::new(vec![]);
    let input = action_input(&server, "open");
    let mut selection = input.clone();
    selection["url"] = json!("file:///forged");
    selection["profileId"] = json!("forged");
    assert_eq!(
        preview::surface(&server.env, &selection).unwrap_err(),
        "permission_denied"
    );
    server.operate.store(true, Ordering::Relaxed);
    let view = preview::surface(&server.env, &selection).unwrap();
    assert_eq!(
        view["snapshot"]["navStatus"]["url"],
        "https://example.test/"
    );
    assert_eq!(view["snapshot"]["profileId"], "default");
    let mut scope = view.clone();
    scope["profileId"] = json!("default");
    let mut status = snapshot("a");
    status["navStatus"] =
        json!({"_tag":"Success","url":"https://example.test/actual","title":"Actual"});
    status["frameDirectory"] = json!("PRIVATE");
    preview::report_status(&server.env, &scope, &status).unwrap();
    let call = server
        .calls
        .lock()
        .unwrap()
        .iter()
        .find(|r| r["tag"] == "preview.reportStatus")
        .unwrap()
        .clone();
    assert_eq!(call["payload"].as_object().unwrap().len(), 5);
    assert!(!call.to_string().contains("PRIVATE"));
    assert_eq!(call["payload"]["navStatus"]["title"], "Actual");
    let mut wrong = status.clone();
    wrong["profileId"] = json!("other");
    assert_eq!(
        preview::report_status(&server.env, &scope, &wrong).unwrap_err(),
        "preview_scope_changed"
    );
    *server.mode.lock().unwrap() = "server-tab".into();
    assert_eq!(
        preview::surface(&server.env, &selection).unwrap_err(),
        "server_browser_surface_required"
    );
    assert_eq!(
        preview::report_status(&server.env, &scope, &status).unwrap_err(),
        "server_browser_surface_required"
    );
    *server.mode.lock().unwrap() = "profile-less".into();
    scope["profileId"] = json!("work");
    status["profileId"] = json!("work");
    preview::report_status(&server.env, &scope, &status).unwrap();
    server.operate.store(false, Ordering::Relaxed);
    assert_eq!(
        preview::report_status(&server.env, &scope, &status).unwrap_err(),
        "permission_denied"
    );
    assert_eq!(
        server
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "preview.reportStatus")
            .count(),
        2
    );
}

#[test]
fn authorized_coordinator_runs_real_qt_reports_native_status_and_applies_admitted_actions() {
    use omarchy_t3_client::{browser_guest, browser_sessions, preview_actions};
    use std::{fs, sync::mpsc};
    let server = Server::new(vec![]);
    server.operate.store(true, Ordering::Relaxed);
    *server.mode.lock().unwrap() = format!("{}/page", server.env.base_url);
    let read = preview::read(&server.env, "thread").unwrap();
    let root = std::env::temp_dir().join(format!("ot3-session-{}", uuid::Uuid::new_v4()));
    let config = browser_guest::Config::isolated(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/omarchy-t3-client"),
        &root,
    )
    .unwrap();
    let store = config.client_settings_path();
    let storage = config.storage_root();
    let e = server.env.clone();
    let (tx, rx) = mpsc::channel();
    let manager = browser_sessions::Manager::start(
        config,
        move |id| {
            if id == e.id {
                Ok(e.clone())
            } else {
                Err("unknown_environment".into())
            }
        },
        || Ok(json!({"browserDefaultProfileId":"default","browserDefaultZoomFactor":1})),
        move |v| {
            let _ = tx.send(v);
        },
    );
    let show = json!({"op":"browser-show","requestId":"surface-one","environment":"fixture","thread":"thread","tabId":"a","originKey":read["originKey"],"serverEpoch":"one","width":640,"height":400,"url":"file:///forged","profileId":"forged"});
    manager.request(show.clone(), true);
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .expect("authorized surface deadline");
        assert_ne!(v["kind"], "error", "{v}");
        if v["kind"] == "frame" {
            assert_eq!(v["requestId"], "surface-one");
            assert_eq!(v["tabId"], "a");
            break;
        }
    }
    let until = Instant::now() + Duration::from_secs(4);
    while !server.calls.lock().unwrap().iter().any(|r| {
        r["tag"] == "preview.reportStatus" && r["payload"]["navStatus"]["_tag"] == "Success"
    }) && Instant::now() < until
    {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(server
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["tag"] == "preview.reportStatus"
            && r["payload"]["navStatus"]["_tag"] == "Success"));
    for event in [
        json!({"type":"press","x":50,"y":40,"button":1,"buttons":1}),
        json!({"type":"release","x":50,"y":40,"button":1,"buttons":0}),
    ] {
        manager.request(
            json!({"op":"browser-input","requestId":"surface-one","event":event}),
            true,
        );
    }
    let until = Instant::now() + Duration::from_secs(3);
    while !server
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["http"] == "GET /clicked HTTP/1.1")
        && Instant::now() < until
    {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(server
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["http"] == "GET /clicked HTTP/1.1"));
    let navigate = json!({"op":"preview-action","action":"navigate","environment":"fixture","thread":"thread","tabId":"a","requestId":uuid::Uuid::new_v4().to_string(),"originKey":read["originKey"],"serverEpoch":"one","url":format!("{}/next",server.env.base_url)});
    let result = preview_actions::execute(&server.env, &navigate);
    assert_eq!(result.as_ref().unwrap()["state"], "complete");
    manager.action(navigate, result);
    let until = Instant::now() + Duration::from_secs(3);
    while !server
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["http"] == "GET /next HTTP/1.1")
        && Instant::now() < until
    {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(server
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["http"] == "GET /next HTTP/1.1"));
    // History goes through the original desktop-control boundary, not an
    // invented native history RPC. Native status proves the actual Qt URL.
    let reported = |after: usize, url: &str| {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            let calls = server.calls.lock().unwrap();
            if calls.iter().skip(after).any(|r| {
                r["tag"] == "preview.reportStatus"
                    && r["payload"]["navStatus"]["_tag"] == "Success"
                    && r["payload"]["navStatus"]["url"] == url
            }) {
                return calls.len();
            }
            drop(calls);
            assert!(
                Instant::now() < until,
                "missing actual browser status {url}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    };
    let count = reported(0, &format!("{}/next", server.env.base_url));
    manager.request(
        json!({"op":"browser-control","requestId":"surface-one","action":"back"}),
        true,
    );
    let count = reported(count, &format!("{}/page", server.env.base_url));
    manager.request(
        json!({"op":"browser-control","requestId":"surface-one","action":"forward"}),
        true,
    );
    reported(count, &format!("{}/next", server.env.base_url));
    let loads = server
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["http"] == "GET /next HTTP/1.1")
        .count();
    manager.request(
        json!({"op":"browser-control","requestId":"surface-one","action":"hard-reload"}),
        true,
    );
    let until = Instant::now() + Duration::from_secs(3);
    while server
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["http"] == "GET /next HTTP/1.1")
        .count()
        == loads
        && Instant::now() < until
    {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        server
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["http"] == "GET /next HTTP/1.1")
            .count()
            > loads
    );
    manager.request(
        json!({"op":"browser-control","requestId":"surface-one","action":"zoom-in"}),
        true,
    );
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if v["kind"] == "zoom" {
            assert_eq!(v["value"], 1.1);
            break;
        }
    }
    let mut again = show.clone();
    again["requestId"] = json!("surface-two");
    manager.request(again, true);
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if v["kind"] == "shown" && v["requestId"] == "surface-two" {
            assert_eq!(
                v["zoomFactor"], 1.1,
                "per-tab zoom survives presentation changes"
            );
            break;
        }
    }
    use omarchy_t3_client::browser_cleanup::{Runtime, Scope};
    let scope = [Scope {
        environment: "fixture".into(),
        origin_key: read["originKey"].as_str().unwrap().into(),
    }];
    // A nonempty import must not recreate the existing Qt page or its history.
    let page_loads = || {
        server
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["http"].as_str().is_some_and(|v| v.starts_with("GET /")))
            .count()
    };
    let before_import = page_loads();
    let mut importing = manager.begin("default", &store, &scope).unwrap();
    assert!(importing
        .write_cookie(
            "fixture",
            omarchy_t3_client::browser_cookies::ImportedCookie {
                url: format!("{}/", server.env.base_url),
                name: "live-import".into(),
                value: "PRIVATE_LIVE_COOKIE".into(),
                domain: None,
                path: "/".into(),
                secure: false,
                http_only: true,
                expiration_date: None,
                same_site: "strict",
            }
        )
        .unwrap());
    let flushing = thread::spawn(move || {
        let result = importing.flush_import("fixture");
        (importing, result)
    });
    thread::sleep(Duration::from_millis(100));
    assert!(
        !flushing.is_finished(),
        "fresh cookie awaits Chromium persistence"
    );
    let parallel_start = Instant::now();
    let parallel = manager.begin("parallel", &store, &scope).unwrap();
    assert!(
        parallel_start.elapsed() < Duration::from_secs(2),
        "another profile remains available while live import persistence is pending"
    );
    drop(parallel);
    let (importing, result) = flushing.join().unwrap();
    result.unwrap();
    drop(importing);
    let mut resume = show.clone();
    resume["requestId"] = json!("after-import");
    manager.request(resume, true);
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if v["requestId"] == "after-import" && v["kind"] == "frame" {
            break;
        }
    }
    assert_eq!(
        page_loads(),
        before_import,
        "import/lease end/reattach cannot reload the live page"
    );
    let count = server.calls.lock().unwrap().len();
    manager.request(
        json!({"op":"browser-control","requestId":"after-import","action":"back"}),
        true,
    );
    reported(count, &format!("{}/page", server.env.base_url));
    let count = server.calls.lock().unwrap().len();
    manager.request(
        json!({"op":"browser-control","requestId":"after-import","action":"forward"}),
        true,
    );
    reported(count, &format!("{}/next", server.env.base_url));
    let mut cleanup = manager.begin("default", &store, &scope).unwrap();
    let mut blocked = show.clone();
    blocked["requestId"] = json!("during-cleanup");
    manager.request(blocked, true);
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if v["requestId"] == "during-cleanup" {
            assert_eq!(v["error"], "browser_profile_busy");
            break;
        }
    }
    cleanup.clear("fixture").unwrap();
    let partition = omarchy_t3_client::browser_runtime::partition("fixture", "default").unwrap();
    assert!(
        !storage
            .join(
                partition["partition"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("persist:")
            )
            .exists(),
        "active Qt profile flushed and removed under reservation"
    );
    drop(cleanup);
    let mut reopened = show.clone();
    reopened["requestId"] = json!("after-cleanup");
    manager.request(reopened, true);
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if v["requestId"] == "after-cleanup" {
            assert_ne!(v["kind"], "error", "{v}");
            if v["kind"] == "shown" {
                assert_eq!(
                    v["zoomFactor"], 1.1,
                    "fresh authorized attachment retains per-tab zoom across profile operations"
                );
            }
            if v["kind"] == "frame" {
                break;
            }
        }
    }
    manager.hide();
    while rx.try_recv().is_ok() {}
    thread::sleep(Duration::from_millis(100));
    assert!(!rx.try_iter().any(|v| v["kind"] == "frame"));
    server.operate.store(false, Ordering::Relaxed);
    let mut denied = show;
    denied["requestId"] = json!("denied");
    manager.request(denied, true);
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if v["requestId"] == "denied" {
            assert_eq!(v["error"], "permission_denied");
            break;
        }
    }
    manager.shutdown().unwrap();
    drop(manager);
    let frames = root.join("cache/omarchy-t3-code/browser-sessions");
    let until = Instant::now() + Duration::from_secs(3);
    while fs::read_dir(&frames).unwrap().count() > 0 && Instant::now() < until {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(fs::read_dir(frames).unwrap().count(), 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn actual_worker_observes_read_only_serializes_lifecycle_and_interrupts_when_hidden() {
    use std::{
        fs,
        io::{BufRead, BufReader},
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        sync::mpsc,
    };
    let server = Server::new(vec![]);
    let root = std::env::temp_dir().join(format!("ot3-preview-ipc-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let token = root.join("token");
    fs::write(&token, "synthetic").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let fixture = root.join("environments.json");
    fs::write(&fixture,json!({"environments":[{"id":"fixture","base_url":server.env.base_url,"token_file":token}]}).to_string()).unwrap();
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
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", root.join("home"))
            .env("T3CODE_HOME", root.join("native"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("OMARCHY_T3_FIXTURE_ENVIRONMENTS", fixture)
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
        stdin.flush().unwrap();
    };
    send(
        &mut stdin,
        json!({"op":"preview-watch","environment":"fixture","thread":"thread","requestId":"hidden"}),
    );
    thread::sleep(Duration::from_millis(150));
    assert!(!server
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["tag"] == "subscribePreviewEvents"));
    send(&mut stdin, json!({"op":"visible","value":true}));
    send(
        &mut stdin,
        json!({"op":"preview-watch","environment":"fixture","thread":"thread","requestId":"visible"}),
    );
    let until = Instant::now() + Duration::from_secs(5);
    let origin;
    loop {
        let value = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .expect("preview IPC deadline");
        if value["type"] == "preview-state" {
            assert_eq!(value["data"]["requestId"], "visible");
            assert_eq!(value["data"]["kind"], "list");
            assert_eq!(value["data"]["canOperate"], false);
            assert!(!value.to_string().contains("PRIVATE"));
            origin = value["data"]["originKey"].clone();
            break;
        }
    }
    server.operate.store(true, Ordering::Relaxed);
    *server.mode.lock().unwrap() = "blocked".into();
    let one = json!({"op":"preview-action","action":"open","environment":"fixture","thread":"thread","requestId":"serial-one","originKey":origin,"serverEpoch":"one"});
    let mut two = one.clone();
    two["requestId"] = json!("serial-two");
    send(&mut stdin, one.clone());
    send(&mut stdin, two);
    let until = Instant::now() + Duration::from_secs(2);
    while !server
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["tag"] == "preview.open")
        && Instant::now() < until
    {
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        server
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "preview.open")
            .count(),
        1,
        "same-thread second mutation must wait for first acknowledgement"
    );
    *server.mode.lock().unwrap() = "stable".into();
    let mut receipts = vec![];
    let until = Instant::now() + Duration::from_secs(3);
    while receipts.len() < 2 {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .expect("preview lifecycle IPC deadline");
        if v["type"] == "action" && v["data"]["op"] == "preview-action" {
            assert_eq!(v["data"]["result"]["state"], "complete", "{v}");
            receipts.push(v["data"]["requestId"].clone());
        }
    }
    assert_eq!(receipts, vec![json!("serial-one"), json!("serial-two")]);
    send(&mut stdin, one);
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        let v = rx
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if v["type"] == "action" && v["data"]["requestId"] == "serial-one" {
            assert_eq!(v["data"]["error"], "preview_command_replayed");
            break;
        }
    }
    assert_eq!(
        server
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "preview.open")
            .count(),
        2
    );
    send(&mut stdin, json!({"op":"visible","value":false}));
    let until = Instant::now() + Duration::from_secs(2);
    while server.interrupts.load(Ordering::Relaxed) == 0 && Instant::now() < until {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        server.interrupts.load(Ordering::Relaxed),
        1,
        "hidden view must interrupt its native stream"
    );
    assert!(server.calls.lock().unwrap().iter().all(|r| matches!(
        r["tag"].as_str(),
        Some("server.getConfig" | "preview.list" | "subscribePreviewEvents" | "preview.open" | "orchestration.subscribeShell" | "subscribeServerConfig")
    )));
    drop(stdin);
    let _ = child.0.kill();
    let _ = child.0.wait();
    reader.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}
