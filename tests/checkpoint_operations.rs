use omarchy_t3_client::{
    checkpoint_journal as journal, checkpoint_operations as ops, checkpoint_revert as rewind,
    drafts, protocol, write, Environment,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
fn projection() -> Value {
    json!({"thread":{"id":"thread:one","activeProviderThreadId":"provider:one","providerInstanceId":"codex","modelSelection":{"instanceId":"codex","model":"m"},"worktreePath":"/synthetic/tree","rollbackRequestId":null},"checkpointScopes":[{"id":"scope","threadId":"thread:one"}],"providerThreads":[{"id":"provider:one","providerInstanceId":"codex","providerSessionId":"session"}],"checkpoints":[{"id":"initial","threadId":"thread:one","scopeId":"scope","runId":null,"ordinalWithinScope":0,"appRunOrdinal":null,"status":"ready"},{"id":"end","threadId":"thread:one","scopeId":"scope","runId":"run","ordinalWithinScope":1,"appRunOrdinal":1,"status":"ready"}],"runs":[{"id":"run","ordinal":1,"status":"completed"}],"turnItems":[{"item":{"type":"user_message","messageId":"message","runId":"run","inputIntent":"turn_start"}}],"messages":[{"id":"message","role":"user","runId":"run","text":"Ultrathink:\nOriginal user prompt\n\n<terminal_context>\n- Terminal 1 line 4:\n  output\n</terminal_context>","attachments":[],"context":[]}]})
}
fn request(p: &Value) -> Value {
    json!({"environment":"fixture","thread":"thread:one","requestId":uuid::Uuid::new_v4().to_string(),"reference":rewind::reference(p,"message").unwrap(),"confirmed":true,"restoreFiles":false})
}
#[test]
fn native_rollback_ack_is_not_completion_recovery_is_atomic_and_uncertain_retry_is_identical() {
    let root = std::env::temp_dir().join(format!("ot3-checkpoint-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    std::env::set_var("XDG_STATE_HOME", root.join("state"));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let uncertain = Arc::new(AtomicBool::new(false));
    let allow = Arc::new(AtomicBool::new(true));
    let p = Arc::new(Mutex::new(projection()));
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let uploaded = Arc::new(Mutex::new(Vec::<u8>::new()));
    let (ss, uu, aa, pp, cc, uploaded_copy) = (
        stop.clone(),
        uncertain.clone(),
        allow.clone(),
        p.clone(),
        calls.clone(),
        uploaded.clone(),
    );
    let server = thread::spawn(move || {
        while !ss.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut peek = [0; 2048];
            let n = stream.peek(&mut peek).unwrap();
            let first = String::from_utf8_lossy(&peek[..n]);
            if (first.starts_with("GET /api/")
                || first.starts_with("POST /api/attachments/upload/"))
                && !first.to_ascii_lowercase().contains("upgrade: websocket")
            {
                let mut headers = vec![];
                let mut byte = [0];
                while !headers.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                }
                let headers = String::from_utf8(headers).unwrap();
                let (mime, body) = if headers.starts_with("GET /api/auth/session ") {
                    ("application/json",json!({"authenticated":true,"permissions":if aa.load(Ordering::Relaxed){vec!["orchestration:operate"]}else{vec!["orchestration:read"]}}).to_string().into_bytes())
                } else if headers.starts_with("GET /api/assets/") {
                    assert!(!headers.to_ascii_lowercase().contains("authorization:"));
                    ("text/plain", b"file bytes".to_vec())
                } else {
                    assert!(headers.starts_with("POST /api/attachments/upload/"));
                    let len = headers
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|s| s.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    let mut bytes = vec![0; len];
                    stream.read_exact(&mut bytes).unwrap();
                    *uploaded_copy.lock().unwrap() = bytes;
                    ("application/json", b"{}".to_vec())
                };
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
                stream.write_all(&body).unwrap();
                continue;
            }
            let mut ws = tungstenite::accept(stream).unwrap();
            let r: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            cc.lock().unwrap().push(r.clone());
            let result = match r["tag"].as_str().unwrap() {
                "orchestration.getThreadProjection" => pp.lock().unwrap().clone(),
                "server.getConfig" => {
                    json!({"providers":[{"instanceId":"codex","enabled":true,"supportsConversationRollback":true}]})
                }
                "assets.createUrl" => {
                    assert_eq!(r["payload"]["resource"]["attachmentId"], "source-file");
                    json!({"relativeUrl":"/api/assets/signed-fixture"})
                }
                "attachments.createUploadUrl" => {
                    assert_eq!(r["payload"]["sizeBytes"], 10);
                    json!({"relativeUrl":"/api/attachments/upload/synthetic","attachmentId":"new-file"})
                }
                "orchestration.dispatchCommand" => {
                    let command = &r["payload"];
                    assert_eq!(command["type"], "checkpoint.rollback");
                    let record = journal::read("fixture", "thread:one").unwrap().unwrap();
                    assert_eq!(record["envelope"]["command"], *command);
                    assert!(record["state"] == "prepared" || record["state"] == "accepted");
                    if uu.swap(false, Ordering::Relaxed) {
                        let mut projection = pp.lock().unwrap();
                        projection["thread"]["rollbackRequestId"] = command["commandId"].clone();
                        projection["runs"][0]["status"] = json!("rolled_back");
                        ws.close(None).unwrap();
                        continue;
                    }
                    json!({})
                }
                other => panic!("unexpected native method {other}"),
            };
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
    let key = "fixture/thread:one";
    drafts::save(key, &json!({"text":"Initial draft","attachments":[]})).unwrap();
    let normalized =
        omarchy_t3_client::normalize_messages(&json!({"projection":p.lock().unwrap().clone()}))
            .unwrap();
    assert_eq!(normalized["messages"][0]["canRewindMessage"], true);
    assert_eq!(
        normalized["messages"][0]["revertReference"]["checkpointId"],
        "initial"
    );
    assert_eq!(
        normalized["messages"][0]["recallableText"],
        "Original user prompt"
    );
    let preview = ops::preview(
        &e,
        &json!({"environment":"fixture","thread":"thread:one","messageId":"message"}),
    )
    .unwrap();
    assert_eq!(preview["reference"]["checkpointId"], "initial");
    let first = request(&p.lock().unwrap());
    let receipt = ops::dispatch(&e, &first).unwrap();
    assert_eq!(receipt["state"], "accepted");
    assert_eq!(drafts::load(key).unwrap()["text"], "Initial draft");
    let status =
        || json!({"environment":"fixture","thread":"thread:one","operationId":first["requestId"]});
    assert_eq!(ops::observe(&e, &status()).unwrap()["state"], "accepted");
    let mut replay = first.clone();
    replay["retry"] = json!(true);
    ops::dispatch(&e, &replay).unwrap();
    let sent = calls.lock().unwrap();
    let writes: Vec<_> = sent
        .iter()
        .filter(|r| r["tag"] == "orchestration.dispatchCommand")
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0]["payload"], writes[1]["payload"]);
    drop(sent);
    assert_eq!(write::dispatch(&e,&json!({"op":"send","environment":"fixture","thread":"thread:one","requestId":uuid::Uuid::new_v4().to_string(),"text":"Must wait","attachments":[]})).unwrap_err(),"revert_already_pending");
    {
        let mut projection = p.lock().unwrap();
        projection["thread"]["rollbackRequestId"] =
            json!(format!("omarchy-{}", first["requestId"].as_str().unwrap()));
        projection["runs"][0]["status"] = json!("rolled_back");
    }
    assert_eq!(ops::observe(&e, &status()).unwrap()["state"], "complete");
    let recovery = json!({"environment":"fixture","thread":"thread:one","operationId":first["requestId"],"draft":{"text":"Typed while waiting","attachments":[]}});
    let recovered = ops::recover(&recovery).unwrap();
    assert_eq!(
        recovered["draft"]["text"],
        "Typed while waiting\n\nOriginal user prompt"
    );
    assert_eq!(
        recovered["draft"]["recoveryIds"],
        json!([first["requestId"]])
    );
    assert_eq!(
        ops::recover(&recovery).unwrap()["draft"]["text"],
        recovered["draft"]["text"]
    );
    assert_eq!(
        drafts::save(key, &json!({"text":"Stale autosave","attachments":[]})).unwrap_err(),
        "draft_recovery_changed"
    );
    let mut latest = recovered["draft"].clone();
    latest["text"] = json!("New edit after recovery");
    drafts::save(key, &latest).unwrap();
    let result = ops::recover(
        &json!({"environment":"fixture","thread":"thread:one","operationId":first["requestId"]}),
    )
    .unwrap();
    assert_eq!(result["draft"]["text"], "New edit after recovery");
    let mut next = projection();
    next["thread"]["rollbackRequestId"] = p.lock().unwrap()["thread"]["rollbackRequestId"].clone();
    *p.lock().unwrap() = next;
    let second = request(&p.lock().unwrap());
    ops::dispatch(&e, &second).unwrap();
    ops::recover(
        &json!({"environment":"fixture","thread":"thread:one","operationId":first["requestId"]}),
    )
    .unwrap();
    assert_eq!(
        journal::read("fixture", "thread:one").unwrap().unwrap()["request"]["requestId"],
        second["requestId"]
    );
    assert!(journal::ensure_available("fixture", "thread:one").is_err());
    {
        let mut projection = p.lock().unwrap();
        projection["thread"]["rollbackRequestId"] =
            json!(format!("omarchy-{}", second["requestId"].as_str().unwrap()));
        projection["thread"]["rollbackFailure"] = json!({"requestId":projection["thread"]["rollbackRequestId"],"message":"Synthetic failure"});
    }
    let status =
        json!({"environment":"fixture","thread":"thread:one","operationId":second["requestId"]});
    assert_eq!(ops::observe(&e, &status).unwrap()["state"], "failed");
    let mut retry = second.clone();
    retry["retry"] = json!(true);
    assert_eq!(ops::dispatch(&e, &retry).unwrap()["state"], "failed");
    // Native accepted and completed, but the transport response was lost. Fresh
    // correlated read-back completes the original action without a second rollback.
    *p.lock().unwrap() = projection();
    let third = request(&p.lock().unwrap());
    uncertain.store(true, Ordering::Relaxed);
    assert_eq!(ops::dispatch(&e, &third).unwrap()["state"], "uncertain");
    let before = calls
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["tag"] == "orchestration.dispatchCommand")
        .count();
    let mut retry = third.clone();
    retry["retry"] = json!(true);
    assert_eq!(ops::dispatch(&e, &retry).unwrap()["state"], "complete");
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "orchestration.dispatchCommand")
            .count(),
        before
    );
    ops::recover(
        &json!({"environment":"fixture","thread":"thread:one","operationId":third["requestId"]}),
    )
    .unwrap();
    // Real authenticated signed-asset download -> fresh native attachment upload
    // happens before rollback, and recovery retains the latest existing files.
    let mut with_file = projection();
    with_file["messages"][0]["attachments"] = json!([{"id":"source-file","type":"file","name":"notes.txt","mimeType":"text/plain","sizeBytes":10}]);
    *p.lock().unwrap() = with_file;
    let mut with_existing = drafts::load(key).unwrap();
    with_existing["attachments"] = json!([{"uploadId":"existing","name":"existing.txt","uploading":false,"attachment":{"id":"existing-file","type":"file","name":"existing.txt","mimeType":"text/plain","sizeBytes":5}}]);
    drafts::save(key, &with_existing).unwrap();
    let fourth = request(&p.lock().unwrap());
    ops::dispatch(&e, &fourth).unwrap();
    assert_eq!(*uploaded.lock().unwrap(), b"file bytes");
    let r = journal::action(fourth["requestId"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(r["attachments"][0]["attachment"]["id"], "new-file");
    {
        let mut projection = p.lock().unwrap();
        projection["thread"]["rollbackRequestId"] =
            json!(format!("omarchy-{}", fourth["requestId"].as_str().unwrap()));
        projection["runs"][0]["status"] = json!("rolled_back");
    }
    ops::observe(
        &e,
        &json!({"environment":"fixture","thread":"thread:one","operationId":fourth["requestId"]}),
    )
    .unwrap();
    let mut latest = drafts::load(key).unwrap();
    latest["attachments"].as_array_mut().unwrap().push(json!({"uploadId":"later","name":"later.txt","uploading":false,"attachment":{"id":"later-file","type":"file","name":"later.txt","mimeType":"text/plain","sizeBytes":2}}));
    let result = ops::recover(
        &json!({"environment":"fixture","thread":"thread:one","operationId":fourth["requestId"],"draft":latest}),
    )
    .unwrap();
    let ids: Vec<_> = result["draft"]["attachments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["attachment"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["existing-file", "later-file", "new-file"]);
    let preserved = result["draft"].clone();
    let mut full = preserved.clone();
    full["attachments"] = json!(vec![with_existing["attachments"][0].clone(); 100]);
    drafts::save(key, &full).unwrap();
    let mut available = projection();
    available["messages"][0]["attachments"] = json!([{"id":"source-file","type":"file","name":"notes.txt","mimeType":"text/plain","sizeBytes":10}]);
    *p.lock().unwrap() = available;
    let no_room = request(&p.lock().unwrap());
    let sent_before = calls
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["tag"] == "orchestration.dispatchCommand")
        .count();
    assert_eq!(
        ops::dispatch(&e, &no_room).unwrap_err(),
        "invalid_attachments"
    );
    assert!(journal::action(no_room["requestId"].as_str().unwrap())
        .unwrap()
        .is_none());
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "orchestration.dispatchCommand")
            .count(),
        sent_before
    );
    drafts::save(key, &preserved).unwrap();
    allow.store(false, Ordering::Relaxed);
    *p.lock().unwrap() = projection();
    let denied = request(&p.lock().unwrap());
    assert_eq!(
        ops::dispatch(&e, &denied).unwrap_err(),
        "revert_operate_required"
    );
    assert_eq!(journal::list().unwrap().len(), 0);
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_rolled_back_runs_remove_messages_tools_plans_and_checkpoint_rows_after_readback() {
    let mut p = projection();
    p["runs"][0]["status"] = json!("rolled_back");
    p["checkpoints"][1]["files"] =
        json!([{"path":"changed.rs","kind":"modified","additions":1,"deletions":0}]);
    p["plans"] = json!([{"id":"plan","threadId":"thread:one","runId":"run","nodeId":"node","kind":"proposed_plan","status":"active","markdown":"# Old plan"}]);
    p["turnItems"] = json!([{"item":{"id":"tool","threadId":"thread:one","runId":"run","nodeId":"node","type":"command_execution","status":"completed","startedAt":1,"input":"ls","output":"old"}},{"item":{"id":"plan-item","type":"proposed_plan","planId":"plan","runId":"run","startedAt":2}}]);
    let view = omarchy_t3_client::normalize_messages(&json!({"projection":p})).unwrap();
    assert_eq!(view["messages"], json!([]));
    assert_eq!(view["timeline"], json!([]));
    assert_eq!(view["planView"]["plans"], json!([]));
    assert_eq!(view["threadState"]["working"], false);
}
