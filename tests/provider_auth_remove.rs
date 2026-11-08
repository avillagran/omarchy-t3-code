//! Actual provider-setup consumer with a synthetic permission/RPC transport peer.
use omarchy_t3_client::{protocol, provider_setup, Environment};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[test]
fn removal_refreshes_authorization_and_sends_the_exact_selected_method_once() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let peer = thread::spawn(move || {
        let mut requests = Vec::new();
        // A later permission snapshot revokes manage access on this same owner.
        for allowed in [true, false] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buffer = [0; 4096];
            let size = stream.read(&mut buffer).unwrap();
            assert!(String::from_utf8_lossy(&buffer[..size]).starts_with("GET /api/auth/session "));
            let body = json!({"authenticated":true,"permissions":if allowed {vec!["providers:manage"]} else {vec!["orchestration:read"]}}).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            drop(stream);
            if allowed {
                let (stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut socket = tungstenite::accept(stream).unwrap();
                let request: Value =
                    serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
                requests.push(request.clone());
                socket.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":request["id"],"exit":{"_tag":"Success","value":{"instanceId":"codex_work","phase":"idle","methods":[]}}}).to_string().into())).unwrap();
            }
        }
        listener.set_nonblocking(true).unwrap();
        let until = Instant::now() + Duration::from_millis(300);
        while Instant::now() < until {
            match listener.accept() {
                Ok(_) => panic!("Denied removal opened another connection"),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("{error}"),
            }
        }
        requests
    });
    let connection = protocol::Connection::new(
        &base,
        None,
        protocol::Credential::bearer("synthetic-token").unwrap(),
    )
    .unwrap();
    let environment = Environment {
        id: "node".into(),
        name: "NODE".into(),
        base_url: base,
        token_file: "/unused".into(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(connection)),
    };
    let input = json!({"action":"remove-auth-method","instanceId":" \u{feff}codex_work\u{a0}","methodId":" profile:a\0b ","canRemove":true});
    let result = provider_setup::run(&environment, &input).unwrap();
    assert_eq!(
        result,
        json!({"instanceId":"codex_work","kind":"auth","state":{"instanceId":"codex_work","phase":"idle","methods":[]}})
    );
    assert_eq!(
        provider_setup::run(&environment, &input).unwrap_err(),
        "permission_denied"
    );
    let requests = peer.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["tag"], "provider.auth.removeMethod");
    assert_eq!(
        requests[0]["payload"],
        json!({"instanceId":"codex_work","methodId":"profile:a\0b"})
    );
}

#[test]
fn reconnect_forwards_the_present_native_method_and_never_defaults_invalid_input() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let peer = thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..3 {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
            socket.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":request["id"],"exit":{"_tag":"Success","value":{"phase":"waiting"}}}).to_string().into())).unwrap();
            requests.push(request);
        }
        requests
    });
    let connection = protocol::Connection::new(
        &base,
        None,
        protocol::Credential::bearer("synthetic-token").unwrap(),
    )
    .unwrap();
    let environment = Environment {
        id: "node".into(),
        name: "NODE".into(),
        base_url: base,
        token_file: "/unused".into(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(connection)),
    };
    // Invalid present values are rejected before any socket/optional fallback.
    for method in [
        Value::Null,
        json!(7),
        json!(" \u{feff}\u{a0}"),
        json!("😀".repeat(65)),
    ] {
        assert_eq!(
            provider_setup::run(
                &environment,
                &json!({"action":"login","instanceId":"codex","methodId":method})
            )
            .unwrap_err(),
            "invalid_method_id"
        );
    }
    let selected = format!("chatgpt-profile:{}", "😀".repeat(50));
    for method in [&selected, "\u{feff}profile:a\0\nb\u{a0}"] {
        provider_setup::run(
            &environment,
            &json!({"action":"login","instanceId":"codex","methodId":method}),
        )
        .unwrap();
    }
    provider_setup::run(
        &environment,
        &json!({"action":"login","instanceId":"codex"}),
    )
    .unwrap();
    let requests = peer.join().unwrap();
    assert!(requests
        .iter()
        .all(|request| request["tag"] == "provider.auth.start"));
    assert_eq!(
        requests[0]["payload"],
        json!({"instanceId":"codex","methodId":selected})
    );
    assert_eq!(
        requests[1]["payload"],
        json!({"instanceId":"codex","methodId":"profile:a\0\nb"})
    );
    assert_eq!(requests[2]["payload"], json!({"instanceId":"codex"}));
}
