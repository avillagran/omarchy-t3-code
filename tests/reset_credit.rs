use omarchy_t3_client::write;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[test]
fn actual_write_dispatch_rechecks_scope_and_never_replays_a_spent_credit() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let allowed = Arc::new(AtomicBool::new(true));
    let stop = Arc::new(AtomicBool::new(false));
    let drop_reply = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let outcome = Arc::new(Mutex::new(json!({"outcome":"reset"})));
    let (a, s, d, c, o) = (
        allowed.clone(),
        stop.clone(),
        drop_reply.clone(),
        calls.clone(),
        outcome.clone(),
    );
    let server = thread::spawn(move || {
        while !s.load(Ordering::Acquire) {
            let mut socket = match listener.accept() {
                Ok((socket, _)) => socket,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = [0; 8192];
            let end = Instant::now() + Duration::from_secs(2);
            let size = loop {
                match socket.peek(&mut bytes) {
                    Ok(n) if bytes[..n].windows(4).any(|v| v == b"\r\n\r\n") => break n,
                    Ok(0) => break 0,
                    Ok(_) => {
                        assert!(Instant::now() < end);
                        thread::sleep(Duration::from_millis(1))
                    }
                    Err(_) => break 0,
                }
            };
            if size == 0 {
                continue;
            }
            let header = String::from_utf8_lossy(&bytes[..size]);
            assert!(
                header
                    .to_ascii_lowercase()
                    .contains("authorization: bearer write-only"),
                "every scope check and mutation uses the write credential"
            );
            if header.starts_with("GET /api/auth/session ") {
                let mut consumed = Vec::new();
                let mut byte = [0];
                while !consumed.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    consumed.push(byte[0]);
                    assert!(consumed.len() < 8192)
                }
                let permissions = if a.load(Ordering::Acquire) {
                    vec!["providers:manage"]
                } else {
                    vec!["orchestration:operate"]
                };
                let value = json!({"authenticated":true,"permissions":permissions}).to_string();
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",value.len(),value).unwrap();
                continue;
            }
            let mut ws = tungstenite::accept(socket).unwrap();
            loop {
                match ws.read() {
                    Ok(tungstenite::Message::Text(text)) => {
                        let value: Value = serde_json::from_str(&text).unwrap();
                        if value["_tag"] == "Ping" {
                            ws.send(tungstenite::Message::Text(
                                json!({"_tag":"Pong"}).to_string().into(),
                            ))
                            .unwrap();
                            continue;
                        }
                        if value["_tag"] != "Request" {
                            continue;
                        }
                        c.lock().unwrap().push(value.clone());
                        assert_eq!(value["tag"], "provider.consumeResetCredit");
                        if !d.load(Ordering::Acquire) {
                            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":value["id"],"exit":{"_tag":"Success","value":o.lock().unwrap().clone()}}).to_string().into())).unwrap();
                        }
                        break;
                    }
                    Ok(tungstenite::Message::Ping(v)) => {
                        ws.send(tungstenite::Message::Pong(v)).unwrap()
                    }
                    other => panic!("{other:?}"),
                }
            }
        }
    });
    let root = std::env::temp_dir().join(format!("reset-credit-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let read = root.join("read-token");
    let write_token = root.join("write-token");
    for (path, value) in [(&read, "read-only"), (&write_token, "write-only")] {
        fs::write(path, value).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap()
    }
    let config = root.join("environments.json");
    fs::write(&config,json!({"environments":[{"id":"fixture","base_url":base,"token_file":read,"write_token_file":write_token}]}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", config);
    let access =
        || write::execute(&json!({"op":"provider-reset-credit-access","environment":"fixture"}));
    let redeem = |input| {
        write::execute(&json!({"op":"provider-reset-credit","environment":"fixture","input":input}))
    };
    assert_eq!(access().unwrap()["canManageProviders"], true);
    allowed.store(false, Ordering::Release);
    assert!(redeem(json!({"instanceId":"codex"})).is_err());
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(access().unwrap()["canManageProviders"], false);
    assert!(redeem(json!({"instanceId":""})).is_err());
    assert!(calls.lock().unwrap().is_empty());
    allowed.store(true, Ordering::Release);
    for expected in ["reset", "nothingToReset", "noCredit", "alreadyRedeemed"] {
        *outcome.lock().unwrap() = json!({"outcome":expected});
        assert_eq!(
            redeem(json!({"instanceId":"codex"})).unwrap()["outcome"],
            expected
        )
    }
    *outcome.lock().unwrap() = json!({"outcome":"reset","warning":"fixture warning"});
    assert_eq!(
        redeem(json!({"sourceId":"source","accountId":"account","creditId":"credit"})).unwrap()
            ["warning"],
        "fixture warning"
    );
    assert_eq!(
        calls.lock().unwrap().last().unwrap()["payload"],
        json!({"sourceId":"source","accountId":"account","creditId":"credit"})
    );
    let before = calls.lock().unwrap().len();
    drop_reply.store(true, Ordering::Release);
    assert!(redeem(json!({"instanceId":"codex"})).is_err());
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        calls.lock().unwrap().len(),
        before + 1,
        "an uncertain mutation is never retried"
    );
    stop.store(true, Ordering::Release);
    let _ = TcpStream::connect(base.trim_start_matches("http://"));
    server.join().unwrap();
    // Retain all private fixtures and evidence. No tree removal or symlink fixtures.
}
