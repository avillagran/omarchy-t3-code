use omarchy_t3_client::protocol;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::Duration,
};
#[test]
fn explicit_lifecycle_deadline_survives_the_original_twenty_second_limit_and_heartbeats() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut ws = tungstenite::accept_hdr(
            socket,
            |request: &tungstenite::handshake::server::Request, response| {
                assert_eq!(request.headers()["authorization"], "Bearer synthetic");
                assert!(!request.uri().to_string().contains("synthetic"));
                Ok(response)
            },
        )
        .unwrap();
        let request: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(request["tag"], "device.configure");
        assert_eq!(
            request["payload"],
            json!({"enabled":true,"onboardingCompleted":true})
        );
        ws.send(tungstenite::Message::Ping(vec![1, 2].into()))
            .unwrap();
        assert!(matches!(ws.read().unwrap(), tungstenite::Message::Pong(_)));
        ws.send(tungstenite::Message::Text(
            json!({"_tag":"Ping"}).to_string().into(),
        ))
        .unwrap();
        let pong: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(pong["_tag"], "Pong");
        // Real idle time: the existing 12s socket/20s RPC deadline would cut setup.
        ws.get_mut()
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let start = std::time::Instant::now();
        let mut proactive_ping = false;
        while start.elapsed() < Duration::from_secs(21) {
            match ws.read() {
                Ok(tungstenite::Message::Text(text)) => {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    assert_eq!(value["_tag"], "Ping");
                    proactive_ping = true;
                    ws.send(tungstenite::Message::Text(
                        json!({"_tag":"Pong"}).to_string().into(),
                    ))
                    .unwrap();
                }
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                other => panic!("unexpected heartbeat frame: {other:?}"),
            }
        }
        assert!(proactive_ping, "the client keeps an idle native RPC alive");
        ws.send(tungstenite::Message::Text(
            json!({"_tag":"Exit","requestId":"stale","exit":{"_tag":"Success","value":"WRONG"}})
                .to_string()
                .into(),
        ))
        .unwrap();
        ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"hostStatus":"ready"}}}).to_string().into())).unwrap();
        let _ = ws.read();
    });
    let c = protocol::Connection::new(
        &base,
        None,
        protocol::Credential::bearer("synthetic").unwrap(),
    )
    .unwrap();
    assert_eq!(
        c.rpc_with_timeout(
            "device.configure",
            json!({"enabled":true,"onboardingCompleted":true}),
            Duration::from_secs(25)
        )
        .unwrap(),
        json!({"hostStatus":"ready"})
    );
    server.join().unwrap();
}
#[test]
fn timeout_interrupts_without_replay_and_invalid_deadlines_never_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let c = protocol::Connection::new(
        &base,
        None,
        protocol::Credential::bearer("synthetic").unwrap(),
    )
    .unwrap();
    assert_eq!(
        c.rpc_with_timeout("device.list", json!({}), Duration::ZERO)
            .unwrap_err(),
        "invalid_rpc_timeout"
    );
    assert_eq!(
        c.rpc_with_timeout("device.list", json!({}), Duration::from_secs(301))
            .unwrap_err(),
        "invalid_rpc_timeout"
    );
    let server = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut ws = tungstenite::accept(socket).unwrap();
        let request: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(request["_tag"], "Request");
        loop {
            let message = ws.read().unwrap();
            if let tungstenite::Message::Text(s) = message {
                let v: Value = serde_json::from_str(&s).unwrap();
                if v["_tag"] == "Interrupt" {
                    assert_eq!(v["requestId"], "1");
                    break;
                }
            }
        }
        listener.set_nonblocking(true).unwrap();
        assert!(listener.accept().is_err(), "no automatic retry connection");
    });
    assert_eq!(
        c.rpc_with_timeout(
            "device.list",
            json!({"updateTool":"hub"}),
            Duration::from_millis(140)
        )
        .unwrap_err(),
        "rpc_outcome_unknown"
    );
    server.join().unwrap();
}
#[test]
fn lifecycle_uses_existing_dpop_ticket_admission_without_exporting_bound_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut bytes = vec![];
        let mut b = [0];
        while !bytes.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut b).unwrap();
            bytes.push(b[0]);
        }
        let headers = String::from_utf8(bytes).unwrap();
        assert!(headers.starts_with("POST /api/auth/websocket-ticket "));
        assert!(headers
            .to_lowercase()
            .contains("authorization: dpop synthetic-bound"));
        assert!(headers.to_lowercase().contains("\r\ndpop: "));
        let body = json!({"ticket":"synthetic-one-use-ticket"}).to_string();
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        drop(socket);
        let (socket, _) = listener.accept().unwrap();
        let mut ws = tungstenite::accept_hdr(
            socket,
            |request: &tungstenite::handshake::server::Request, response| {
                assert!(request
                    .uri()
                    .to_string()
                    .contains("wsTicket=synthetic-one-use-ticket"));
                assert!(!request.uri().to_string().contains("synthetic-bound"));
                assert!(request.headers().get("authorization").is_none());
                Ok(response)
            },
        )
        .unwrap();
        let request: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(request["tag"], "device.list");
        ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Failure","cause":{"_tag":"AuthScopeError","private":"DO NOT RETURN"}}}).to_string().into())).unwrap();
    });
    let key = Arc::new(protocol::DpopKey::generate());
    let credential =
        protocol::Credential::dpop("synthetic-bound", key, chrono::Utc::now().timestamp() + 60)
            .unwrap();
    let c = protocol::Connection::new(&base, None, credential).unwrap();
    assert_eq!(
        c.rpc_with_timeout(
            "device.list",
            json!({"updateTool":"hub"}),
            Duration::from_secs(2)
        )
        .unwrap_err(),
        "write_not_authorized: AuthScopeError"
    );
    server.join().unwrap();
}
#[test]
fn rejections_keep_the_servers_reason_but_no_other_cause_data() {
    // Effect 4 encodes a cause as its list of reasons.
    let cause = json!([{"_tag":"Fail","error":{"_tag":"ProjectNotFound","message":"Project p-1 was not found.\nstack","request":"PRIVATE"}}]);
    let error = protocol::rejection_for_test(&cause);
    assert_eq!(error, "rpc_rejected: Project p-1 was not found.");
    assert_eq!(protocol::error_code(&error), "rpc_rejected");
    assert_eq!(protocol::rejection_for_test(&json!({"private":"PRIVATE"})), "rpc_rejected");
    assert_eq!(protocol::rejection_for_test(&json!({"_tag":"Die","defect":"boom"})), "rpc_rejected: boom");
    assert_eq!(protocol::error_code("Not a code: text"), "Not a code: text");
    assert_eq!(protocol::error_code("invalid_id"), "invalid_id");
}
#[test]
fn vendored_protocol_base_matches_the_existing_pinned_dependency_exactly() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/t3-protocol");
    let provenance: Value =
        serde_json::from_slice(&std::fs::read(root.join("UPSTREAM.json")).unwrap()).unwrap();
    assert_eq!(
        provenance["revision"],
        "1a616ab6ccb31b689f41ff6f74915e69c780b1a3"
    );
    for (name, expected) in provenance["upstreamFiles"].as_object().unwrap() {
        let mut bytes = std::fs::read(root.join(name)).unwrap();
        if name == "src/lib.rs" {
            let suffix = b"\nmod long_rpc;\n";
            assert!(bytes.ends_with(suffix));
            bytes.truncate(bytes.len() - suffix.len());
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            expected.as_str().unwrap(),
            "base source changed: {name}"
        );
    }
}
