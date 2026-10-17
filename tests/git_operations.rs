use omarchy_t3_client::{git_operations as git, protocol, Environment};
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
fn status() -> Value {
    json!({"isRepo":true,"hasPrimaryRemote":true,"isDefaultRef":false,"hasWorkingTreeChanges":true,"hasUpstream":true,"refName":"feature","aheadCount":1,"behindCount":0,"pr":null,"workingTree":{"files":[{"path":"a.rs","insertions":2,"deletions":1}],"insertions":2,"deletions":1}})
}
fn intent(view: &Value, operation: &str) -> Value {
    let id = uuid::Uuid::new_v4().to_string();
    let mut intent = json!({"scope":view["scope"],"revision":view["revision"],"operation":operation,"confirmed":true,"defaultBranchConfirmed":false,"actionId":id});
    if operation == "script" {
        intent["scriptId"] = json!("test")
    };
    json!({"op":"git-action","environment":"fixture","thread":"thread","requestId":id,"intent":intent})
}
#[test]
fn shared_native_transport_journals_before_stream_and_never_replays_unknown_or_old_actions() {
    let root = std::env::temp_dir().join(format!("ot3-git-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("bin")).unwrap();
    std::env::set_var("XDG_STATE_HOME", root.join("state"));
    use std::os::unix::fs::PermissionsExt;
    let launcher = root.join("bin/omarchy-launch-terminal");
    fs::write(&launcher, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
    let old_path = std::env::var_os("PATH");
    std::env::set_var("PATH", root.join("bin"));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let fail = Arc::new(AtomicBool::new(false));
    let fail_write = Arc::new(AtomicBool::new(false));
    let fw = fail_write.clone();
    let allow = Arc::new(AtomicBool::new(true));
    let native_status = Arc::new(Mutex::new(status()));
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (ss, ff, aa, ns, log, server_root) = (
        stop.clone(),
        fail.clone(),
        allow.clone(),
        native_status.clone(),
        seen.clone(),
        root.clone(),
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
            let mut peek = [0; 512];
            let n = stream.peek(&mut peek).unwrap();
            let request = String::from_utf8_lossy(&peek[..n]);
            if request.starts_with("GET /api/")
                && !request.contains("Upgrade: websocket")
                && !request.contains("upgrade: websocket")
            {
                let mut headers = vec![];
                let mut byte = [0];
                while !headers.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                }
                let body=if request.starts_with("GET /api/auth/session "){json!({"authenticated":true,"permissions":if aa.load(Ordering::Relaxed){vec!["source-control:write","terminal:operate","terminal:read"]}else{vec!["orchestration:read"]}})}else{assert!(request.starts_with("GET /api/orchestration/shell "));json!({"threads":[{"id":"thread","projectId":"p","worktreePath":"/remote/tree"}],"projects":[{"id":"p","workspaceRoot":"/remote/root"}]})}.to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                continue;
            }
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            log.lock().unwrap().push(request.clone());
            if request["tag"] == "git.runStackedAction" {
                let p = &request["payload"];
                let journal = server_root
                    .join("state/omarchy-t3-code/outbox/git-actions")
                    .join(format!("{}.json", p["actionId"].as_str().unwrap()));
                let persisted: Value = serde_json::from_slice(&fs::read(journal).unwrap()).unwrap();
                assert_eq!(persisted["state"], "pending");
                assert_eq!(persisted["plan"]["steps"][0]["payload"], *p);
                if ff.swap(false, Ordering::Relaxed) {
                    ws.close(None).unwrap();
                    continue;
                }
                let result = json!({"action":p["action"],"branch":{"status":"skipped_not_requested"},"commit":{"status":"created","commitSha":"abc"},"push":{"status":"skipped_not_requested"},"pr":{"status":"skipped_not_requested"},"toast":{"title":"Committed"}});
                ws.send(tungstenite::Message::Text(json!({"_tag":"Chunk","requestId":"1","values":[{"kind":"phase_started","actionId":p["actionId"],"cwd":p["cwd"],"action":p["action"],"phase":"commit"},{"kind":"action_finished","actionId":p["actionId"],"cwd":p["cwd"],"action":p["action"],"result":result}]}).to_string().into())).unwrap();
                let _ = ws.read();
                continue;
            }
            if request["tag"] == "terminal.write" && fw.swap(false, Ordering::Relaxed) {
                ws.close(None).unwrap();
                continue;
            }
            let result = match request["tag"].as_str().unwrap() {
                "server.getSettings" => {
                    json!({"defaultProjectScripts":[{"id":"test","name":"Test","command":"echo sample","icon":"test","runOnWorktreeCreate":false}],"projectSettingsFolded":true})
                }
                "vcs.refreshStatus" => ns.lock().unwrap().clone(),
                "terminal.open" | "terminal.write" => json!({}),
                other => panic!("unexpected native RPC {other}"),
            };
            ws.send(tungstenite::Message::Text(
                json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":result}})
                    .to_string()
                    .into(),
            ))
            .unwrap();
        }
    });
    let connection = protocol::Connection::new(
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
        connection: Some(Arc::new(connection)),
    };
    let scope = || json!({"environment":"fixture","thread":"thread","projectId":"p","cwd":"/remote/tree","requestId":uuid::Uuid::new_v4().to_string()});
    let view = git::read(&e, &scope()).unwrap();
    assert_eq!(view["permissions"]["sourceControlWrite"], true);
    assert_eq!(view["permissions"]["terminalWrite"], true);
    let one = intent(&view, "commit");
    let receipt = git::action(&e, &one).unwrap();
    assert_eq!(receipt["state"], "succeeded");
    assert_eq!(receipt["result"]["commit"]["commitSha"], "abc");
    let count = || {
        seen.lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "git.runStackedAction")
            .count()
    };
    assert_eq!(count(), 1);
    assert_eq!(git::action(&e, &one).unwrap()["state"], "succeeded");
    assert_eq!(count(), 1);
    let view = git::read(&e, &scope()).unwrap();
    let two = intent(&view, "commit");
    git::action(&e, &two).unwrap();
    assert_eq!(count(), 2);
    assert_eq!(git::action(&e, &one).unwrap()["actionId"], one["requestId"]);
    assert_eq!(count(), 2);
    let view = git::read(&e, &scope()).unwrap();
    native_status.lock().unwrap()["refName"] = json!("different");
    assert_eq!(
        git::action(&e, &intent(&view, "commit")).unwrap_err(),
        "git_stale_result"
    );
    assert_eq!(count(), 2);
    allow.store(false, Ordering::Relaxed);
    assert_eq!(
        git::action(&e, &intent(&view, "commit")).unwrap_err(),
        "permission_denied"
    );
    assert_eq!(count(), 2);
    allow.store(true, Ordering::Relaxed);
    let view = git::read(&e, &scope()).unwrap();
    let uncertain = intent(&view, "commit");
    fail.store(true, Ordering::Relaxed);
    let receipt = git::action(&e, &uncertain).unwrap();
    assert_eq!(receipt["state"], "unknown");
    assert_eq!(count(), 3);
    assert_eq!(git::action(&e, &uncertain).unwrap()["state"], "unknown");
    assert_eq!(count(), 3);
    assert_eq!(
        git::action(&e, &intent(&view, "commit")).unwrap_err(),
        "git_operation_unknown"
    );
    assert_eq!(
        git::status(&e, &json!({"scope":view["scope"]})).unwrap()["state"],
        "unknown"
    );
    let mut resolve =
        json!({"scope":view["scope"],"actionId":uncertain["requestId"],"confirmed":false});
    assert!(git::resolve(&e, &resolve).is_err());
    resolve["confirmed"] = json!(true);
    assert_eq!(git::resolve(&e, &resolve).unwrap()["state"], "reviewed");
    let view = git::read(&e, &scope()).unwrap();
    let script = intent(&view, "script");
    let result = git::action(&e, &script).unwrap();
    assert_eq!(result["state"], "submitted");
    assert_eq!(
        result["terminalId"],
        format!("term-ot3-script-{}", script["requestId"].as_str().unwrap())
    );
    let captured = seen.lock().unwrap();
    let steps: Vec<_> = captured
        .iter()
        .filter(|r| r["tag"] == "terminal.open" || r["tag"] == "terminal.write")
        .collect();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["tag"], "terminal.open");
    assert_eq!(
        steps[0]["payload"]["env"],
        json!({"T3CODE_PROJECT_ROOT":"/remote/root","T3CODE_WORKTREE_PATH":"/remote/tree"})
    );
    assert_eq!(steps[1]["payload"]["terminalId"], result["terminalId"]);
    assert_eq!(steps[1]["payload"]["data"], "echo sample\r");
    drop(captured);
    git::action(&e, &script).unwrap();
    assert_eq!(
        seen.lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "terminal.write")
            .count(),
        1
    );
    // A restarted host has no live checkout lock. Persisted pending is uncertain,
    // and reopening/retrying the journal never delivers terminal input twice.
    let latest = fs::read_dir(root.join("state/omarchy-t3-code/outbox/git"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    let mut interrupted: Value = serde_json::from_slice(&fs::read(&latest).unwrap()).unwrap();
    interrupted["state"] = json!("pending");
    omarchy_t3_client::save_json(&latest, &interrupted).unwrap();
    assert_eq!(
        git::status(&e, &json!({"scope":view["scope"]})).unwrap()["state"],
        "unknown"
    );
    assert_eq!(git::action(&e, &script).unwrap()["state"], "unknown");
    assert_eq!(
        seen.lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "terminal.write")
            .count(),
        1
    );
    let resolve = json!({"scope":view["scope"],"actionId":script["requestId"],"confirmed":true});
    git::resolve(&e, &resolve).unwrap();
    let view = git::read(&e, &scope()).unwrap();
    let ambiguous_script = intent(&view, "script");
    fail_write.store(true, Ordering::Relaxed);
    let receipt = git::action(&e, &ambiguous_script).unwrap();
    assert_eq!(receipt["state"], "unknown");
    assert!(receipt["terminalId"].is_string());
    assert_eq!(
        git::action(&e, &ambiguous_script).unwrap()["state"],
        "unknown"
    );
    assert_eq!(
        seen.lock()
            .unwrap()
            .iter()
            .filter(|r| r["tag"] == "terminal.write")
            .count(),
        2
    );
    stop.store(true, Ordering::Relaxed);
    server.join().unwrap();
    if let Some(path) = old_path {
        std::env::set_var("PATH", path)
    } else {
        std::env::remove_var("PATH")
    };
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_pr_metadata_reuses_scoped_pr_reference_without_guessing_urls() {
    let scope = json!({"environment":"node","projectId":"project"});
    for (url, repository) in [
        ("https://github.com/owner/repo/pull/42", "owner/repo"),
        (
            "https://git.example/group/team/repo/-/merge_requests/42",
            "group/team/repo",
        ),
        ("https://code.example/owner/repo/pulls/42", "owner/repo"),
    ] {
        let r = git::pull_request_reference(&scope, &json!({"number":42,"url":url}));
        assert_eq!(r["repository"], repository);
        assert_eq!(r["projectId"], "project");
        assert_eq!(r["environmentId"], "node");
    }
    for url in [
        "javascript:alert(1)",
        "https://example.test/owner/repo/pull/7",
        "https://example.test/unknown/42",
    ] {
        assert!(git::pull_request_reference(&scope, &json!({"number":42,"url":url})).is_null())
    }
}
