use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

struct Fixture {
    child: Option<Child>,
    stop: Arc<AtomicBool>,
    root: std::path::PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "mosaic stream lifecycle timed out"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn eight_mosaic_streams_across_hosts_deliver_updates_and_cancel_only_removed_tiles() {
    let root = std::env::temp_dir().join(format!("t3-mosaic-live-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let mut fixture = Fixture {
        child: None,
        stop: stop.clone(),
        root: root.clone(),
    };
    let token = root.join("token");
    fs::write(&token, "synthetic-token").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let opened = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(AtomicUsize::new(0));
    let mut environments = vec![];
    let mut servers = vec![];
    for host in ["a", "b"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        environments.push(json!({"id":host,"name":host,"base_url":format!("http://{}",listener.local_addr().unwrap()),"token_file":token}));
        let (stop, opened, closed) = (stop.clone(), opened.clone(), closed.clone());
        servers.push(thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else { thread::sleep(Duration::from_millis(5)); continue };
                let (stop, opened, closed) = (stop.clone(), opened.clone(), closed.clone());
                thread::spawn(move || {
                    socket.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                    let mut peek = [0; 8192];
                    let n = socket.peek(&mut peek).unwrap_or(0);
                    let request = String::from_utf8_lossy(&peek[..n]).to_string();
                    if !request.starts_with("GET /ws") {
                        let mut bytes = [0; 8192]; let _ = socket.read(&mut bytes);
                        let body = if request.contains("/bounded") {
                            let id = request.split("/threads/").nth(1).unwrap().split('/').next().unwrap();
                            json!({"snapshotSequence":0,"projection":{"thread":{"id":id},"messages":[{"id":"initial","role":"user","text":format!("snapshot {id}"),"createdAt":1000}],"runs":[],"turnItems":[]}})
                        } else { json!({"threads":[],"projects":[]}) }.to_string();
                        let _ = write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
                        return;
                    }
                    let Ok(mut ws) = tungstenite::accept(socket) else { return };
                    let Ok(message) = ws.read() else { return };
                    let req: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
                    match req["tag"].as_str().unwrap_or("") {
                        "server.getConfig" => {
                            let _ = ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"threadResumeCompletionMarker":true}}}).to_string().into()));
                        }
                        "orchestration.subscribeThread" => {
                            let id = req["payload"]["threadId"].as_str().unwrap();
                            assert!(id.starts_with(host), "thread routed to the wrong host");
                            assert_eq!(req["payload"]["afterSequence"], 0);
                            opened.fetch_add(1, Ordering::Relaxed);
                            let _ = ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":[{"kind":"synchronized"}]}).to_string().into()));
                            // Let the initial snapshot's render throttle elapse before the delta.
                            thread::sleep(Duration::from_millis(100));
                            let event = json!({"kind":"event","sequence":1,"event":{"type":"message.updated","payload":{"id":"live","role":"assistant","text":format!("stream {id}"),"createdAt":2000,"attachments":[]}}});
                            let _ = ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":[event]}).to_string().into()));
                            while !stop.load(Ordering::Relaxed) {
                                match ws.read() {
                                    Ok(tungstenite::Message::Text(text)) => {
                                        let value: Value = serde_json::from_str(&text).unwrap();
                                        if value["_tag"] == "Interrupt" { break }
                                        if value["_tag"] == "Ping" { let _ = ws.send(tungstenite::Message::Text(json!({"_tag":"Pong"}).to_string().into())); }
                                    }
                                    Ok(tungstenite::Message::Close(_)) => break,
                                    Err(tungstenite::Error::Io(e)) if matches!(e.kind(),std::io::ErrorKind::TimedOut|std::io::ErrorKind::WouldBlock) => {}
                                    Err(_) => break,
                                    _ => {}
                                }
                            }
                            closed.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => {}
                    }
                });
            }
        }));
    }
    let config = root.join("environments.json");
    fs::write(&config, json!({"environments":environments}).to_string()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
        .arg("watch")
        .env("HOME", root.join("home"))
        .env("T3CODE_HOME", root.join("native"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("OMARCHY_T3_FIXTURE_ENVIRONMENTS", config)
        .env_remove("OPENCODE_API_KEY")
        .env_remove("OPENCODE_GO_API_KEY")
        .env_remove("KIMI_CODING_API_KEY")
        .env_remove("NOUS_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    fixture.child = Some(child);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                let _ = tx.send(value);
            }
        }
    });
    let targets: Vec<_> = (0..8)
        .map(|i| {
            let host = if i % 2 == 0 { "a" } else { "b" };
            json!({"environment":host,"thread":format!("{host}{i}"),"serial":100+i})
        })
        .collect();
    writeln!(input, "{}", json!({"op":"visible","value":true})).unwrap();
    writeln!(input, "{}", json!({"op":"mosaic-watch","targets":targets})).unwrap();
    let mut received = HashSet::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while received.len() < 8 {
        assert!(
            Instant::now() < deadline,
            "missing mosaic updates: {received:?}"
        );
        let Ok(event) = rx.recv_timeout(Duration::from_millis(500)) else {
            continue;
        };
        if event["type"] != "mosaic-messages" {
            continue;
        }
        let serial = event["data"]["serial"].as_u64().unwrap();
        let target = &targets[(serial - 100) as usize];
        let messages = event["data"]["result"]["messages"].as_array().unwrap();
        if messages
            .iter()
            .any(|m| m["text"] == format!("stream {}", target["thread"].as_str().unwrap()))
        {
            assert_eq!(messages.len(), 2);
            received.insert(serial);
        }
    }
    assert_eq!(opened.load(Ordering::Relaxed), 8);
    writeln!(
        input,
        "{}",
        json!({"op":"mosaic-watch","targets":[targets[0]]})
    )
    .unwrap();
    until(|| closed.load(Ordering::Relaxed) == 7);
    assert_eq!(
        opened.load(Ordering::Relaxed),
        8,
        "retained tile must not reconnect"
    );
    writeln!(input, "{}", json!({"op":"visible","value":false})).unwrap();
    until(|| closed.load(Ordering::Relaxed) == 8);
    writeln!(input, "{}", json!({"op":"quit"})).unwrap();
    assert!(fixture.child.as_mut().unwrap().wait().unwrap().success());
    stop.store(true, Ordering::Relaxed);
    for server in servers {
        server.join().unwrap();
    }
}
