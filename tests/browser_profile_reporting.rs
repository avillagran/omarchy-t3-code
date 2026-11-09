//! Actual client HTTP authentication and persistent WS transport against a fixed
//! synthetic protocol peer. This does not execute the server catalog model.
use omarchy_t3_client::{browser_profile_reporting::Control, environments, rpc_sessions};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tungstenite::Message;

struct Shared {
    operate: AtomicBool,
    browser: AtomicBool,
    reject: AtomicBool,
    revision: AtomicU64,
    calls: Mutex<Vec<Value>>,
    reporters: Mutex<HashSet<u64>>,
    stop: AtomicBool,
}
struct Peer {
    base: String,
    shared: Arc<Shared>,
    task: Option<thread::JoinHandle<()>>,
}
impl Peer {
    fn new(operate: bool, browser: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let shared = Arc::new(Shared {
            operate: AtomicBool::new(operate),
            browser: AtomicBool::new(browser),
            reject: AtomicBool::new(false),
            revision: AtomicU64::new(1),
            calls: Mutex::new(Vec::new()),
            reporters: Mutex::new(HashSet::new()),
            stop: AtomicBool::new(false),
        });
        let state = shared.clone();
        let task = thread::spawn(move || {
            let mut children = Vec::new();
            let mut sequence = 0;
            while !state.stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, _)) => {
                        sequence += 1;
                        let state = state.clone();
                        children.push(thread::spawn(move || serve(socket, state, sequence)));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("private peer accept failed: {error}"),
                }
            }
            for child in children {
                child.join().unwrap();
            }
        });
        Self {
            base,
            shared,
            task: Some(task),
        }
    }
    fn calls(&self) -> Vec<Value> {
        self.shared.calls.lock().unwrap().clone()
    }
    fn reporters(&self) -> usize {
        self.shared.reporters.lock().unwrap().len()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.task.take().unwrap().join().unwrap();
    }
}
fn serve(mut socket: TcpStream, state: Arc<Shared>, identity: u64) {
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let until = Instant::now() + Duration::from_secs(2);
    let mut head = [0u8; 8192];
    let size = loop {
        match socket.peek(&mut head) {
            Ok(size) if head[..size].windows(4).any(|part| part == b"\r\n\r\n") => break size,
            Ok(0) => return,
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) => {}
            Err(_) => return,
        }
        assert!(Instant::now() < until, "private peer header deadline");
    };
    let header = String::from_utf8_lossy(&head[..size]).to_ascii_lowercase();
    let authorization = header
        .lines()
        .filter(|line| line.starts_with("authorization:"))
        .collect::<Vec<_>>();
    assert_eq!(
        authorization,
        ["authorization: bearer synthetic-profile-report-token"]
    );
    if !header.contains("upgrade: websocket") {
        assert!(header.starts_with("get /api/auth/session "));
        let mut read = vec![0; size];
        socket.read_exact(&mut read).unwrap();
        let body =
            json!({"authenticated":true,"permissions":if state.operate.load(Ordering::Acquire) {
            vec!["preview:operate"] } else { Vec::<&str>::new() }})
            .to_string();
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        return;
    }
    let Ok(mut ws) = tungstenite::accept(socket) else {
        return;
    };
    let mut subscription = None;
    let mut sent_revision = 0;
    while !state.stop.load(Ordering::Acquire) {
        if let Some(id) = &subscription {
            let revision = state.revision.load(Ordering::Acquire);
            if revision != sent_revision {
                let event = json!({"_tag":"Chunk","requestId":id,"values":[{"version":1,"type":"snapshot",
                    "config":{"environment":{"capabilities":{"serverBrowser":state.browser.load(Ordering::Acquire)}},
                    "settings":{"revision":revision},"providers":[]}}]});
                if ws.send(Message::Text(event.to_string().into())).is_err() {
                    break;
                }
                sent_revision = revision;
            }
        }
        let frame = match ws.read() {
            Ok(Message::Text(value)) => serde_json::from_str::<Value>(&value).unwrap(),
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                continue
            }
            Err(_) => break,
        };
        if frame["_tag"] == "Ping" {
            if ws
                .send(Message::Text(json!({"_tag":"Pong"}).to_string().into()))
                .is_err()
            {
                break;
            }
        } else if frame["_tag"] == "Request" && frame["tag"] == "subscribeServerConfig" {
            subscription = Some(frame["id"].clone());
        } else if frame["_tag"] == "Request" && frame["tag"] == "preview.reportProfiles" {
            state.calls.lock().unwrap().push(frame.clone());
            let exit = if state.reject.load(Ordering::Acquire) {
                json!({"_tag":"Failure","cause":[{"_tag":"Fail","error":{"_tag":"RpcNotImplemented","message":"Fixed fixture rejection"}}]})
            } else {
                state.reporters.lock().unwrap().insert(identity);
                json!({"_tag":"Success"})
            };
            if ws
                .send(Message::Text(
                    json!({"_tag":"Exit","requestId":frame["id"],"exit":exit})
                        .to_string()
                        .into(),
                ))
                .is_err()
            {
                break;
            }
        }
    }
    state.reporters.lock().unwrap().remove(&identity);
}
fn wait(mut check: impl FnMut() -> bool, message: &str) {
    let until = Instant::now() + Duration::from_secs(5);
    while !check() {
        assert!(Instant::now() < until, "{message}");
        thread::sleep(Duration::from_millis(10));
    }
}
fn next_targets(
    control: &Control,
    events: &mpsc::Receiver<Value>,
    mut accept: impl FnMut(&Value) -> bool,
) -> Value {
    let until = Instant::now() + Duration::from_secs(6);
    loop {
        let remaining = until.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "target event deadline");
        if events.recv_timeout(remaining).unwrap()["op"] == "browser-profile-targets-ready" {
            for target in control.take_targets() {
                if accept(&target) {
                    return target;
                }
            }
        }
    }
}
fn input(target: &Value, name: &str) -> Value {
    json!({"environment":target["environment"],"sessionId":target["sessionId"],"configId":target["configId"],
        "input":{"profiles":[{"id":name,"name":name,"kind":"persistent"}],"defaultProfileId":name,"recordingFrameRate":60}})
}
#[test]
fn actual_http_authority_and_persistent_report_socket_survive_result_and_fence_replacement() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let root = std::env::temp_dir().join(format!("profile-report-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let token = root.join("token");
    fs::write(&token, "synthetic-profile-report-token").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let allowed = Peer::new(true, true);
    let denied = Peer::new(false, true);
    let unavailable = Peer::new(true, false);
    let config = root.join("environments.json");
    fs::write(&config,json!({"environments":[
        {"id":"allowed","base_url":allowed.base,"token_file":token,"write_token_file":token},
        {"id":"denied","base_url":denied.base,"token_file":token,"write_token_file":token},
        {"id":"unavailable","base_url":unavailable.base,"token_file":token,"write_token_file":token}]}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", &config);
    let (tx, rx) = mpsc::channel();
    rpc_sessions::set_events(tx.clone());
    let mut control = Control::new(tx);
    control.sync(&environments().unwrap());
    let first = next_targets(&control, &rx, |event| {
        event["environment"] == "allowed" && event["operable"] == true
    });
    control.submit(&input(&first, "work"));
    wait(
        || allowed.calls().len() == 1 && allowed.reporters() == 1,
        "persistent report completed",
    );
    assert!(rpc_sessions::session("allowed", true).unwrap().is_open());
    assert_eq!(
        allowed.reporters(),
        1,
        "unary result does not retire connection reporter"
    );
    assert!(denied.calls().is_empty() && unavailable.calls().is_empty());
    allowed.shared.reject.store(true, Ordering::Release);
    control.submit(&input(&first, "rejected"));
    wait(|| allowed.calls().len() == 2, "rejection delivered once");
    allowed.shared.revision.fetch_add(1, Ordering::AcqRel);
    let changed = next_targets(&control, &rx, |event| {
        event["environment"] == "allowed"
            && event["operable"] == true
            && event["configId"] != first["configId"]
    });
    assert_eq!(
        allowed.calls().len(),
        2,
        "RPC rejection has no automatic retry"
    );
    control.submit(&input(&first, "obsolete-config"));
    allowed.shared.reject.store(false, Ordering::Release);
    control.submit(&input(&changed, "latest"));
    wait(|| allowed.calls().len() == 3, "current config report");
    assert_eq!(allowed.calls()[2]["payload"]["defaultProfileId"], "latest");
    rpc_sessions::close_environment("allowed");
    wait(|| allowed.reporters() == 0, "actual old socket retirement");
    let replacement = next_targets(&control, &rx, |event| {
        event["environment"] == "allowed"
            && event["operable"] == true
            && event["sessionId"] != first["sessionId"]
    });
    control.submit(&input(&first, "obsolete-socket"));
    control.submit(&input(&replacement, "replacement"));
    wait(
        || allowed.calls().len() == 4 && allowed.reporters() == 1,
        "fresh physical socket report",
    );
    assert_eq!(
        allowed.calls()[3]["payload"]["defaultProfileId"],
        "replacement"
    );
    control.sync(&[]);
    rpc_sessions::close_all();
    wait(
        || allowed.reporters() == 0,
        "removed environment reporter retired",
    );
    drop(control);
    // All owned fixture files are retained for disposable container teardown.
}
