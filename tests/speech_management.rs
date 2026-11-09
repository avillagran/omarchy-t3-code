use omarchy_t3_client::write;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
fn request(socket: &mut TcpStream) -> (String, Value) {
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut bytes = vec![];
    let mut byte = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        socket.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() < 65536);
    }
    let header = String::from_utf8(bytes).unwrap();
    let length = header
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>().unwrap())
        .unwrap_or(0);
    assert!(length <= 128000);
    let mut body = vec![0; length];
    socket.read_exact(&mut body).unwrap();
    (
        header,
        if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap()
        },
    )
}
struct Server {
    stop: Arc<AtomicBool>,
    task: Option<thread::JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(task) = self.task.take() {
            let result = task.join();
            if !thread::panicking() {
                result.unwrap();
            }
        }
    }
}
#[test]
fn actual_dispatch_uses_current_write_authority_and_correlates_native_mutation_failures() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let scopes = Arc::new(AtomicU8::new(1));
    let calls = Arc::new(Mutex::new(vec![]));
    let stop = Arc::new(AtomicBool::new(false));
    let (g, c, s) = (scopes.clone(), calls.clone(), stop.clone());
    let task = thread::spawn(move || {
        while !s.load(Ordering::Acquire) {
            let mut socket = match listener.accept() {
                Ok((socket, _)) => socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(error) => panic!("{error}"),
            };
            let (header, body) = request(&mut socket);
            assert!(header
                .to_lowercase()
                .contains("authorization: bearer write-current"));
            let path = header.split_whitespace().nth(1).unwrap();
            c.lock().unwrap().push(json!({"path":path,"body":body}));
            let (status, response) = match path {
                "/api/auth/session" => {
                    let bits = g.load(Ordering::Acquire);
                    let mut permissions = vec![];
                    if bits & 1 != 0 {
                        permissions.push("orchestration:operate");
                    }
                    if bits & 2 != 0 {
                        permissions.push("settings:write");
                    }
                    (
                        "200 OK",
                        json!({"authenticated":true,"permissions":permissions}),
                    )
                }
                "/api/voice/status" => (
                    "200 OK",
                    json!({"supported":false,"reason":"Synthetic unsupported platform"}),
                ),
                "/api/voice/models?benchmarks=1" => ("200 OK", json!({"models":[]})),
                "/api/voice/models/prepare" => ("200 OK", Value::Null),
                "/api/voice/models/download" => (
                    "400 Bad Request",
                    json!({"_tag":"EnvironmentRequestInvalidError","code":"invalid_request","reason":"speech_busy","traceId":"synthetic-trace"}),
                ),
                "/api/voice/models/select" => continue,
                "/api/voice/cloud/xai/key" => {
                    assert_eq!(body, json!({"key":"synthetic-key"}));
                    ("200 OK", Value::Null)
                }
                _ => panic!("Unexpected private route {path}"),
            };
            let body = response.to_string();
            write!(socket,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    let _server = Server {
        stop,
        task: Some(task),
    };
    let root = std::env::temp_dir().join(format!("speech-management-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let read = root.join("read-token");
    let write_token = root.join("write-token");
    for (path, value) in [(&read, "read-grants-all"), (&write_token, "write-current")] {
        fs::write(path, value).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let config = root.join("environments.json");
    fs::write(&config,json!({"environments":[{"id":"voice","base_url":base,"token_file":read,"write_token_file":write_token}]}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", config);
    let invoke = |action: &str, extra: Value| {
        let mut command = json!({"op":"speech","environment":"voice","action":action});
        for (key, value) in extra.as_object().unwrap() {
            command[key] = value.clone();
        }
        write::execute(&command)
    };
    assert_eq!(
        invoke("access", json!({})).unwrap(),
        json!({"ok":true,"value":{"canOperate":true,"canConfigure":false}})
    );
    assert_eq!(
        invoke("status", json!({})).unwrap()["value"]["supported"],
        false
    );
    // The SDK JSON-content flag does not change the already selected write
    // identity, including models and explicit model preparation.
    assert_eq!(
        invoke("models", json!({"benchmarks":true})).unwrap(),
        json!({"ok":true,"value":{"models":[]}})
    );
    assert_eq!(
        invoke("prepare", json!({})).unwrap(),
        json!({"ok":true,"value":null})
    );
    assert_eq!(
        invoke("download", json!({"modelId":"synthetic"})).unwrap_err(),
        "permission_denied"
    );
    assert!(!calls
        .lock()
        .unwrap()
        .iter()
        .any(|call| call["path"] == "/api/voice/models/download"));
    scopes.store(3, Ordering::Release);
    assert_eq!(
        invoke("download", json!({"modelId":"synthetic"})).unwrap(),
        json!({"ok":false,"failure":{"code":"invalid_request","reason":"speech_busy","traceId":"synthetic-trace"}})
    );
    assert!(invoke("select", json!({"modelId":"synthetic"})).is_err());
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call["path"] == "/api/voice/models/select")
            .count(),
        1,
        "Unknown mutation must not replay"
    );
    let result = invoke(
        "cloud-key",
        json!({"provider":"xai","key":" synthetic-key "}),
    )
    .unwrap();
    assert_eq!(result, json!({"ok":true,"value":null}));
    assert!(!result.to_string().contains("synthetic-key"));
    scopes.store(0, Ordering::Release);
    assert_eq!(
        invoke("status", json!({})).unwrap_err(),
        "permission_denied"
    );
    for action in ["models", "prepare"] {
        assert_eq!(invoke(action, json!({})).unwrap_err(), "permission_denied");
    }
    let count = calls.lock().unwrap().len();
    assert_eq!(
        invoke("future", json!({})).unwrap_err(),
        "invalid_speech_action"
    );
    assert_eq!(calls.lock().unwrap().len(), count);
    // Private fixtures and credential files are retained for container teardown.
}
