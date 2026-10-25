// The outbox keeps the exact command JSON it sends: commands now travel through the
// typed `Command`/`ThreadLaunch` of t3-protocol, and what reaches T3 (first send and
// explicit retry) must be byte for byte the JSON persisted before sending.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

fn outbox(root: &Path, id: &str) -> PathBuf {
    root.join("omarchy-t3-code/outbox").join(format!("{id}.json"))
}
fn bytes(v: &Value) -> String {
    serde_json::to_string(v).unwrap()
}

#[test]
fn outbox_commands_reach_t3_byte_for_byte_on_send_and_retry() {
    let root = std::env::temp_dir().join(format!("t3-outbox-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    std::env::set_var("XDG_STATE_HOME", &root);
    let token = root.join("token");
    std::fs::write(&token, "synthetic-private-token").unwrap();
    std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let e = Environment { id: "fixture".into(), name: "Fixture".into(), base_url: format!("http://{}", listener.local_addr().unwrap()), token_file: token, write_token_file: None, bearer: None, connection_error: None, connection: None };
    let (sent, received) = mpsc::channel::<(String, String)>();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut ws = tungstenite::accept(stream.unwrap()).unwrap();
            let request: Value = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            let tag = request["tag"].as_str().unwrap().to_string();
            let value = if tag == "orchestration.launchThread" { json!({"threadId":request["payload"]["threadId"],"projection":{},"resumed":false}) } else { json!({"sequence":1}) };
            sent.send((tag, bytes(&request["payload"]))).unwrap();
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":request["id"],"exit":{"_tag":"Success","value":value}}).to_string().into())).unwrap();
        }
    });
    let next = || received.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    let model = json!({"instanceId":"codex","model":"gpt-5"});
    let file = json!({"type":"file","id":"kept-file","name":"notes.txt","mimeType":"text/plain","sizeBytes":5});

    // write::dispatch: the persisted command is the last RPC; runtime and interaction
    // mode commands go first and are not persisted.
    let sends = [
        json!({"op":"send","thread":"chat","text":"Hello ![notes.txt](t3-context://v1/file/kept-file)","attachments":[file],"delivery":"steer","modelSelection":model,"runtimeMode":"full-access","interactionMode":"plan"}),
        json!({"op":"send","thread":"chat","text":"Queued","attachments":[],"delivery":"queue"}),
        json!({"op":"stop","thread":"chat","runId":"run-1"}),
        json!({"op":"queue","action":"edit","thread":"chat","runId":"queued","text":"Edited","attachments":[file],"context":{"refs":[]}}),
        json!({"op":"question","action":"respond","thread":"chat","runtimeRequestId":"ask%253A1","answers":{"when":"before"}}),
    ];
    for mut input in sends {
        let id = uuid::Uuid::new_v4().to_string();
        input["requestId"] = json!(id);
        let mut first = vec![];
        write::dispatch(&e, &input).unwrap();
        let modes = input.get("runtimeMode").map_or(0, |_| 1) + input.get("interactionMode").map_or(0, |_| 1);
        for _ in 0..=modes {
            first.push(next());
        }
        let stored = std::fs::read(outbox(&root, &id)).unwrap();
        let envelope: Value = serde_json::from_slice(&stored).unwrap();
        let (tag, payload) = first.last().unwrap();
        assert_eq!(tag, "orchestration.dispatchCommand");
        assert_eq!(*payload, bytes(&envelope["command"]), "{input}");
        write::dispatch(&e, &input).unwrap();
        let retry: Vec<_> = (0..=modes).map(|_| next()).collect();
        assert_eq!(retry, first, "a retry resends the same bytes");
        assert_eq!(std::fs::read(outbox(&root, &id)).unwrap(), stored, "the outbox is not rewritten");
    }

    // The exact wire JSON of a message, as main sent it before the typed API.
    let id = uuid::Uuid::new_v4().to_string();
    write::dispatch(&e, &json!({"op":"send","thread":"chat","requestId":id,"text":"Exact","attachments":[],"delivery":"send"})).unwrap();
    assert_eq!(next().1, format!(r#"{{"attachments":[],"commandId":"omarchy-{id}","createdBy":"user","creationSource":"web","deliveryIntent":"auto","dispatchMode":{{"type":"start_immediately"}},"messageId":"omarchy-{id}","text":"Exact","threadId":"chat","type":"message.dispatch"}}"#));

    // write::manage: the envelope keeps the RPC tag and the command it sends.
    let actions = [
        json!({"action":"new-thread","projectId":"project","title":" Legacy ","modelSelection":model,"branch":"main","worktreePath":null}),
        json!({"action":"new-thread","projectId":"project","title":"Launch","modelSelection":model,"runtimeMode":"approval-required","workspaceStrategy":{"type":"worktree","baseRef":"main","branch":"feature/x","startFromOrigin":false},"launchWithMessage":true,"sendRequestId":uuid::Uuid::new_v4().to_string(),"text":"Start","attachments":[file]}),
        json!({"action":"new-thread","projectId":"project","title":"Root","modelSelection":model,"workspaceStrategy":{"type":"root"}}),
        json!({"action":"fork","thread":"chat","runId":"run-1","title":"Chat"}),
        json!({"action":"merge-back","thread":"fork","runId":"run-2","targetThread":"chat"}),
        json!({"action":"stop-subagent","thread":"chat","runId":"run-1","subagentId":"agent-1"}),
        json!({"action":"thread-rename","thread":"chat","title":"Renamed"}),
        json!({"action":"pin","thread":"chat"}),
        json!({"action":"archive","thread":"chat"}),
        json!({"action":"snooze","thread":"chat","snoozedUntil":"2099-01-01T09:00:00.000Z"}),
        json!({"action":"settle","thread":"chat"}),
    ];
    for mut input in actions {
        let id = uuid::Uuid::new_v4().to_string();
        input["requestId"] = json!(id);
        write::manage(&e, &input).unwrap();
        let first = next();
        let stored = std::fs::read(outbox(&root, &id)).unwrap();
        let envelope: Value = serde_json::from_slice(&stored).unwrap();
        assert_eq!(first.0, envelope["rpc"].as_str().unwrap(), "{input}");
        assert_eq!(first.1, bytes(&envelope["command"]), "{input}");
        write::manage(&e, &input).unwrap();
        assert_eq!(next(), first, "a retry resends the same bytes");
        assert_eq!(std::fs::read(outbox(&root, &id)).unwrap(), stored);
    }
    std::fs::remove_dir_all(root).unwrap();
}

/// Commands ot3 builds outside the outbox paths above: parsing them into `Command`
/// and back must not change a byte.
#[test]
fn typed_commands_keep_the_json_ot3_builds() {
    let commands = [
        json!({"type":"thread.visit","commandId":"omarchy-visit-1","threadId":"chat","visitedAt":"2026-10-24T12:00:00.000Z"}),
        json!({"type":"thread.pin.reorder","commandId":"omarchy-pin-1","threadId":"chat","orderKey":"m"}),
        json!({"type":"thread.metadata.update","commandId":"omarchy-branch-1","threadId":"chat","branch":"main","worktreePath":null}),
        json!({"type":"thread.metadata.update","commandId":"omarchy-branch-2","threadId":"chat","branch":"feature/x"}),
        json!({"type":"thread.unarchive","threadId":"chat","commandId":"omarchy-archive-1"}),
        json!({"type":"thread.delete","threadId":"chat","commandId":"omarchy-archive-2"}),
        json!({"type":"queued-run.cancel","commandId":"omarchy-compact-cancel-1","threadId":"chat","runId":"run-1"}),
        json!({"type":"thread.create","commandId":"omarchy-1","threadId":"omarchy-thread-1","projectId":"project","title":"#7 Fix","modelSelection":{"instanceId":"codex","model":"gpt-5"},"runtimeMode":"approval-required","interactionMode":"default","branch":"pr-7","worktreePath":"/work/pr-7","createdBy":"user","creationSource":"web"}),
    ];
    for command in commands {
        let typed = protocol::api::Command::from_json(&command).unwrap();
        assert_eq!(bytes(&typed.payload().unwrap()), bytes(&command));
    }
    for link in ["link", "unlink", "watch", "unwatch"] {
        let input = json!({"thread":"chat","link":link,"reference":{"host":"github.com","repository":"o/r","number":7,"url":"https://github.com/o/r/pull/7"}});
        let command = protocol::pull_requests::thread_command(&input).unwrap();
        assert_eq!(bytes(&protocol::api::Command::from_json(&command).unwrap().payload().unwrap()), bytes(&command), "{link}");
    }
}
