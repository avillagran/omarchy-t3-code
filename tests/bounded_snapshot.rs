use omarchy_t3_client::{bounded_snapshot::restore, protocol, Client, Environment};
use serde_json::{json, Value};
#[test]
fn restores_exact_native_local_prefix_before_inherited_tail_only_when_marked() {
    let legacy = json!({"snapshotSequence":7,"historyCursor":"opaque","projection":{"thread":{"id":"t"},"turnItems":[{"id":"one"},{"id":"two"},{"id":"tail"}],"visibleTurnItems":[{"visibility":"inherited","sourceThreadId":"parent","item":{"id":"inherited"}},{"visibility":"local","sourceThreadId":"t","item":{"id":"one"}},{"visibility":"synthetic","sourceThreadId":"t","item":{"id":"two"}}]}});
    assert_eq!(restore(legacy.clone()).unwrap(), legacy);
    let mut compact = legacy.clone();
    compact["turnItemsOmitLocalVisible"] = json!(true);
    compact["projection"]["turnItems"] = json!([{"id":"tail"}]);
    let restored = restore(compact.clone()).unwrap();
    assert_eq!(restored, legacy);
    assert_eq!(restore(restored.clone()).unwrap(), restored);
    compact["turnItemsOmitLocalVisible"] = json!(false);
    assert!(restore(compact.clone()).is_err());
    compact["turnItemsOmitLocalVisible"] = json!(true);
    compact["projection"]["visibleTurnItems"] = Value::Null;
    assert!(restore(compact).is_err());
}
#[test]
fn production_http_keeps_generic_limit_and_accepts_large_compact_thread_snapshots() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        thread,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let item = json!({"id":"large","text":"x".repeat(9_000_000)});
    let compact = json!({"snapshotSequence":3,"turnItemsOmitLocalVisible":true,"projection":{"thread":{"id":"t"},"messages":[],"turnItems":[],"visibleTurnItems":[{"visibility":"local","sourceThreadId":"t","item":item}]}});
    let legacy = restore(compact.clone()).unwrap().to_string();
    assert!(legacy.len() > 8 * 1024 * 1024);
    let compact = compact.to_string();
    assert!(compact.len() > 8 * 1024 * 1024);
    let server = thread::spawn(move || {
        for i in 0..2 {
            let (mut s, _) = listener.accept().unwrap();
            let mut header = vec![];
            let mut b = [0];
            while !header.ends_with(b"\r\n\r\n") {
                s.read_exact(&mut b).unwrap();
                header.push(b[0]);
            }
            let h = String::from_utf8(header).unwrap();
            assert!(h.to_lowercase().contains("x-t3-orchestration-protocol: 2"));
            let body = if i == 0 {
                assert!(h.starts_with("GET /api/orchestration/shell HTTP"));
                &legacy
            } else {
                assert!(h.starts_with(
                    "GET /api/orchestration/threads/t/bounded?compactTurnItems=1&dedupeBodies=1&compactIds=1 HTTP"
                ));
                &compact
            };
            write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            let _ = s.write_all(body.as_bytes());
        }
    });
    let e = Environment {
        id: "fixture".into(),
        name: "Fixture".into(),
        base_url: base.clone(),
        token_file: "/unused".into(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(
            protocol::Connection::new(
                &base,
                None,
                protocol::Credential::bearer("synthetic").unwrap(),
            )
            .unwrap(),
        )),
    };
    let c = Client::new().unwrap();
    assert_eq!(
        c.get(&e, "/api/orchestration/shell", true)
            .unwrap_err(),
        "too_large"
    );
    let result = c.bounded_thread(&e, "t").unwrap();
    assert_eq!(
        result["projection"]["turnItems"][0]["text"]
            .as_str()
            .unwrap()
            .len(),
        9_000_000
    );
    assert_eq!(result["historyCursor"], Value::Null);
    server.join().unwrap();
}

#[test]
fn large_live_snapshot_is_allowed_only_for_bounded_thread_subscriptions() {
    use std::{net::TcpListener, sync::atomic::{AtomicBool, Ordering}, thread};
    for bounded in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(socket).unwrap();
            let _: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            let item = json!({"kind":"snapshot","projection":{"padding":"x".repeat(9_000_000)}});
            let _ = ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":[item]}).to_string().into()));
            let _ = ws.read();
        });
        let c = protocol::Connection::new(&base, None, protocol::Credential::bearer("synthetic").unwrap()).unwrap();
        let received = AtomicBool::new(false);
        let result = c.subscribe(if bounded {"orchestration.subscribeThread"} else {"device.subscribeState"}, json!({"acceptBoundedSnapshot":true}), || received.load(Ordering::Relaxed), |item| {
            if let Some(item) = item {
                assert_eq!(item["projection"]["padding"].as_str().unwrap().len(), 9_000_000);
                received.store(true, Ordering::Relaxed);
            }
            Ok(())
        });
        assert_eq!(received.load(Ordering::Relaxed), bounded);
        assert_eq!(result.is_ok(), bounded);
        server.join().unwrap();
    }
}

#[test]
fn bounded_snapshot_still_rejects_responses_over_64_mib() {
    use std::{io::{Read, Write}, net::TcpListener, thread};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut header = Vec::new();
        let mut b = [0];
        while !header.ends_with(b"\r\n\r\n") {s.read_exact(&mut b).unwrap(); header.push(b[0]);}
        write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",65*1024*1024).unwrap();
        let chunk = vec![b' '; 1024*1024];
        for _ in 0..65 {if s.write_all(&chunk).is_err() {break}}
    });
    let c = protocol::Connection::new(&base,None,protocol::Credential::bearer("synthetic").unwrap()).unwrap();
    assert_eq!(c.request_json("GET","/api/orchestration/threads/t/bounded?compactTurnItems=1",None,true).unwrap_err(),"too_large");
    server.join().unwrap();
}
