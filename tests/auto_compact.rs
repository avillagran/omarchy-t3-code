use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};
struct Harness {
    root: std::path::PathBuf,
    stop: Arc<AtomicBool>,
    server: Option<thread::JoinHandle<()>>,
    commands: Arc<Mutex<Vec<Value>>>,
    reads: Arc<Mutex<usize>>,
}
impl Harness {
    fn new(mode: &str) -> Self {
        let root = std::env::temp_dir().join(format!("t3-auto-compact-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("state/omarchy-t3-code")).unwrap();
        fs::write(root.join("state/omarchy-t3-code/ui-state.json"),json!({"schemaVersion":1,"autoCompactEnabled":true,"autoCompactProviderMinutes":{"codex":1}}).to_string()).unwrap();
        if mode.starts_with("memory") {
            fs::write(root.join("state/omarchy-t3-code/ui-state.json"), json!({"schemaVersion":1,"autoCompactEnabled":false,"idleReleaseEnabled":true,"idleReleaseMinutes":1}).to_string()).unwrap();
        }
        let token = root.join("token");
        fs::write(&token, "synthetic-token").unwrap();
        fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        fs::write(root.join("env.json"),json!({"environments":[{"id":"fixture","name":"Fixture","base_url":format!("http://{}",listener.local_addr().unwrap()),"token_file":token}]}).to_string()).unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let iso = |n| {
            chrono::DateTime::from_timestamp_millis(n)
                .unwrap()
                .to_rfc3339()
        };
        let mut raw = json!({"id":"thread","title":"Synthetic","projectId":"p","providerInstanceId":"codex","modelSelection":{"instanceId":"codex","model":"m"},"activeProviderThreadId":"pt","activeRunId":null,"status":"completed","updatedAt":iso(now-50000),"latestRunCompletedAt":iso(now-50000),"latestUserAuthoredMessageAt":iso(now-60000),"latestRunId":"r","latestRunStatus":"completed"});
        if mode == "settled" {
            raw["settledOverride"] = json!("settled");
            raw["settledAt"] = json!(iso(now - 40000));
        }
        let mut projection = json!({"thread":raw,"messages":[{"id":"human","role":"user","createdBy":"user","text":"synthetic task","createdAt":iso(now-if mode=="expired"{120000}else{60000})}],"providerThreads":[{"id":"pt","appThreadId":"thread","providerInstanceId":"codex"}],"providerTurns":[{"id":"turn","providerThreadId":"pt","tokenUsage":{"usedTokens":50000,"updatedAt":iso(now-if mode=="scheduled"{43000}else if mode=="expired"{61000}else{50000})}}],"runs":[{"id":"r","providerThreadId":"pt","status":"completed","completedAt":iso(now-50000)}],"runtimeRequests":[],"plans":[],"subagents":[]});
        if mode.starts_with("memory") {
            projection["thread"]["latestRunCompletedAt"] = json!(iso(now - 180000));
            projection["thread"]["latestUserAuthoredMessageAt"] = json!(iso(now - 190000));
            projection["runs"][0]["completedAt"] = json!(iso(now - 180000));
            projection["providerThreads"][0]["driver"] = json!("codex");
            projection["providerThreads"][0]["providerSessionId"] = json!("native-session");
            projection["providerThreads"][0]["nativeThreadRef"] =
                json!({"driver":"codex","nativeId":"saved-conversation"});
            projection["providerSessions"] = json!([{"id":"native-session","driver":"codex","providerInstanceId":"codex","status":"ready","createdAt":iso(now-200000)}]);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let stopper = stop.clone();
        let commands = Arc::new(Mutex::new(vec![]));
        let record = commands.clone();
        let mode = mode.to_owned();
        let reads = Arc::new(Mutex::new(0usize));
        let server_reads = reads.clone();
        let server = thread::spawn(move || {
            while !stopper.load(Ordering::Relaxed) {
                if let Ok((mut socket, _)) = listener.accept() {
                    let commands = record.clone();
                    let mode = mode.clone();
                    let p = projection.clone();
                    let reads = server_reads.clone();
                    thread::spawn(move || {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut head = [0; 4096];
                        let n = socket.peek(&mut head).unwrap_or(0);
                        let first = String::from_utf8_lossy(&head[..n]);
                        if first.starts_with("GET /ws") {
                            let mut ws = tungstenite::accept(socket).unwrap();
                            let req: Value =
                                serde_json::from_str(ws.read().unwrap().to_text().unwrap())
                                    .unwrap();
                            if req["tag"] == "orchestration.subscribeShell" {
                                let _ = ws.close(None);
                                return;
                            }
                            let result = match req["tag"].as_str().unwrap() {
                                "server.getConfig" => {
                                    json!({"providers":[{"instanceId":"codex","enabled":true,"slashCommands":[{"name":"compact"}]}]})
                                }
                                "server.getSettings" if mode == "server" => {
                                    json!({"idleCompaction":"onReturn","providerInstances":{"codex":{"config":{"idleCompaction":"beforeCacheExpires"}}}})
                                }
                                "server.getSettings" => json!({"idleCompaction":"onReturn"}),
                                "orchestration.dispatchCommand" => {
                                    let command = req["payload"].clone();
                                    assert!(matches!(
                                        command["type"].as_str(),
                                        Some(
                                            "message.dispatch"
                                                | "queued-run.cancel"
                                                | "provider-session.detach"
                                        )
                                    ));
                                    if command["type"] == "message.dispatch" {
                                        assert_eq!(command["text"], "/compact");
                                        assert_eq!(
                                            command["dispatchMode"]["type"],
                                            "start_immediately"
                                        );
                                        assert!(command.get("deliveryIntent").is_none());
                                        assert_eq!(
                                            command["modelSelection"]["instanceId"],
                                            "codex"
                                        );
                                    }
                                    commands.lock().unwrap().push(command);
                                    if mode == "uncertain" {
                                        let _ = ws.close(None);
                                        return;
                                    }
                                    json!({"sequence":10})
                                }
                                _ => panic!("unexpected RPC: {}", req["tag"]),
                            };
                            let _=ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}}).to_string().into()));
                        } else {
                            let body = if first.contains("/bounded?compactTurnItems=1") {
                                let mut p = p.clone();
                                let mut count = reads.lock().unwrap();
                                *count += 1;
                                if mode == "settled-race" && *count >= 2 {
                                    p["thread"]["settledOverride"] = json!("settled");
                                    p["thread"]["settledAt"] =
                                        json!(chrono::Utc::now().to_rfc3339());
                                }
                                if (mode == "busy" || mode == "memory-busy") && *count >= 2 {
                                    p["thread"]["activeRunId"] = json!("other");
                                }
                                if mode == "queued" {
                                    if let Some(sent) = commands
                                        .lock()
                                        .unwrap()
                                        .iter()
                                        .find(|c| c["type"] == "message.dispatch")
                                    {
                                        p["runs"].as_array_mut().unwrap().push(json!({"id":"our-queued","status":"queued","userMessageId":sent["messageId"]}));
                                    }
                                }
                                json!({"projection":p,"snapshotSequence":1})
                            } else {
                                json!({"threads":[p["thread"]],"projects":[]})
                            };
                            let mut bytes = [0; 8192];
                            let _ = socket.read(&mut bytes);
                            let body = body.to_string();
                            let _=write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
                        }
                    });
                } else {
                    thread::sleep(Duration::from_millis(5))
                }
            }
        });
        Self {
            root,
            stop,
            server: Some(server),
            commands,
            reads,
        }
    }
    fn start(&self) -> (Child, ChildStdin, mpsc::Receiver<Value>) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
            .arg("watch")
            .env("HOME", self.root.join("home"))
            .env("T3CODE_HOME", self.root.join("native"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_DATA_HOME", self.root.join("data"))
            .env(
                "OMARCHY_T3_FIXTURE_ENVIRONMENTS",
                self.root.join("env.json"),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = c.stdin.take().unwrap();
        let stdout = c.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    let _ = tx.send(v);
                }
            }
        });
        (c, stdin, rx)
    }
    fn wait(rx: &mpsc::Receiver<Value>, status: &str) {
        loop {
            let v = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            if v["type"] == "compaction"
                && v["data"]["key"] == "fixture/thread"
                && v["data"]["status"] == status
            {
                break;
            }
        }
    }
    fn end(mut c: Child, mut input: ChildStdin) {
        writeln!(input, "{}", json!({"op":"quit"})).unwrap();
        c.wait().unwrap();
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.server.take().unwrap().join().unwrap();
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn preventive_once_survives_worker_restart_and_uses_estimated_provider_ttl() {
    let h = Harness::new("ok");
    let (c, mut i, rx) = h.start();
    Harness::wait(&rx, "accepted");
    for _ in 0..3 {
        writeln!(i, "{}", json!({"op":"refresh"})).unwrap();
    }
    thread::sleep(Duration::from_millis(200));
    Harness::end(c, i);
    assert_eq!(h.commands.lock().unwrap().len(), 1);
    let ledger: Value = serde_json::from_slice(
        &fs::read(h.root.join("state/omarchy-t3-code/compaction/ledger.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(ledger["fixture/thread"]["estimated"], true);
    let (c, i, rx) = h.start();
    Harness::wait(&rx, "accepted");
    Harness::end(c, i);
    assert_eq!(h.commands.lock().unwrap().len(), 1);
    let path = h.root.join("state/omarchy-t3-code/compaction/ledger.json");
    let mut crash: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    crash["fixture/thread"]["status"] = json!("claimed");
    fs::write(&path, crash.to_string()).unwrap();
    let (c, i, rx) = h.start();
    Harness::wait(&rx, "uncertain");
    Harness::end(c, i);
    assert_eq!(h.commands.lock().unwrap().len(), 1);
}
#[test]
fn uncertain_dispatch_is_never_automatically_repeated() {
    let h = Harness::new("uncertain");
    let (c, i, rx) = h.start();
    Harness::wait(&rx, "uncertain");
    Harness::end(c, i);
    let (c, i, rx) = h.start();
    Harness::wait(&rx, "uncertain");
    Harness::end(c, i);
    assert_eq!(h.commands.lock().unwrap().len(), 1);
}
#[test]
fn revalidation_prevents_compacting_new_active_work() {
    let h = Harness::new("busy");
    let (c, i, rx) = h.start();
    Harness::wait(&rx, "busy");
    Harness::end(c, i);
    assert!(h.commands.lock().unwrap().is_empty());
}
#[test]
fn server_side_idle_compaction_is_never_doubled() {
    let h = Harness::new("server");
    let (c, i, rx) = h.start();
    Harness::wait(&rx, "server");
    Harness::end(c, i);
    assert!(h.commands.lock().unwrap().is_empty());
}
#[test]
fn native_queued_race_cancels_only_own_maintenance_run() {
    let h = Harness::new("queued");
    let (c, i, rx) = h.start();
    Harness::wait(&rx, "cancelled");
    Harness::end(c, i);
    let commands = h.commands.lock().unwrap();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[1]["type"], "queued-run.cancel");
    assert_eq!(commands[1]["runId"], "our-queued");
}

#[test]
fn scheduled_deadline_wakes_before_the_normal_idle_poll() {
    let h = Harness::new("scheduled");
    let start = std::time::Instant::now();
    let (c, i, rx) = h.start();
    Harness::wait(&rx, "scheduled");
    Harness::wait(&rx, "accepted");
    Harness::end(c, i);
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(h.commands.lock().unwrap().len(), 1);
}

#[test]
fn expired_scheduled_entry_becomes_terminal_without_repeated_context_reads() {
    let h = Harness::new("expired");
    let dir = h.root.join("state/omarchy-t3-code/compaction");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("ledger.json"),
        json!({"fixture/thread":{"status":"scheduled","episode":"old","dueAt":1}}).to_string(),
    )
    .unwrap();
    let (c, mut i, rx) = h.start();
    Harness::wait(&rx, "expired");
    for _ in 0..3 {
        writeln!(i, "{}", json!({"op":"refresh"})).unwrap();
    }
    thread::sleep(Duration::from_millis(200));
    Harness::end(c, i);
    assert_eq!(*h.reads.lock().unwrap(), 1);
    assert!(h.commands.lock().unwrap().is_empty());
}

#[test]
fn settled_snapshot_never_reads_context_or_dispatches() {
    let h = Harness::new("settled");
    // A previously scheduled deadline must be retired even without a context read.
    let dir = h.root.join("state/omarchy-t3-code/compaction");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("ledger.json"),json!({"fixture/thread":{"status":"scheduled","episode":"old","dueAt":chrono::Utc::now().timestamp_millis()+1000}}).to_string()).unwrap();
    let (c, input, rx) = h.start();
    Harness::wait(&rx, "unavailable");
    thread::sleep(Duration::from_millis(150));
    assert_eq!(*h.reads.lock().unwrap(), 0);
    assert!(h.commands.lock().unwrap().is_empty());
    let ledger: Value =
        serde_json::from_slice(&fs::read(dir.join("ledger.json")).unwrap()).unwrap();
    assert_eq!(ledger["fixture/thread"]["status"], "unavailable");
    Harness::end(c, input);
}
#[test]
fn freshly_settled_thread_cannot_be_awakened_by_compaction() {
    let h = Harness::new("settled-race");
    let (c, input, rx) = h.start();
    Harness::wait(&rx, "unavailable");
    assert_eq!(*h.reads.lock().unwrap(), 2);
    assert!(h.commands.lock().unwrap().is_empty());
    Harness::end(c, input);
}

fn memory_event(rx: &mpsc::Receiver<Value>, expected: &str) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(6);
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(v) => {
                if v["type"] == "idle-release" && v["data"]["key"] == "fixture/thread" {
                    return v["data"]["status"] == expected;
                }
            }
            Err(_) => break,
        }
    }
    false
}

#[test]
fn idle_memory_releases_native_session_without_compacting_and_deduplicates_restart() {
    let h = Harness::new("memory");
    let (c, i, rx) = h.start();
    let released = memory_event(&rx, "released");
    Harness::end(c, i);
    assert!(released);
    let commands = h.commands.lock().unwrap().clone();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0]["type"], "provider-session.detach");
    assert_eq!(commands[0]["providerSessionId"], "native-session");
    assert!(commands[0].get("text").is_none());
    let (c, i, rx) = h.start();
    let released = memory_event(&rx, "released");
    Harness::end(c, i);
    assert!(released);
    assert_eq!(h.commands.lock().unwrap().len(), 1);
}
#[test]
fn idle_memory_rechecks_new_work_before_releasing() {
    let h = Harness::new("memory-busy");
    let (c, i, rx) = h.start();
    let busy = memory_event(&rx, "busy");
    Harness::end(c, i);
    assert!(busy);
    assert!(h.commands.lock().unwrap().is_empty());
}
