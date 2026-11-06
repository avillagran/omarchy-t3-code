use omarchy_t3_client::{protocol, thread_details, Environment};
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

#[test]
fn explicit_terminal_paths_require_fresh_scope_and_metadata_and_keep_editor_positions() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let allow = Arc::new(AtomicBool::new(true));
    let exists = Arc::new(AtomicBool::new(true));
    let lost = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (done, granted, present, lose, log) = (
        stop.clone(),
        allow.clone(),
        exists.clone(),
        lost.clone(),
        calls.clone(),
    );
    let server = thread::spawn(move || {
        while !done.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut peek = [0; 1024];
            let n = stream.peek(&mut peek).unwrap();
            let start = String::from_utf8_lossy(&peek[..n]);
            if start.starts_with("GET /api/auth/session ")
                || start.starts_with("GET /api/orchestration/shell ")
            {
                let session = start.starts_with("GET /api/auth/session ");
                let mut header = Vec::new();
                let mut byte = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                    assert!(header.len() < 16384);
                }
                let body=if session {json!({"authenticated":true,"permissions":if granted.load(Ordering::Relaxed){vec!["orchestration:read","orchestration:operate","filesystem:read"]}else{vec!["orchestration:read","filesystem:read"]}})}else{json!({"projects":[{"id":"project","workspaceRoot":"/work/project"}],"threads":[{"id":"thread","projectId":"project","worktreePath":"/work/project/.t3/worktree"}]})}.to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                continue;
            }
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            let value = match request["tag"].as_str().unwrap() {
                "server.getConfig" => json!({"availableEditors":["zed","vscode"]}),
                "filesystem.getMetadata" => {
                    log.lock().unwrap().push(request.clone());
                    json!({"entries":[if present.load(Ordering::Relaxed){json!({"kind":"file","private":"PRIVATE","path":request["payload"]["paths"][0]})}else{Value::Null}]})
                }
                "shell.openInEditor" => {
                    log.lock().unwrap().push(request.clone());
                    if lose.load(Ordering::Relaxed) {
                        continue;
                    }
                    Value::Null
                }
                tag => panic!("unexpected RPC {tag}"),
            };
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":request["id"],"exit":{"_tag":"Success","value":value}}).to_string().into())).unwrap();
        }
    });
    struct Peer(Arc<AtomicBool>, Option<thread::JoinHandle<()>>);
    impl Drop for Peer {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
            if let Some(thread) = self.1.take() {
                let _ = thread.join();
            }
        }
    }
    let _peer = Peer(stop, Some(server));
    let connection = protocol::Connection::new(
        &base,
        None,
        protocol::Credential::bearer("synthetic-terminal-link-token").unwrap(),
    )
    .unwrap();
    let environment = Environment {
        id: "fixture".into(),
        name: "Fixture".into(),
        base_url: base,
        token_file: Default::default(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(connection)),
    };
    for (target, path) in [
        (
            "/work/project/src/main.rs:12:7",
            "/work/project/src/main.rs",
        ),
        (
            r"C:\Users\Jane\project\main.ts:9:3",
            r"C:\Users\Jane\project\main.ts",
        ),
        ("~/project/main.rs:1", "~/project/main.rs"),
        ("/work/project/file:١٢", "/work/project/file:١٢"),
    ] {
        let command = json!({"thread":"thread","target":target,"editor":"zed"});
        let read = thread_details::read(&environment, &command).unwrap();
        assert_eq!(
            read["terminalLink"],
            json!({"target":target,"path":path,"kind":"file"})
        );
        assert!(!read.to_string().contains("PRIVATE"));
        let opened = thread_details::open(&environment, &command).unwrap();
        assert_eq!(opened["target"], target);
        let log = calls.lock().unwrap();
        let last = log.last().unwrap();
        assert_eq!(last["tag"], "shell.openInEditor");
        assert_eq!(last["payload"], json!({"cwd":target,"editor":"zed"}));
        let metadata = &log[log.len() - 2];
        assert_eq!(metadata["payload"], json!({"paths":[path]}));
    }
    let opens = || {
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call["tag"] == "shell.openInEditor")
            .count()
    };
    let before = opens();
    let command = json!({"thread":"thread","target":"/work/project/gone.rs:3","editor":"zed"});
    thread_details::read(&environment, &command).unwrap();
    exists.store(false, Ordering::Relaxed);
    assert_eq!(
        thread_details::open(&environment, &command).unwrap_err(),
        "terminal_link_unavailable"
    );
    assert_eq!(
        opens(),
        before,
        "a vanished path must not open after a hover read"
    );
    exists.store(true, Ordering::Relaxed);
    allow.store(false, Ordering::Relaxed);
    assert_eq!(
        thread_details::open(&environment, &command).unwrap_err(),
        "permission_denied"
    );
    assert_eq!(opens(), before);
    allow.store(true, Ordering::Relaxed);
    assert_eq!(
        thread_details::open(
            &environment,
            &json!({"thread":"missing","target":"/work/project/file","editor":"zed"})
        )
        .unwrap_err(),
        "unknown_thread"
    );
    assert_eq!(opens(), before);
    thread_details::open(&environment, &json!({"thread":"thread","editor":"vscode"})).unwrap();
    assert_eq!(
        calls.lock().unwrap().last().unwrap()["payload"],
        json!({"cwd":"/work/project/.t3/worktree","editor":"vscode"})
    );
    let before = opens();
    lost.store(true, Ordering::Relaxed);
    assert!(thread_details::open(&environment, &command).is_err());
    assert_eq!(
        opens(),
        before + 1,
        "an uncertain editor-open must not retry automatically"
    );
}
