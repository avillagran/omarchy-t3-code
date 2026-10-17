use omarchy_t3_client::{diff_review, protocol, review_document, Environment};
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
fn historical_snapshot_shares_one_range_across_many_files_and_never_reads_checkout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let allow = Arc::new(AtomicBool::new(true));
    let changed = Arc::new(AtomicBool::new(false));
    let wrong = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (ss, aa, cc, ww, log) = (
        stop.clone(),
        allow.clone(),
        changed.clone(),
        wrong.clone(),
        seen.clone(),
    );
    let mut patch=(0..80).map(|i|format!("diff --git a/f{i}.js b/f{i}.js\n--- a/f{i}.js\n+++ b/f{i}.js\n@@ -2 +2 @@\n-const value = 1;\n+const value = 2;\n")).collect::<String>();
    patch.push_str("diff --git a/permissions.sh b/permissions.sh\nold mode 100644\nnew mode 100755\ndiff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n");
    let server = thread::spawn(move || {
        while !ss.load(Ordering::Relaxed) {
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
            let mut peek = [0; 512];
            let n = stream.peek(&mut peek).unwrap();
            let header = String::from_utf8_lossy(&peek[..n]);
            if header.starts_with("GET /api/auth/session ") {
                let mut bytes = vec![];
                let mut b = [0];
                while !bytes.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut b).unwrap();
                    bytes.push(b[0]);
                }
                let body=json!({"authenticated":true,"permissions":if aa.load(Ordering::Relaxed){vec!["orchestration:read"]}else{vec![]}}).to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                continue;
            }
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            log.lock().unwrap().push(request.clone());
            assert_eq!(
                request["tag"], "orchestration.getTurnDiff",
                "historical rendering must never call a checkout/filesystem API"
            );
            assert_eq!(
                request["payload"],
                json!({"threadId":"t","fromTurnCount":2,"toTurnCount":3,"ignoreWhitespace":false})
            );
            let diff = if cc.load(Ordering::Relaxed) {
                format!("{patch}\nchanged history")
            } else {
                patch.clone()
            };
            let result = json!({"threadId":"t","fromTurnCount":2,"toTurnCount":if ww.load(Ordering::Relaxed){4}else{3},"diff":diff});
            ws.send(tungstenite::Message::Text(
                json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}})
                    .to_string()
                    .into(),
            ))
            .unwrap();
        }
    });
    let c = protocol::Connection::new(
        &base,
        None,
        protocol::Credential::bearer("synthetic").unwrap(),
    )
    .unwrap();
    let e = Environment {
        id: "fixture".into(),
        name: "Fixture".into(),
        base_url: base,
        token_file: "/unused".into(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(c)),
    };
    let input = json!({"environment":"fixture","thread":"t","fromTurnCount":2,"toTurnCount":3,"ignoreWhitespace":false,"summary":{"fromTurnCount":2,"toTurnCount":3,"files":[]}});
    let view = diff_review::execute(&e, &input).unwrap();
    let files = view["files"].as_array().unwrap();
    assert_eq!(files.len(), 82);
    let id = &files[0]["reviewSnapshotId"];
    assert!(id.is_string());
    assert!(files.iter().take(80).all(|f| f["reviewSnapshotId"] == *id));
    assert!(files[80]["reviewSnapshotId"].is_null());
    assert!(files[80]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["text"] == "new mode 100755"));
    assert_eq!(files[81]["binary"], true);
    assert!(files[81]["reviewSnapshotId"].is_null());
    let mut open = json!({"environment":"fixture","thread":"t","reviewSnapshotId":id,"filePath":"f0.js","layout":"split","theme":{"appearance":"dark","variables":{}}});
    for path in ["f0.js", "f79.js"] {
        open["filePath"] = json!(path);
        let doc = review_document::open(&e, &open).unwrap();
        assert_eq!(doc["title"], path);
        assert_eq!(doc["html"], true);
        assert_eq!(doc["contextError"], "");
        assert!(doc["contents"]
            .as_str()
            .unwrap()
            .contains("\"canLoadContext\":false"));
    }
    let before = seen.lock().unwrap().len();
    open["filePath"] = json!("not-in-historical-patch");
    assert_eq!(
        review_document::open(&e, &open).unwrap_err(),
        "review_file_changed"
    );
    assert_eq!(seen.lock().unwrap().len(), before);
    open["filePath"] = json!("f0.js");
    open["thread"] = json!("other");
    assert_eq!(
        review_document::open(&e, &open).unwrap_err(),
        "git_scope_changed"
    );
    open["thread"] = json!("t");
    allow.store(false, Ordering::Relaxed);
    assert_eq!(
        review_document::open(&e, &open).unwrap_err(),
        "permission_denied"
    );
    assert_eq!(seen.lock().unwrap().len(), before);
    allow.store(true, Ordering::Relaxed);
    changed.store(true, Ordering::Relaxed);
    assert_eq!(
        review_document::open(&e, &open).unwrap_err(),
        "review_file_changed"
    );
    changed.store(false, Ordering::Relaxed);
    wrong.store(true, Ordering::Relaxed);
    assert_eq!(
        review_document::open(&e, &open).unwrap_err(),
        "invalid_diff_range"
    );
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
}
