use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    thread,
};
#[test]
fn native_approval_validates_once_and_replays_exactly_after_resolution() {
    let root = std::env::temp_dir().join(format!("t3-approvals-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    std::env::set_var("XDG_STATE_HOME", &root);
    let token = root.join("token");
    std::fs::write(&token, "synthetic-private-token").unwrap();
    std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let e = Environment {
        id: "fixture".into(),
        name: "Fixture".into(),
        base_url: format!("http://{}", listener.local_addr().unwrap()),
        token_file: token,
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: None,
    };
    let opaque = format!("acp:{}", "encoded%253A".repeat(40));
    let request = opaque.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut bytes = [0; 4096];
        let n = stream.read(&mut bytes).unwrap();
        assert!(String::from_utf8_lossy(&bytes[..n])
            .starts_with("GET /api/orchestration/threads/chat/bounded "));
        let body=json!({"projection":{"messages":[],"runtimeRequests":[{"id":request,"kind":"command","status":"pending","responseCapability":{"type":"live"}}],"turnItems":[{"type":"approval_request","requestId":request,"prompt":"cargo test"}]}}).to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        drop(stream);
        let mut first = Value::Null;
        for n in 0..2 {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let req: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(req["tag"], "orchestration.dispatchCommand");
            let c = &req["payload"];
            assert_eq!(c["type"], "runtime-request.respond");
            assert_eq!(c["requestId"], request);
            assert_eq!(c["decision"], "acceptForSession");
            assert!(c.get("answers").is_none());
            if n == 0 {
                first = c.clone()
            } else {
                assert_eq!(*c, first)
            }
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"sequence":1}}}).to_string().into())).unwrap();
        }
    });
    let mut input = json!({"op":"approval","thread":"chat","runtimeRequestId":opaque,"requestId":uuid::Uuid::new_v4().to_string(),"decision":"acceptForSession"});
    assert!(write::dispatch(&e, &input).is_ok());
    assert!(write::dispatch(&e, &input).is_ok());
    input["decision"] = json!("decline");
    assert_eq!(
        write::dispatch(&e, &input).unwrap_err(),
        "retry_payload_changed"
    );
    server.join().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn approvals_reach_chat_and_disappear_on_live_resolution() {
    let mut d = json!({"projection":{"messages":[],"runtimeRequests":[{"id":"r","kind":"file-read","status":"pending","responseCapability":{"type":"live"}}],"turnItems":[{"type":"approval_request","requestId":"r","prompt":"/etc/hosts"}]}});
    assert_eq!(
        normalize_messages(&d).unwrap()["approvals"][0]["detail"],
        "/etc/hosts"
    );
    protocol::projection::apply(
        &mut d["projection"],
        &json!({"type":"runtime-request.updated","payload":{"id":"r","kind":"file-read","status":"resolved"}}),
    );
    assert_eq!(normalize_messages(&d).unwrap()["approvals"], json!([]));
}
