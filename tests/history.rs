use omarchy_t3_client::*;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
#[test]
fn authenticated_native_history_page_is_normalized_without_live_controls() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let e = Environment {
        id: "fixture".into(),
        name: "Fixture".into(),
        base_url: format!("http://{}", listener.local_addr().unwrap()),
        token_file: Default::default(),
        write_token_file: None,
        bearer: Some("synthetic-private-token".into()),
        connection_error: None,
        connection: None,
    };
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut bytes = [0; 8192];
        let n = stream.read(&mut bytes).unwrap();
        let headers = String::from_utf8_lossy(&bytes[..n]).to_lowercase();
        assert!(headers.starts_with(
            "get /api/orchestration/threads/chat/history?cursor=opaque%2b%2f%3d%3f%26 "
        ));
        assert!(headers.contains("authorization: bearer synthetic-private-token"));
        assert!(headers.contains("x-t3-orchestration-protocol: 2"));
        let body=json!({"snapshotSequence":1,"hasMoreHistory":true,"nextCursor":"older","items":[{"sourceThreadId":"chat","sourceItemId":"item","item":{"id":"item","type":"assistant_message","messageId":"message","startedAt":"2026-10-16T00:00:00Z","text":"**Earlier** answer","attachments":[{"type":"image","id":"img","name":"preview.png","mimeType":"image/png","sizeBytes":123}]}}]}).to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
    });
    let result = write::history(&e, &json!({"thread":"chat","cursor":"opaque+/=?&"})).unwrap();
    assert_eq!(result["timeline"][0]["id"], "message");
    assert!(result["timeline"][0]["rendered"]
        .as_str()
        .unwrap()
        .contains("<strong>Earlier</strong>"));
    assert_eq!(result["timeline"][0]["attachments"][0]["id"], "img");
    assert_eq!(result["historyCursor"], "older");
    assert_eq!(result["hasMoreHistory"], true);
    assert_eq!(result["approvals"], json!([]));
    server.join().unwrap();
}
#[test]
fn search_uses_visible_markdown_labels_not_formatting_or_hidden_urls() {
    let d = json!({"projection":{"messages":[{"id":"a","role":"assistant","text":"A **need**le and [label](https://hidden.example) with `code`"},{"id":"u","role":"user","text":"**literal**"}]}});
    let r = normalize_messages(&d).unwrap();
    assert_eq!(
        r["messages"][0]["searchText"],
        "A needle and label with code\n"
    );
    assert_eq!(r["messages"][1]["searchText"], "**literal**");
}
