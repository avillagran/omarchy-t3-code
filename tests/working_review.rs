use omarchy_t3_client::working_review::present;
use serde_json::{json, Value};
fn source(kind: &str) -> Value {
    json!({"id":kind,"kind":kind,"title":"Changes","baseRef":"main","headRef":"feature","diffHash":"opaque-native-hash","truncated":true,"diff":"not retained","files":[{"path":" spaced 名.rs ","previousPath":null,"additions":3,"deletions":2}]})
}
#[test]
fn full_statistics_survive_truncated_patch_and_sources_stay_distinct() {
    let mut branch = source("branch-range");
    branch["files"][0]["previousPath"] = json!("previous 名.rs");
    branch["files"][0]["additions"] = json!(9);
    let raw = json!({"sources":[source("working-tree"),branch]});
    let tree = present(&raw, "working-tree").unwrap();
    let branch = present(&raw, "branch-range").unwrap();
    assert_eq!(tree["files"][0]["path"], " spaced 名.rs ");
    assert_eq!(tree["additions"], 3);
    assert_eq!(branch["additions"], 9);
    assert_eq!(branch["files"][0]["oldPath"], "previous 名.rs");
    assert_eq!(branch["files"][0]["kind"], "renamed");
    assert_eq!(tree["files"][0]["pending"], true);
    assert!(tree["source"].get("diff").is_none());
    assert_eq!(tree["source"]["truncated"], true);
}
#[test]
fn legacy_stats_fallback_limits_and_invalid_native_results() {
    let mut s = source("working-tree");
    s.as_object_mut().unwrap().remove("files");
    s["diff"] = json!("diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-old\n+new\n");
    let out = present(&json!({"sources":[s]}), "working-tree").unwrap();
    assert_eq!(out["files"][0]["path"], "a");
    assert_eq!(out["files"][0]["pending"], true);
    assert_eq!(out["files"][0]["rows"], json!([]));
    let mut s = source("working-tree");
    s["files"][0]["additions"] = json!(-1);
    assert!(present(&json!({"sources":[s]}), "working-tree").is_err());
    let s = source("working-tree");
    assert!(present(&json!({"sources":[s.clone(),s.clone()]}), "working-tree").is_err());
    let mut many = s.clone();
    many["files"] = json!((0..2049)
        .map(|i| json!({"path":format!("f{i}"),"previousPath":null,"additions":0,"deletions":0}))
        .collect::<Vec<_>>());
    assert_eq!(
        present(&json!({"sources":[many]}), "working-tree").unwrap_err(),
        "diff_limit_files"
    );
    assert!(present(&json!({"sources":[s]}), "all").is_err());
    assert_eq!(
        present(&json!({"sources":[]}), "working-tree").unwrap()["files"],
        json!([])
    );
}

#[test]
fn native_transport_uses_authoritative_remote_checkout_and_filesystem_permission() {
    use omarchy_t3_client::{protocol, working_review, Environment};
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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let allow = Arc::new(AtomicBool::new(true));
    let moved = Arc::new(AtomicBool::new(false));
    let during = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (ss, aa, mm, dd, log) = (
        stop.clone(),
        allow.clone(),
        moved.clone(),
        during.clone(),
        seen.clone(),
    );
    let stale = Arc::new(AtomicBool::new(false));
    let changed = stale.clone();
    let server = thread::spawn(move || {
        while !ss.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(3));
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
            if header.starts_with("GET /api/")
                && !header.to_lowercase().contains("upgrade: websocket")
            {
                let auth = header.starts_with("GET /api/auth/session ");
                let mut bytes = vec![];
                let mut byte = [0];
                while !bytes.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                let body=if auth{json!({"authenticated":true,"permissions":if aa.load(Ordering::Relaxed){vec!["filesystem:read","orchestration:read"]}else{vec![]}})}else{json!({"threads":[{"id":"t","projectId":"p","worktreePath":if mm.load(Ordering::Relaxed){"/remote/moved"}else{"/remote/tree"}}],"projects":[{"id":"p","workspaceRoot":"/remote/root"}]})}.to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                continue;
            }
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            log.lock().unwrap().push(request.clone());
            if request["tag"] == "review.getDiffFileContents" {
                let p = &request["payload"];
                assert_eq!(p["cwd"], "/remote/tree");
                assert_eq!(p["sourceKind"], "working-tree");
                assert_eq!(p["changeType"], "change");
                assert_eq!(p["oldPath"], "a.rs");
                assert_eq!(p["newPath"], "a.rs");
                ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"oldContents":"old\n","newContents":"new\n"}}}).to_string().into())).unwrap();
                continue;
            }
            if request["tag"] == "vcs.listRefs" {
                let p = &request["payload"];
                assert_eq!(p["cwd"], "/remote/tree");
                assert_eq!(p["refKind"], "remote");
                assert_eq!(p["query"], "main");
                assert_eq!(p["cursor"], 100);
                assert_eq!(p["limit"], 100);
                assert_eq!(p["includeMatchingRemoteRefs"], true);
                let result = json!({"refs":[{"name":"origin/main","isRemote":true,"remoteName":"origin","current":false,"isDefault":true,"worktreePath":null}],"nextCursor":null,"totalCount":101,"isRepo":true,"hasPrimaryRemote":true});
                ws.send(tungstenite::Message::Text(
                    json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}})
                        .to_string()
                        .into(),
                ))
                .unwrap();
                continue;
            }
            assert_eq!(request["tag"], "review.getDiffPreview");
            let p = &request["payload"];
            assert_eq!(p["cwd"], "/remote/tree");
            let mut tree = source("working-tree");
            let mut branch = source("branch-range");
            for s in [&mut tree, &mut branch] {
                s["diff"] = json!(
                    "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n"
                );
                s["files"] =
                    json!([{"path":"a.rs","previousPath":null,"additions":1,"deletions":1}]);
                s["truncated"] = json!(false);
                if changed.load(Ordering::Relaxed) {
                    s["diffHash"] = json!("changed-after-full-file-read");
                }
            }
            if dd.load(Ordering::Relaxed) {
                mm.store(true, Ordering::Relaxed);
            }
            let result = json!({"cwd":p["cwd"],"sources":[tree,branch]});
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
    let mut input = json!({"environment":"fixture","thread":"t","projectId":"p","cwd":"/remote/tree","requestId":uuid::Uuid::new_v4().to_string(),"sourceKind":"working-tree","filePath":"a.rs","ignoreWhitespace":false,"baseRef":"origin/main"});
    let out = working_review::execute(&e, &input).unwrap();
    assert_eq!(out["file"]["path"], "a.rs");
    assert!(out["file"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["text"] == "+new"));
    {
        let log = seen.lock().unwrap();
        assert_eq!(log.len(), 2);
        assert!(log[0]["payload"].get("file").is_none());
        assert_eq!(
            log[1]["payload"]["file"],
            json!({"path":"a.rs","previousPath":null,"sourceKind":"working-tree"})
        );
        assert_eq!(log[1]["payload"]["ignoreWhitespace"], false);
        assert_eq!(log[1]["payload"]["baseRef"], "origin/main");
    }
    let doc_input = json!({"environment":"fixture","thread":"t","reviewSnapshotId":out["file"]["reviewSnapshotId"],"theme":{"appearance":"dark","variables":{}},"layout":"stacked","wrap":true});
    let document = omarchy_t3_client::review_document::open(&e, &doc_input).unwrap();
    assert_eq!(document["html"], true);
    assert_eq!(document["internalReview"], true);
    assert_eq!(document["contextError"], "");
    stale.store(true, Ordering::Relaxed);
    assert_eq!(
        omarchy_t3_client::review_document::open(&e, &doc_input).unwrap_err(),
        "review_file_changed"
    );
    stale.store(false, Ordering::Relaxed);
    let mut other = doc_input.clone();
    other["thread"] = json!("other");
    assert_eq!(
        omarchy_t3_client::review_document::open(&e, &other).unwrap_err(),
        "git_scope_changed"
    );
    allow.store(false, Ordering::Relaxed);
    let before = seen.lock().unwrap().len();
    assert_eq!(
        omarchy_t3_client::review_document::open(&e, &doc_input).unwrap_err(),
        "permission_denied"
    );
    assert_eq!(seen.lock().unwrap().len(), before);
    allow.store(true, Ordering::Relaxed);
    input["filePath"] = json!("/etc/passwd");
    let before = seen.lock().unwrap().len();
    assert_eq!(
        working_review::execute(&e, &input).unwrap_err(),
        "review_file_changed"
    );
    assert_eq!(seen.lock().unwrap().len(), before + 1);
    allow.store(false, Ordering::Relaxed);
    let before = seen.lock().unwrap().len();
    assert_eq!(
        working_review::execute(&e, &input).unwrap_err(),
        "permission_denied"
    );
    assert_eq!(seen.lock().unwrap().len(), before);
    allow.store(true, Ordering::Relaxed);
    input["cwd"] = json!("/local/wrong");
    assert_eq!(
        working_review::execute(&e, &input).unwrap_err(),
        "git_scope_changed"
    );
    assert_eq!(seen.lock().unwrap().len(), before);
    input["cwd"] = json!("/remote/tree");
    input["filePath"] = Value::Null;
    let mut ri = input.clone();
    ri["query"] = json!(" main ");
    ri["cursor"] = json!(100);
    ri["refKind"] = json!("remote");
    let page = working_review::refs(&e, &ri).unwrap();
    assert_eq!(page["refs"][0]["name"], "origin/main");
    assert_eq!(page["nextCursor"], Value::Null);
    assert_eq!(page["scope"]["cwd"], "/remote/tree");
    allow.store(false, Ordering::Relaxed);
    let before = seen.lock().unwrap().len();
    assert_eq!(
        working_review::refs(&e, &ri).unwrap_err(),
        "permission_denied"
    );
    assert_eq!(seen.lock().unwrap().len(), before);
    allow.store(true, Ordering::Relaxed);
    during.store(true, Ordering::Relaxed);
    assert_eq!(
        working_review::execute(&e, &input).unwrap_err(),
        "git_scope_changed"
    );
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
}

#[test]
fn ref_contract_trims_query_preserves_pages_and_rejects_invalid_cursor_and_results() {
    use omarchy_t3_client::working_review::{refs_payload, refs_present};
    let input =
        json!({"cwd":"/remote/tree","refKind":"local","query":"  feature/名  ","cursor":100});
    assert_eq!(
        refs_payload(&input).unwrap(),
        json!({"cwd":"/remote/tree","refKind":"local","query":"feature/名","cursor":100,"limit":100,"includeMatchingRemoteRefs":true})
    );
    for (key, value) in [
        ("refKind", json!("all")),
        ("cursor", json!(-1)),
        ("cursor", json!("100")),
        ("query", json!("a".repeat(257))),
        ("query", json!("😀".repeat(129))),
    ] {
        let mut i = input.clone();
        i[key] = value;
        assert!(refs_payload(&i).is_err());
    }
    let mut raw = json!({"refs":[{"name":"main","current":false,"isDefault":true,"worktreePath":null}],"nextCursor":101,"totalCount":110,"isRepo":true,"hasPrimaryRemote":true});
    assert_eq!(refs_present(&raw, &input).unwrap()["query"], "feature/名");
    raw["nextCursor"] = json!(100);
    assert!(refs_present(&raw, &input).is_err());
    raw["nextCursor"] = json!(111);
    assert!(refs_present(&raw, &input).is_err());
    raw["nextCursor"] = Value::Null;
    raw["refs"] = json!([{"name":"main","current":false,"isDefault":true},{"name":"main","current":false,"isDefault":true}]);
    assert!(refs_present(&raw, &input).is_err());
    let mut empty = input.clone();
    empty["query"] = json!("");
    empty["cursor"] = Value::Null;
    let p = refs_payload(&empty).unwrap();
    assert!(p.get("query").is_none() && p.get("cursor").is_none());
}
