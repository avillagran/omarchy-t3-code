use omarchy_t3_client::{protocol, quota_sources as quotas, Environment};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};
fn limits() -> Value {
    json!({"checkedAt":"2026-10-18T12:00:00Z","credentialFingerprint":"same-key","windows":[{"id":"monthly","kind":"monthly","label":"Monthly","usedPercent":30,"resetsAt":"2026-11-01T00:00:00Z"}],"secret":"PRIVATE"})
}
fn provider() -> Value {
    json!({"instanceId":"custom-go","driver":"opencode","enabled":true,"installed":true,"auth":{"status":"authenticated","email":"account@example.test","accessToken":"PRIVATE"},"usageLimits":limits(),"environment":{"OPENCODE_API_KEY":"PRIVATE"},"models":[{"id":"PRIVATE"}]})
}
#[test]
fn public_projection_is_forward_compatible_and_drops_credentials() {
    let out = quotas::providers(
        &json!([provider(),{"instanceId":"future","driver":"future-driver","enabled":true,"installed":true,"usageLimits":limits()}]),
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[1]["driver"], "future-driver");
    assert_eq!(out[0]["usageLimits"]["windows"][0]["kind"], "monthly");
    assert!(!json!(out).to_string().contains("PRIVATE"));
    let raw = json!([{"id":"hub","label":"Hub","managementKey":"PRIVATE","accounts":[{"id":"one","driver":"codex","email":"same@example.test","workspaceId":"workspace","usageLimits":limits(),"accessToken":"PRIVATE"}],"error":"PRIVATE"}]);
    let sources = quotas::sources(&raw);
    assert_eq!(sources[0]["accounts"][0]["workspaceId"], "workspace");
    assert!(!json!(sources).to_string().contains("PRIVATE"));
    let mut invalid = limits();
    invalid["windows"] = json!([{"id":"bad","kind":"weekly","label":"Bad","usedPercent":101},{"id":"ok","kind":"other","label":"Other","usedPercent":0,"resetsAt":"bad"},{"id":"future","kind":"future-kind","label":"Future","usedPercent":0}]);
    let safe = quotas::limits(&invalid).unwrap();
    assert_eq!(safe["windows"].as_array().unwrap().len(), 1);
    assert!(safe["windows"][0].get("resetsAt").is_none());
    invalid["externalUsage"] = json!({"label":"Usage","url":"https://secret@example.test/path"});
    assert!(quotas::limits(&invalid)
        .unwrap()
        .get("externalUsage")
        .is_none());
}
#[test]
fn snapshots_replace_sources_and_unrelated_events_do_not_mutate_state() {
    let mut state = json!({"usageLimitSources":[{"id":"old"}]});
    assert!(!quotas::apply(
        &mut state,
        &json!({"version":1,"type":"settingsUpdated","payload":{"secret":"PRIVATE"}})
    ));
    assert!(quotas::apply(
        &mut state,
        &json!({"version":1,"type":"snapshot","config":{"providers":[provider()]}})
    ));
    assert!(state["usageLimitSources"].as_array().unwrap().is_empty());
    assert!(quotas::apply(
        &mut state,
        &json!({"version":1,"type":"usageLimitSourcesUpdated","payload":{"sources":[{"id":"hub","accounts":[]}]}})
    ));
    assert_eq!(state["usageLimitSources"][0]["id"], "hub");
    assert!(quotas::apply(
        &mut state,
        &json!({"version":1,"type":"providerStatuses","payload":{"providers":[]}})
    ));
    assert!(state["providers"].as_array().unwrap().is_empty());
    assert_eq!(state["usageLimitSources"].as_array().unwrap().len(), 1);
}
#[test]
fn actual_native_stream_opts_into_hub_replay_and_projects_before_callback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 8192];
        let n = stream.read(&mut request).unwrap();
        assert!(String::from_utf8_lossy(&request[..n]).starts_with("GET /api/auth/session "));
        let body = json!({"authenticated":true,"permissions":["orchestration:read"]}).to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        drop(stream);
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        let request: Value =
            serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(request["tag"], "subscribeServerConfig");
        assert_eq!(request["payload"], json!({"usageLimitSources":true}));
        let events = json!([{"version":1,"type":"snapshot","config":{"providers":[provider()],"settings":{"secret":"PRIVATE"}}},{"version":1,"type":"settingsUpdated","payload":{"secret":"PRIVATE"}},{"version":1,"type":"usageLimitSourcesUpdated","payload":{"sources":[{"id":"hub","label":"Hub","accounts":[{"id":"account","driver":"codex","usageLimits":limits(),"key":"PRIVATE"}]}]}}]);
        socket
            .send(tungstenite::Message::Text(
                json!({"_tag":"Chunk","requestId":"1","values":events})
                    .to_string()
                    .into(),
            ))
            .unwrap();
        loop {
            match socket.read() {
                Ok(tungstenite::Message::Close(_)) | Err(_) => break,
                Ok(tungstenite::Message::Text(t)) if t.contains("Interrupt") => break,
                _ => {}
            }
        }
    });
    let e = Environment {
        id: "host".into(),
        name: "Host".into(),
        base_url: base.clone(),
        token_file: Default::default(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(
            protocol::Connection::new(
                &base,
                None,
                protocol::Credential::bearer("synthetic-token-value").unwrap(),
            )
            .unwrap(),
        )),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let mut events = vec![];
    quotas::subscribe(
        &e,
        || cancel.load(Ordering::Relaxed),
        |event| {
            if let Some(event) = event {
                assert!(!event.to_string().contains("PRIVATE"));
                if event["type"] == "usageLimitSourcesUpdated" {
                    cancel.store(true, Ordering::Relaxed)
                }
                events.push(event);
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(events.len(), 2);
    server.join().unwrap();
}

#[test]
fn denied_session_never_opens_a_subscription() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        s.read(&mut request).unwrap();
        let body = r#"{"authenticated":true,"permissions":[]}"#;
        write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        drop(s);
        listener.set_nonblocking(true).unwrap();
        thread::sleep(std::time::Duration::from_millis(100));
        assert!(listener.accept().is_err());
    });
    let e = Environment {
        id: "host".into(),
        name: "Host".into(),
        base_url: base.clone(),
        token_file: Default::default(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(
            protocol::Connection::new(
                &base,
                None,
                protocol::Credential::bearer("synthetic-token-value").unwrap(),
            )
            .unwrap(),
        )),
    };
    let mut called = false;
    assert_eq!(
        quotas::subscribe(
            &e,
            || false,
            |_| {
                called = true;
                Ok(())
            }
        )
        .unwrap_err(),
        "permission_denied"
    );
    assert!(!called);
    server.join().unwrap();
}
