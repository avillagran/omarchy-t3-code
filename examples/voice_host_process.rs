//! Exercise the actual metadata facade and owned PCM/HTTP path in private Podman.
use omarchy_t3_client::{
    protocol::speech::decode_pcm_request,
    voice_capture::{
        resources::{Assets, DescriptorArtifact, Runtime},
        Cleanup,
    },
    voice_consent::ConsentOwner,
    voice_host::{Host, Resources},
    Environment,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
fn private(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}
fn children() -> BTreeSet<u32> {
    let mut result = BTreeSet::new();
    for task in fs::read_dir("/proc/self/task").unwrap() {
        if let Ok(text) = fs::read_to_string(task.unwrap().path().join("children")) {
            result.extend(text.split_whitespace().map(|id| id.parse::<u32>().unwrap()));
        }
    }
    result
}
fn isolated_processes() -> BTreeSet<u32> {
    fs::read_dir("/proc")
        .unwrap()
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| *pid != 1 && *pid != std::process::id())
        .collect()
}
fn assert_retired(baseline: &BTreeSet<u32>, budget: Duration) {
    let deadline = Instant::now() + budget;
    while (children() != *baseline || !isolated_processes().is_empty()) && Instant::now() < deadline
    {
        thread::sleep(Duration::from_millis(5));
    }
    let remaining = isolated_processes();
    for pid in &remaining {
        let root = Path::new("/proc").join(pid.to_string());
        let status = fs::read_to_string(root.join("status")).unwrap_or_default();
        let status = status
            .lines()
            .filter(|line| {
                ["Name:", "State:", "PPid:", "Uid:"]
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
            })
            .take(4)
            .collect::<Vec<_>>()
            .join(";");
        let command = fs::read_to_string(root.join("comm")).unwrap_or_default();
        let stat = fs::read_to_string(root.join("stat")).unwrap_or_default();
        let stat = stat
            .rsplit_once(')')
            .map(|(_, fields)| {
                fields
                    .split_whitespace()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        eprintln!(
            "HOST_CHILD {pid} {} {status} stat={stat}",
            command.trim().chars().take(64).collect::<String>()
        );
    }
    assert_eq!(children(), *baseline);
    assert!(
        remaining.is_empty(),
        "Unexpected retained namespace processes: {remaining:?}"
    );
}
fn environment(base: &str, id: &str) -> Environment {
    Environment {
        id: id.into(),
        name: String::new(),
        base_url: base.into(),
        token_file: PathBuf::from("/state/read-token"),
        write_token_file: Some(PathBuf::from("/state/write-token")),
        bearer: None,
        connection_error: None,
        connection: None,
    }
}
fn response(socket: &mut TcpStream, body: Value) {
    let body = body.to_string();
    write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
}
struct Peer {
    running: Arc<AtomicBool>,
    thread: thread::JoinHandle<(usize, usize)>,
    base: String,
}
impl Peer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let running = Arc::new(AtomicBool::new(true));
        let active = running.clone();
        let thread = thread::spawn(move || {
            let mut pcm_bytes = 0;
            let mut cleanup_calls = 0;
            while active.load(Ordering::Acquire) {
                let mut socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("{error}"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                    assert!(header.len() < 16384);
                }
                let header = String::from_utf8(header).unwrap();
                assert!(header
                    .to_ascii_lowercase()
                    .contains("authorization: bearer private-host-write\r\n"));
                assert!(!header.contains("private-host-read"));
                let size = header
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                    .unwrap_or(0);
                assert!(size < 1_048_576);
                let mut body = vec![0; size];
                socket.read_exact(&mut body).unwrap();
                if header.starts_with("GET /api/auth/session ") {
                    response(
                        &mut socket,
                        json!({"authenticated":true,"permissions":["orchestration:operate"]}),
                    );
                } else if header.starts_with("GET /api/voice/status ") {
                    response(
                        &mut socket,
                        json!({"supported":true,"state":"ready","modelId":"private-model","model":"Private synthetic model","size":1,"supportsStreaming":false,"supportsBufferedStreaming":false,"supportsTranslation":false,"acceleration":"cpu","modelUnloadTimeout":"never","gpuDevices":[]}),
                    );
                } else if header.starts_with("POST /api/voice/transcribe ") {
                    let (options, pcm) = decode_pcm_request(&body).unwrap();
                    assert!(!pcm.is_empty() && pcm.len().is_multiple_of(4));
                    assert!(pcm
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|bytes| f32::from_le_bytes(*bytes).is_finite()));
                    assert!(pcm
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|bytes| f32::from_le_bytes(*bytes).abs() > 0.0001));
                    assert_eq!(
                        options,
                        omarchy_t3_client::protocol::speech::TranscriptionOptions::default()
                    );
                    pcm_bytes += pcm.len();
                    response(&mut socket, json!({"text":"actual hosted transcript"}));
                } else if header.starts_with("POST /api/voice/post-process ") {
                    let body: Value = serde_json::from_slice(&body).unwrap();
                    assert_eq!(body["transcript"], "actual hosted transcript");
                    assert_eq!(
                        body["draft"],
                        json!({"text":"before ","selection":{"start":7,"end":7}})
                    );
                    cleanup_calls += 1;
                    response(&mut socket, json!({"text":"clean hosted transcript"}));
                } else {
                    panic!("Unexpected private speech route");
                }
            }
            (pcm_bytes, cleanup_calls)
        });
        Self {
            running,
            thread,
            base,
        }
    }
    fn close(self) -> (usize, usize) {
        self.running.store(false, Ordering::Release);
        self.thread.join().unwrap()
    }
}
struct Harness {
    host: Host,
    events: mpsc::Receiver<Value>,
    backlog: VecDeque<Value>,
    next: u64,
}
impl Harness {
    fn send(&mut self, action: &str, session: &str, fields: Value) -> String {
        self.next += 1;
        let id = format!("host-request-{}", self.next);
        self.send_id(&id, action, session, fields);
        id
    }
    fn send_id(&self, id: &str, action: &str, session: &str, fields: Value) {
        eprintln!("HOST_ACTION {action} {id}");
        let mut input = json!({"action":action,"requestId":id,"session":session});
        input
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        self.host.request(&input);
    }
    fn wait(&mut self, predicate: impl Fn(&Value) -> bool) -> Value {
        if let Some(index) = self.backlog.iter().position(&predicate) {
            return self.backlog.remove(index).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(18);
        loop {
            let value = self
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("Actual Host terminal/event deadline");
            eprintln!(
                "HOST_EVENT {} {} {}",
                value["kind"].as_str().unwrap_or("invalid"),
                value["requestId"].as_str().unwrap_or("-"),
                value["error"].as_str().unwrap_or("-")
            );
            if predicate(&value) {
                return value;
            }
            assert!(
                value["kind"] != "result" || value["error"].is_null(),
                "Unexpected Host terminal {}: {}",
                value["requestId"],
                value["error"]
            );
            if value["kind"] != "level" {
                assert!(self.backlog.len() < 1024);
                self.backlog.push_back(value);
            }
        }
    }
    fn result(&mut self, id: &str) -> Value {
        self.wait(|value| value["kind"] == "result" && value["requestId"] == id)
    }
    fn call(&mut self, action: &str, session: &str, fields: Value) -> Value {
        let id = self.send(action, session, fields);
        let result = self.result(&id);
        assert!(result["error"].is_null(), "{action}: {}", result["error"]);
        result["value"].clone()
    }
    fn owner(&self, key: &str, epoch: u64) {
        self.host.owner_changed(key, "draft", epoch);
    }
    fn begin(&mut self, grant: &str) -> String {
        self.call(
            "begin",
            "",
            json!({"ownerKey":"draft-owner","grantIdJson":grant}),
        )["session"]
            .as_str()
            .unwrap()
            .into()
    }
    fn configure(&mut self, session: &str, cleanup: &str) {
        self.call(
            "configure",
            session,
            json!({"optionsJson":options().to_string(),"cleanupGrantJson":cleanup}),
        );
    }
}
fn options() -> Value {
    json!({"speechLanguage":"auto","speechTranslateToEnglish":false,"speechCustomWords":[],"speechRemoveFillerWords":true,"speechSilenceFiltering":true})
}
fn main() {
    assert!(Path::new("/run/.containerenv").exists());
    assert_ne!(
        std::process::id(),
        1,
        "Private runtime requires an init reaper"
    );
    assert!(isolated_processes().is_empty());
    let baseline = children();
    fs::create_dir_all("/tmp/.X11-unix").unwrap();
    private(Path::new("/state/runtime"));
    std::env::set_var("XDG_RUNTIME_DIR", "/state/runtime");
    std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/state/no-bus");
    for (name, value) in [
        ("read-token", "private-host-read"),
        ("write-token", "private-host-write"),
    ] {
        let path = Path::new("/state").join(name);
        fs::write(&path, value).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let runtime = Arc::new(Runtime::admit(Path::new("/electron")).unwrap());
    let path = PathBuf::from("/source/electron-private-fd.node");
    let addon = DescriptorArtifact {
        bytes: fs::metadata(&path).unwrap().len(),
        sha256: format!("{:x}", Sha256::digest(fs::read(&path).unwrap())),
        path,
    };
    private(Path::new("/state/profile"));
    private(Path::new("/state/namespace"));
    let peer = Peer::start();
    let base = peer.base.clone();
    let armed = Arc::new(AtomicBool::new(false));
    let gate = armed.clone();
    let (seen, seen_rx) = mpsc::sync_channel(1);
    let (release, release_rx) = mpsc::sync_channel(1);
    let release_rx = Mutex::new(release_rx);
    let (events, receiver) = mpsc::sync_channel(1024);
    let resources = Resources::new(
        runtime.clone(),
        &addon,
        Path::new("/state/profile"),
        Path::new("/state/namespace"),
    )
    .unwrap()
    .with_synthetic_device()
    .unwrap();
    let host = Host::new(
        resources,
        move |id| {
            if id != "chosen" && id != "draft" {
                return Err("private_environment_unavailable".into());
            }
            if gate.swap(false, Ordering::AcqRel) {
                seen.send(()).unwrap();
                release_rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
            }
            Ok(environment(&base, id))
        },
        move |event| {
            events
                .try_send(event)
                .expect("Bounded synthetic Host event sink")
        },
    );
    let mut h = Harness {
        host,
        events: receiver,
        backlog: VecDeque::new(),
        next: 0,
    };
    h.host.ready();
    h.wait(|v| v["kind"] == "ready" && v["available"] == true);
    h.owner("draft-owner", 1);
    h.call(
        "choose-audio",
        "",
        json!({"destinationJson":r#"{"kind":"environment","environmentId":"chosen"}"#}),
    );
    let old_ack=h.call("pending-cloud","",json!({"destinationJson":r#"{"kind":"cloud","provider":"openai","environmentId":"chosen"}"#}))["acknowledgement"].as_str().unwrap().to_owned();
    let choice = h.call(
        "choose-audio",
        "",
        json!({"destinationJson":r#"{"kind":"environment","environmentId":"chosen"}"#}),
    );
    let grant = serde_json::from_str::<Value>(choice["choiceJson"].as_str().unwrap()).unwrap()
        ["id"]
        .to_string();
    let cleanup = h.call("choose-cleanup", "", json!({"enabled":true}))["cleanupJson"]
        .as_str()
        .unwrap()
        .to_owned();
    let initial_change =
        h.wait(|value| value["kind"] == "consent" && value["value"]["cleanupJson"] == cleanup);
    let initial_generation = initial_change["generation"].as_u64().unwrap();
    // Cancel a real Chromium permission prompt while capture-prepare is deferred.
    let cancelled = h.begin(&grant);
    h.configure(&cancelled, &cleanup);
    let pending = h.send(
        "capture-prepare",
        &cancelled,
        json!({"mode":"buffered","microphoneId":""}),
    );
    h.wait(|v| v["kind"] == "permission" && v["session"] == cancelled);
    h.send_id(&pending, "snapshot", "", json!({}));
    let duplicate = h.result(&pending);
    assert_eq!(duplicate["error"], "voice_operation_busy");
    eprintln!("HOST_CANCEL_DEFERRED");
    h.host
        .request(&json!({"action":"abort-request","target":pending}));
    let terminal = h.result(&pending);
    assert_eq!(terminal["error"], "voice_cancelled");
    h.call("release-capture", &cancelled, json!({}));
    h.send_id(&pending, "snapshot", "", json!({}));
    assert!(h.result(&pending)["error"].is_null());
    h.call("dispose", &cancelled, json!({}));
    // Successful reacquisition proves the deferred cancellation released capture.
    let session = h.begin(&grant);
    h.configure(&session, &cleanup);
    let status = h.call("status", &session, json!({}));
    assert_eq!(
        serde_json::from_str::<Value>(status["valueJson"].as_str().unwrap()).unwrap()["supported"],
        true
    );
    h.call("release-capture", &session, json!({}));
    let prepare = h.send(
        "capture-prepare",
        &session,
        json!({"mode":"buffered","microphoneId":""}),
    );
    let permission = h.wait(|v| v["kind"] == "permission" && v["session"] == session);
    h.call(
        "permission",
        &session,
        json!({"permissionId":permission["permissionId"],"allow":true}),
    );
    assert!(h.result(&prepare)["error"].is_null());
    h.call("record", &session, json!({"seconds":5.0}));
    h.wait(|v| {
        v["kind"] == "level"
            && v["session"] == session
            && v["value"].as_f64().is_some_and(|n| n > 0.0)
    });
    let acknowledged = h.call("acknowledge-cloud", "", json!({"acknowledgement":old_ack}));
    assert_eq!(
        serde_json::from_str::<Value>(acknowledged["choiceJson"].as_str().unwrap()).unwrap()["id"]
            .to_string(),
        grant
    );
    h.wait(|value| {
        value["kind"] == "level"
            && value["session"] == session
            && value["value"].as_f64().is_some_and(|level| level > 0.0)
    });
    let stopped = h.call("stop", &session, json!({}));
    let uri = stopped["uri"]
        .as_str()
        .expect("Actual buffered recording URI")
        .to_owned();
    h.call("release-capture", &session, json!({}));
    let text = h.call("transcribe", &session, json!({"uri":uri}));
    assert_eq!(text["text"], "actual hosted transcript");
    let input = json!({"transcript":"actual hosted transcript","draft":{"text":"before ","selection":{"start":7,"end":7}},"options":{"speechCustomWords":[],"speechPostProcessingPrompt":{"mode":"default","customInstructions":""}}});
    assert_eq!(
        h.call(
            "post-process",
            &session,
            json!({"inputJson":input.to_string()})
        )["text"],
        "clean hosted transcript"
    );
    h.call("delete-recording", &session, json!({"uri":uri}));
    h.call("dispose", &session, json!({}));
    // The real resolver is suspended after capturing owner A; A->B->A may not
    // make that old begin appear newly admitted, even with a matching owner key.
    armed.store(true, Ordering::Release);
    let late = h.send(
        "begin",
        "",
        json!({"ownerKey":"draft-owner","grantIdJson":grant}),
    );
    seen_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    h.owner("other-owner", 2);
    h.owner("draft-owner", 3);
    release.send(()).unwrap();
    assert_eq!(h.result(&late)["error"], "voice_owner_changed");
    h.host.owner_changed("draft-owner", "missing-draft", 4);
    let defaults = h.call("project-context", "", json!({"projectId":null}));
    assert_eq!(
        serde_json::from_str::<Value>(defaults["valueJson"].as_str().unwrap()).unwrap()
            ["speechProjectCustomWords"],
        json!([])
    );
    h.owner("draft-owner", 5);
    // Another original producer writes the same private namespace. No Host RPC
    // polls for it: the persistent reader must deliver its unsolicited snapshot.
    let assets = Arc::new(Assets::consent(Path::new("/state/observer-assets"), &addon).unwrap());
    let writer = ConsentOwner::launch(
        runtime.clone(),
        assets,
        Path::new("/state/observer"),
        Path::new("/state/namespace"),
    )
    .unwrap();
    writer.choose_cleanup(false).unwrap();
    let changed = h.wait(|v| {
        v["kind"] == "consent"
            && v["value"]["cleanupJson"] == "null"
            && v["generation"]
                .as_u64()
                .is_some_and(|generation| generation > initial_generation)
    });
    assert!(changed["generation"].as_u64().is_some_and(|n| n > 0));
    assert_eq!(writer.close(), Cleanup::Confirmed);
    drop(writer);
    assert!(h.host.shutdown().is_ok());
    drop(h);
    assert_retired(&baseline, Duration::from_secs(8));
    // Drop the LAST facade while its real microphone permission is pending.
    // No explicit shutdown or retained Host clone can conceal callback cycles.
    private(Path::new("/state/drop-profile"));
    private(Path::new("/state/drop-namespace"));
    let resources = Resources::new(
        runtime,
        &addon,
        Path::new("/state/drop-profile"),
        Path::new("/state/drop-namespace"),
    )
    .unwrap()
    .with_synthetic_device()
    .unwrap();
    let base = peer.base.clone();
    let (sender, events) = mpsc::sync_channel(1024);
    let host = Host::new(
        resources,
        move |id| Ok(environment(&base, id)),
        move |event| sender.try_send(event).expect("Bounded drop fixture events"),
    );
    let mut dropping = Harness {
        host,
        events,
        backlog: VecDeque::new(),
        next: 0,
    };
    dropping.host.ready();
    dropping.wait(|v| v["kind"] == "ready" && v["available"] == true);
    dropping.owner("draft-owner", 1);
    let choice = dropping.call(
        "choose-audio",
        "",
        json!({"destinationJson":r#"{"kind":"environment","environmentId":"chosen"}"#}),
    );
    let grant = serde_json::from_str::<Value>(choice["choiceJson"].as_str().unwrap()).unwrap()
        ["id"]
        .to_string();
    let session = dropping.begin(&grant);
    dropping.configure(&session, "null");
    dropping.send(
        "capture-prepare",
        &session,
        json!({"mode":"buffered","microphoneId":""}),
    );
    dropping.wait(|v| v["kind"] == "permission" && v["session"] == session);
    assert_ne!(children(), baseline);
    drop(dropping);
    assert_retired(&baseline, Duration::from_secs(12));
    let (pcm_bytes, cleanup_calls) = peer.close();
    assert!(pcm_bytes > 0);
    assert_eq!(cleanup_calls, 1);
    assert_retired(&baseline, Duration::from_secs(5));
    println!(
        "{}",
        json!({"actualHost":true,"actualSyntheticPcmBytes":pcm_bytes,"cleanupHttpCalls":cleanup_calls,"deferredPermissionCancelledAndReacquired":true,"duplicateKeptOriginalRequest":true,"requestIdReusedAfterTerminal":true,"neverOwnedReleaseConfirmed":true,"ownerABARefusedBeforeSessionPublication":true,"idleConsentChangeWithoutHostRpc":true,"shutdownConfirmedAndChildrenReaped":true,"dropWhilePermissionPendingRetiresChildren":true,"staleCloudAckPreservesActualRecording":true,"missingDraftContextUsesOriginalDefault":true,"allOtherNamespaceProcessesGone":true,"containerInitReapsOrphans":true,"scope":"Actual Host facade, original consent/capture and SDK with synthetic microphone/private HTTP; no Qt/watch IPC or live device claim"})
    );
}
