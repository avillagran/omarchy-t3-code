use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait(mut check: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(6);
    while !check() {
        assert!(Instant::now() < until, "embedded terminal deadline");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn embedded_view_preserves_server_scope_utf8_order_and_detach_lifetime() {
    let root = std::env::temp_dir().join(format!("o3-embedded-terminal-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    let paste = root.join("bin/wl-paste");
    fs::write(&paste, "#!/bin/sh\n[ \"$*\" = \"--primary --no-newline --type text\" ] || exit 7\nprintf 'PRIMARY café\\n'\n").unwrap();
    fs::set_permissions(&paste, fs::Permissions::from_mode(0o755)).unwrap();
    let token = root.join("token");
    fs::write(&token, "synthetic-terminal-token").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = root.join("environments.json");
    fs::write(&config,json!({"environments":[{"id":"fixture","name":"Remote fixture","base_url":format!("http://{}",listener.local_addr().unwrap()),"token_file":token,"write_token_file":token}]}).to_string()).unwrap();
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let closed = Arc::new(AtomicBool::new(false));
    let workers = Arc::new(Mutex::new(vec![]));
    let disconnect = Arc::new(AtomicBool::new(false));
    let server_disconnect = disconnect.clone();
    let server_requests = requests.clone();
    let server_stop = stop.clone();
    let server_closed = closed.clone();
    let server_workers = workers.clone();
    let server = thread::spawn(move || {
        while !server_stop.load(Ordering::Relaxed) {
            let Ok((mut socket, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            let requests = server_requests.clone();
            let closed = server_closed.clone();
            let stop = server_stop.clone();
            let disconnect = server_disconnect.clone();
            server_workers.lock().unwrap().push(thread::spawn(move||{
                socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut peek=[0;4096];let Ok(n)=socket.peek(&mut peek) else{return};
                if String::from_utf8_lossy(&peek[..n]).starts_with("GET /ws"){
                    let Ok(mut ws)=tungstenite::accept(socket) else{return};let Ok(message)=ws.read() else{return};let Ok(req)=serde_json::from_str::<Value>(message.to_text().unwrap_or("")) else{return};requests.lock().unwrap().push(req.clone());
                    if req["tag"]=="terminal.attach" {
                        let chunk=json!({"_tag":"Chunk","requestId":"1","values":[{"type":"snapshot","snapshot":{"history":"HISTORY\r\n","sequence":4,"status":"running","cols":80,"rows":24}},{"type":"output","data":"DUPLICATE","sequence":4},{"type":"output","data":"héllo\u{1b}[31m","sequence":5}]});
                        if ws.send(tungstenite::Message::Text(chunk.to_string().into())).is_err(){return}
                        ws.get_mut().set_read_timeout(Some(Duration::from_millis(80))).unwrap();
                        while !stop.load(Ordering::Relaxed){
                            if disconnect.swap(false,Ordering::Relaxed){let _=ws.close(None);break}
                            if closed.load(Ordering::Relaxed){let _=ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":[{"type":"closed","sequence":6}]}).to_string().into()));break}
                            match ws.read(){Ok(tungstenite::Message::Close(_))=>break,Err(tungstenite::Error::Io(e)) if matches!(e.kind(),std::io::ErrorKind::TimedOut|std::io::ErrorKind::WouldBlock)=>{},Err(_)=>break,_=>{}}
                        }
                    }else{
                        if req["tag"]=="terminal.close" {closed.store(true,Ordering::Relaxed);}
                        let _=ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":null}}).to_string().into()));
                    }
                }else{
                    let mut bytes=[0;8192];let _=socket.read(&mut bytes);
                    let shell=json!({"threads":[{"id":"one","projectId":"project","worktreePath":"/remote/work tree","modelSelection":{"instanceId":"provider"}}],"projects":[{"id":"project","workspaceRoot":"/remote/root"}],"environments":[]}).to_string();
                    let _=write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",shell.len(),shell);
                }
            }));
        }
    });
    let mut child = Child(
        Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
            .arg("watch")
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    root.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env("HOME", root.join("home"))
            .env("T3CODE_HOME", root.join("native"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("OMARCHY_T3_FIXTURE_ENVIRONMENTS", config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut input = child.0.stdin.take().unwrap();
    let output = child.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                let _ = tx.send(v);
            }
        }
    });
    let send = |input: &mut std::process::ChildStdin, v: Value| {
        writeln!(input, "{v}").unwrap();
        input.flush().unwrap();
    };
    let event = |id: &str, predicate: fn(&Value) -> bool| {
        let until = Instant::now() + Duration::from_secs(6);
        loop {
            let v = rx
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .expect("terminal event deadline");
            if v["type"] == "terminal-surface"
                && v["data"]["requestId"] == id
                && predicate(&v["data"])
            {
                return v["data"].clone();
            }
        }
    };
    send(&mut input, json!({"op":"visible","value":true}));
    let attach = |id: &str| json!({"op":"terminal-surface-attach","requestId":id,"environment":"fixture","thread":"one","terminalId":"term-pane","columns":80,"rows":24});
    let mut initial = attach("first");
    initial["command"] = json!("printf once");
    send(&mut input, initial);
    assert_eq!(
        event("first", |v| v["event"]["type"] == "snapshot")["data"],
        "HISTORY\r\n"
    );
    assert!(event("first", |v| v["event"]["sequence"] == 4)["data"].is_null());
    assert_eq!(
        event("first", |v| v["event"]["sequence"] == 5)["data"],
        "héllo\u{1b}[31m"
    );
    send(
        &mut input,
        json!({"op":"terminal-surface-write","requestId":"first","thread":"other","terminalId":"wrong","data":"héllo\u{3}"}),
    );
    send(
        &mut input,
        json!({"op":"terminal-surface-resize","requestId":"first","columns":120,"rows":40}),
    );
    send(
        &mut input,
        json!({"op":"terminal-surface-clear","requestId":"first"}),
    );
    wait(|| {
        requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r["tag"] == "terminal.clear")
    });
    let calls = requests.lock().unwrap().clone();
    let attach_call = calls
        .iter()
        .find(|r| r["tag"] == "terminal.attach")
        .unwrap();
    assert_eq!(attach_call["payload"]["cwd"], "/remote/work tree");
    assert_eq!(attach_call["payload"]["providerInstanceId"], "provider");
    assert_eq!(attach_call["payload"]["resume"], true);
    assert_eq!(attach_call["payload"]["restartIfNotRunning"], false);
    let writes: Vec<_> = calls
        .iter()
        .filter(|r| r["tag"] == "terminal.write")
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0]["payload"]["data"], "printf once\r");
    assert_eq!(
        writes[1]["payload"],
        json!({"threadId":"one","terminalId":"term-pane","data":"héllo\u{3}"})
    );
    assert!(calls.iter().any(|r| r["tag"] == "terminal.resize"
        && r["payload"]["cols"] == 120
        && r["payload"]["rows"] == 40));
    let large = format!("{}😀{}", "a".repeat(65535), "é".repeat(40000));
    send(
        &mut input,
        json!({"op":"terminal-surface-write","requestId":"first","data":large}),
    );
    send(
        &mut input,
        json!({"op":"terminal-surface-primary","requestId":"first"}),
    );
    assert_eq!(
        event("first", |v| v["kind"] == "primary-text")["text"],
        "PRIMARY café\n"
    );
    let received = requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["tag"] == "terminal.write")
        .skip(2)
        .map(|r| r["payload"]["data"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(received.len() > 1);
    assert!(received.iter().all(|chunk| chunk.len() <= 65536));
    assert_eq!(received.concat(), large);
    disconnect.store(true, Ordering::Relaxed);
    event("first", |v| v["event"]["type"] == "snapshot");
    assert_eq!(
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "terminal.attach")
            .count(),
        2
    );
    assert_eq!(
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "terminal.write" && r["payload"]["data"] == "printf once\r")
            .count(),
        1
    );
    send(&mut input, json!({"op":"visible","value":false}));
    thread::sleep(Duration::from_millis(120));
    assert!(!requests
        .lock()
        .unwrap()
        .iter()
        .any(|r| r["tag"] == "terminal.close"));
    send(&mut input, json!({"op":"visible","value":true}));
    send(&mut input, attach("second"));
    event("second", |v| v["event"]["type"] == "snapshot");
    send(
        &mut input,
        json!({"op":"terminal-surface-close","requestId":"second"}),
    );
    event("second", |v| v["event"]["type"] == "closed");
    assert_eq!(
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "terminal.close")
            .count(),
        1
    );
    drop(child);
    reader.join().unwrap();
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    for worker in workers.lock().unwrap().drain(..) {
        worker.join().unwrap();
    }
}
