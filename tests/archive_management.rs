use omarchy_t3_client::{archive_management as archive, protocol, Environment};
use serde_json::{json, Value};
fn row(id: &str, project: &str, date: &str) -> Value {
    json!({"id":id,"projectId":project,"title":format!("<b>{id}</b>"),"archivedAt":date,"createdAt":"2026-01-01T00:00:00.000Z","deletedAt":null})
}
fn snapshot() -> Value {
    json!({"schemaVersion":1,"snapshotSequence":20,"projects":[{"id":"p","title":"Project","workspaceRoot":"/remote/p","projectIcon":{"kind":"emoji","emoji":"🌱"},"privateField":"omit"},{"id":"q","title":"Other"}],"threads":[row("a","p","2026-02-01T00:00:00.000Z"),row("z","p","2026-03-01T00:00:00.000Z"),row("b","p","2026-02-01T00:00:00.000Z"),row("qthread","q","2026-03-01T00:00:00.000Z")]})
}
#[test]
fn projects_are_scoped_ordered_and_projected_without_rich_title_interpretation() {
    let data = archive::present(&snapshot(), "remote", Some(vec!["p".into()])).unwrap();
    assert_eq!(data["groups"].as_array().unwrap().len(), 1);
    let g = &data["groups"][0];
    assert_eq!(g["environment"], "remote");
    assert_eq!(g["project"]["projectIcon"]["emoji"], "🌱");
    assert!(g["project"].get("privateField").is_none());
    assert_eq!(
        g["threads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["z", "b", "a"]
    );
    assert_eq!(g["threads"][0]["title"], "<b>z</b>");
    assert!(
        archive::present(&snapshot(), "remote", Some(vec![])).unwrap()["groups"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        archive::present(&snapshot(), "remote", None).unwrap()["groups"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn malformed_archive_does_not_look_like_an_empty_success() {
    let mut s = snapshot();
    s["threads"][1]["id"] = json!("a");
    assert!(archive::present(&s, "e", None).is_err());
    let mut s = snapshot();
    s["threads"][0]["archivedAt"] = Value::Null;
    assert!(archive::present(&s, "e", None).is_err());
    let mut s = snapshot();
    s["schemaVersion"] = json!(0);
    assert!(archive::present(&s, "e", None).is_err());
    let mut s = snapshot();
    s["projects"][1]["id"] = json!("p");
    assert!(archive::present(&s, "e", None).is_err());
}
#[test]
fn production_transport_journals_scopes_permissions_exact_retry_and_observed_completion() {
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
    let state = std::env::temp_dir().join(format!("ot3-archive-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&state).unwrap();
    std::env::set_var("XDG_STATE_HOME", &state);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stopped = Arc::new(AtomicBool::new(false));
    let operate = Arc::new(AtomicBool::new(true));
    let archived = Arc::new(Mutex::new(snapshot()));
    let active = Arc::new(Mutex::new(Vec::<Value>::new()));
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let lost = Arc::new(AtomicBool::new(false));
    let (ss, oo, aa, mm, cc, ll, dir) = (
        stopped.clone(),
        operate.clone(),
        archived.clone(),
        active.clone(),
        calls.clone(),
        lost.clone(),
        state.clone(),
    );
    let server = thread::spawn(move || {
        while !ss.load(Ordering::Relaxed) {
            let (mut stream, _) = match listener.accept() {
                Ok(s) => s,
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
                let mut b = vec![];
                let mut byte = [0];
                while !b.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    b.push(byte[0])
                }
                let body=json!({"authenticated":true,"permissions":if oo.load(Ordering::Relaxed){vec!["orchestration:read","orchestration:operate"]}else{vec!["orchestration:read"]}}).to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                continue;
            }
            if header.starts_with("GET /api/orchestration/shell") {
                let mut b = vec![];
                let mut byte = [0];
                while !b.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    b.push(byte[0])
                }
                let body = json!({"schemaVersion":1,"snapshotSequence":20,"threads":mm.lock().unwrap().clone(),"archivedThreads":aa.lock().unwrap()["threads"],"projects":[]}).to_string();
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                continue;
            }
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            let p = &request["payload"];
            let value = match request["tag"].as_str().unwrap() {
                "orchestration.getArchivedShellSnapshot" => {
                    assert_eq!(p, &json!({}));
                    aa.lock().unwrap().clone()
                }
                "orchestration.dispatchCommand" => {
                    assert!(oo.load(Ordering::Relaxed));
                    assert_eq!(p.as_object().unwrap().len(), 3);
                    let id = p["commandId"]
                        .as_str()
                        .unwrap()
                        .strip_prefix("omarchy-archive-")
                        .unwrap();
                    let journal = dir
                        .join("omarchy-t3-code/outbox/archive-actions")
                        .join(format!("{id}.json"));
                    let record: Value =
                        serde_json::from_slice(&std::fs::read(journal).unwrap()).unwrap();
                    assert_eq!(record["state"], "pending");
                    assert_eq!(record["command"], *p);
                    cc.lock().unwrap().push(p.clone());
                    if ll.load(Ordering::Relaxed) {
                        drop(ws);
                        continue;
                    }
                    json!({"sequence":19})
                }
                x => panic!("unexpected {x}"),
            };
            ws.send(tungstenite::Message::Text(
                json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":value}})
                    .to_string()
                    .into(),
            ))
            .unwrap();
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
    let query = || json!({"environment":"fixture","projectIds":["p"],"requestId":uuid::Uuid::new_v4().to_string()});
    let view = archive::read(&e, &query()).unwrap();
    assert_eq!(view["writable"], true);
    let mut action = json!({"environment":"fixture","thread":"a","viewId":view["viewId"],"operation":"unarchive","actionId":uuid::Uuid::new_v4().to_string()});
    let result = archive::action(&e, &action).unwrap();
    assert_eq!(result["state"], "accepted");
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(archive::action(&e, &action).unwrap()["state"], "accepted");
    assert_eq!(
        calls.lock().unwrap().len(),
        1,
        "same id never implicitly replays"
    );
    let mut competing = action.clone();
    competing["actionId"] = json!(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        archive::action(&e, &competing).unwrap_err(),
        "archive_operation_pending"
    );
    // A lost ACK remains discoverable across screen/process restarts. Explicit retry sends the saved command.
    action["retry"] = json!(true);
    lost.store(true, Ordering::Relaxed);
    assert_eq!(archive::action(&e, &action).unwrap()["state"], "uncertain");
    let reread = archive::read(&e, &query()).unwrap();
    assert_eq!(reread["operations"][0]["actionId"], action["actionId"]);
    lost.store(false, Ordering::Relaxed);
    assert_eq!(archive::action(&e, &action).unwrap()["state"], "accepted");
    {
        let c = calls.lock().unwrap();
        assert_eq!(c.len(), 3);
        assert_eq!(c[0], c[1]);
        assert_eq!(c[0], c[2]);
    }
    {
        let mut s = archived.lock().unwrap();
        s["threads"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["id"] != "a");
    }
    let mut a = row("a", "p", "x");
    a["archivedAt"] = Value::Null;
    active.lock().unwrap().push(a);
    assert_eq!(archive::observe(&e, &action).unwrap()["state"], "complete");
    assert_eq!(archive::action(&e, &action).unwrap()["state"], "complete");
    assert_eq!(calls.lock().unwrap().len(), 3);
    // Same title/project/archive fingerprint is required immediately before native dispatch.
    let mut stale = competing.clone();
    stale["thread"] = json!("z");
    archived.lock().unwrap()["threads"][0]["title"] = json!("renamed");
    assert_eq!(
        archive::action(&e, &stale).unwrap_err(),
        "archive_thread_changed"
    );
    let view = archive::read(&e, &query()).unwrap();
    stale["viewId"] = view["viewId"].clone();
    operate.store(false, Ordering::Relaxed);
    assert_eq!(archive::read(&e, &query()).unwrap()["writable"], false);
    assert_eq!(
        archive::action(&e, &stale).unwrap_err(),
        "permission_denied"
    );
    operate.store(true, Ordering::Relaxed);
    let mut deletion = stale.clone();
    deletion["operation"] = json!("delete");
    assert_eq!(
        archive::action(&e, &deletion).unwrap_err(),
        "confirmation_required"
    );
    deletion["confirmed"] = json!(true);
    assert_eq!(archive::action(&e, &deletion).unwrap()["state"], "accepted");
    {
        let mut s = archived.lock().unwrap();
        s["threads"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["id"] != "z");
    }
    assert_eq!(
        archive::observe(&e, &deletion).unwrap()["state"],
        "complete"
    );
    // Old completion cannot replace the newest per-thread guard, even after re-archiving.
    archived.lock().unwrap()["threads"]
        .as_array_mut()
        .unwrap()
        .push(row("a", "p", "2026-04-01T00:00:00.000Z"));
    active.lock().unwrap().clear();
    let view = archive::read(&e, &query()).unwrap();
    let mut newer = action.clone();
    newer["actionId"] = json!(uuid::Uuid::new_v4().to_string());
    newer["viewId"] = view["viewId"].clone();
    newer["retry"] = json!(false);
    assert_eq!(archive::action(&e, &newer).unwrap()["state"], "accepted");
    assert_eq!(archive::action(&e, &action).unwrap()["state"], "complete");
    competing["viewId"] = view["viewId"].clone();
    competing["actionId"] = json!(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        archive::action(&e, &competing).unwrap_err(),
        "archive_operation_pending"
    );
    let mut wrong = e.clone();
    wrong.base_url = "http://127.0.0.1:1".into();
    assert_eq!(
        archive::observe(&wrong, &newer).unwrap_err(),
        "archive_scope_changed"
    );
    assert_eq!(calls.lock().unwrap().len(), 5);
    stopped.store(true, Ordering::Relaxed);
    server.join().unwrap();
    std::fs::remove_dir_all(state).unwrap();
}
