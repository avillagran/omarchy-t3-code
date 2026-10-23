use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

fn chunk(values: Value) -> tungstenite::Message {
    tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":values}).to_string().into())
}
fn thread_shell(title: &str) -> Value {
    json!({"id":"one","projectId":"p","title":title,"updatedAt":"2026-10-23T10:00:00Z"})
}

/// Shell deltas reach the sidebar without another HTTP shell read, and a reconnect
/// resumes after the last applied sequence with the advertised completion marker.
#[test]
fn shell_deltas_update_the_sidebar_and_reconnects_resume_from_the_cursor() {
    let root = std::env::temp_dir().join(format!("t3-shell-watch-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let token = root.join("token");
    fs::write(&token, "synthetic-token").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let config = root.join("env.json");
    fs::write(&config, json!({"environments":[{"id":"fixture","name":"Fixture","base_url":base,"token_file":token}]}).to_string()).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let http_shell_reads = Arc::new(AtomicUsize::new(0));
    let subscriptions = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (server_stop, reads, seen) = (stop.clone(), http_shell_reads.clone(), subscriptions.clone());
    let server = thread::spawn(move || {
        while !server_stop.load(Ordering::Relaxed) {
            let Ok((mut s, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            let (reads, seen, server_stop) = (reads.clone(), seen.clone(), server_stop.clone());
            thread::spawn(move || {
                s.set_nonblocking(false).unwrap();
                s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut peek = [0; 1024];
                let n = s.peek(&mut peek).unwrap_or(0);
                let request = String::from_utf8_lossy(&peek[..n]).to_string();
                if !request.starts_with("GET /ws") {
                    let mut data = [0; 8192];
                    let _ = s.read(&mut data);
                    if request.starts_with("GET /api/orchestration/shell") {
                        reads.fetch_add(1, Ordering::Relaxed);
                    }
                    let body = json!({"schemaVersion":1,"snapshotSequence":1,"threads":[thread_shell("From HTTP")],"archivedThreads":[],"projects":[]}).to_string();
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                    return;
                }
                let mut ws = tungstenite::accept(s).unwrap();
                let req: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
                match req["tag"].as_str().unwrap_or("") {
                    "server.getConfig" => {
                        let _ = ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"shellResumeCompletionMarker":true}}}).to_string().into()));
                    }
                    "orchestration.subscribeShell" => {
                        let round = {
                            let mut seen = seen.lock().unwrap();
                            seen.push(req["payload"].clone());
                            seen.len()
                        };
                        if round == 1 {
                            ws.send(chunk(json!([{"kind":"snapshot","snapshot":{"schemaVersion":1,"snapshotSequence":10,"threads":[thread_shell("From stream")],"archivedThreads":[],"projects":[]}},{"kind":"synchronized"}]))).unwrap();
                            thread::sleep(Duration::from_millis(1500));
                            ws.send(chunk(json!([{"kind":"thread.updated","sequence":11,"location":"active","thread":thread_shell("Delta")}]))).unwrap();
                            thread::sleep(Duration::from_millis(1500));
                            // Drop the socket without a close frame, as a network change would.
                            return;
                        }
                        ws.send(chunk(json!([{"kind":"synchronized"},{"kind":"thread.updated","sequence":12,"location":"active","thread":thread_shell("Resumed")}]))).unwrap();
                        while !server_stop.load(Ordering::Relaxed) {
                            match ws.read() {
                                Ok(tungstenite::Message::Text(t)) => {
                                    let v: Value = serde_json::from_str(&t).unwrap();
                                    if v["_tag"] == "Interrupt" {
                                        break;
                                    }
                                    if v["_tag"] == "Ping" {
                                        let _ = ws.send(tungstenite::Message::Text(json!({"_tag":"Pong"}).to_string().into()));
                                    }
                                }
                                Ok(tungstenite::Message::Close(_)) => break,
                                Err(tungstenite::Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
                                Err(_) => break,
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            });
        }
    });
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
    let (tx, rx) = mpsc::channel();
    let output = child.stdout.take().unwrap();
    thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                let _ = tx.send(v);
            }
        }
    });
    let start = Instant::now();
    let mut titles = vec![];
    while titles.last().map(String::as_str) != Some("Resumed") {
        assert!(start.elapsed() < Duration::from_secs(20), "titles so far: {titles:?}");
        let Ok(v) = rx.recv_timeout(Duration::from_secs(15)) else { continue };
        if v["type"] == "snapshot" {
            if let Some(title) = v["data"]["threads"][0]["title"].as_str() {
                if titles.last().map(String::as_str) != Some(title) {
                    titles.push(title.to_string());
                }
            }
        }
    }
    assert!(titles.contains(&"Delta".to_string()), "the delta was applied in memory: {titles:?}");
    let payloads = subscriptions.lock().unwrap().clone();
    assert_eq!(payloads[0], json!({"requestCompletionMarker":true}));
    assert_eq!(payloads[1], json!({"afterSequence":11,"requestCompletionMarker":true}));
    // The first poll and at most one fallback read while disconnected; never one per event.
    assert!(http_shell_reads.load(Ordering::Relaxed) <= 2, "HTTP shell reads: {}", http_shell_reads.load(Ordering::Relaxed));
    writeln!(input, "{}", json!({"op":"quit"})).unwrap();
    child.wait().unwrap();
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}
