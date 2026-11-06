use omarchy_t3_client::{composer_context, drafts, protocol, write, Environment};
use serde_json::{json, Value};
use std::{net::TcpListener, sync::Arc, thread};

#[test]
fn terminal_context_survives_durable_drafts_and_exact_send_launch_retries() {
    let root = std::env::temp_dir().join(format!("o3-composer-context-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    std::env::set_var("XDG_STATE_HOME", &root);
    let text = "Inspect [Terminal 1 lines 2-3](t3-context://v1/terminal/terminal_fixture)";
    let context = json!({"version":1,"records":[{"version":1,"kind":"terminal","contextId":"terminal_fixture","label":"Terminal 1 lines 2-3","terminalId":"term-1","terminalLabel":"Terminal 1","lineStart":2,"lineEnd":3,"text":"α\nraw <context> text"}]});
    let draft = json!({"text":text,"context":context,"attachments":[]});
    drafts::save("fixture/thread", &draft).unwrap();
    assert_eq!(drafts::load("fixture/thread").unwrap()["context"], context);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let expected = context.clone();
    let server = thread::spawn(move || {
        let mut original = Value::Null;
        for index in 0..3 {
            let (socket, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(socket).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            let payload = &request["payload"];
            if index < 2 {
                assert_eq!(request["tag"], "orchestration.dispatchCommand");
                assert_eq!(payload["text"], text);
                assert_eq!(payload["context"], expected);
                if index == 0 {
                    original = payload.clone()
                } else {
                    assert_eq!(*payload, original)
                }
            } else {
                assert_eq!(request["tag"], "orchestration.launchThread");
                assert_eq!(payload["initialMessage"]["text"], text);
                assert_eq!(payload["initialMessage"]["context"], expected);
            }
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":"1","exit":{"_tag":"Success","value":{"sequence":1,"threadId":"created"}}}).to_string().into())).unwrap();
        }
    });
    let conn = protocol::Connection::new(
        &base,
        None,
        protocol::Credential::bearer("synthetic-context-token").unwrap(),
    )
    .unwrap();
    let env = Environment {
        id: "fixture".into(),
        name: "Fixture".into(),
        base_url: base,
        token_file: Default::default(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(conn)),
    };
    let mut command = draft;
    command["requestId"] = json!(uuid::Uuid::new_v4().to_string());
    command["op"] = json!("send");
    command["thread"] = json!("thread");
    command["delivery"] = json!("auto");
    let mut frozen =
        json!({"text":text,"context":context,"attachments":[],"attempt":command.clone()});
    assert_eq!(
        drafts::clean(&frozen).unwrap()["attempt"]["context"],
        context
    );
    frozen["context"]["records"][0]["text"] = json!("changed draft after admission");
    assert!(drafts::clean(&frozen).is_err());
    write::dispatch(&env, &command).unwrap();
    write::dispatch(&env, &command).unwrap();
    let mut changed = command.clone();
    changed["context"]["records"][0]["text"] = json!("changed");
    assert_eq!(
        write::dispatch(&env, &changed).unwrap_err(),
        "retry_payload_changed"
    );
    command["requestId"] = json!(uuid::Uuid::new_v4().to_string());
    command["sendRequestId"] = json!(uuid::Uuid::new_v4().to_string());
    command["op"] = json!("manage");
    command["action"] = json!("new-thread");
    command["projectId"] = json!("project");
    command["title"] = json!("Context");
    command["workspaceStrategy"] = json!({"type":"root"});
    command["modelSelection"] = json!({"instanceId":"fixture","model":"model"});
    command["runtimeMode"] = json!("full-access");
    command["interactionMode"] = json!("default");
    command["launchWithMessage"] = json!(true);
    write::manage(&env, &command).unwrap();
    server.join().unwrap();
    assert!(
        composer_context::for_message(&context, "deleted reference").unwrap()["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn native_contexts_survive_four_slot_queue_draft_ipc_without_borrowing_legacy_budget() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    fn context(prefix: &str, character: &str) -> (Value, String) {
        let text = character.repeat(64_000);
        let records = (0..200)
            .map(|index| {
                json!({
                    "version":1,"kind":"terminal","contextId":format!("{prefix}-{index}"),
                    "label":"Captured terminal","terminalId":"term-1","terminalLabel":"Terminal 1",
                    "lineStart":1,"lineEnd":1,"text":text
                })
            })
            .collect::<Vec<_>>();
        let references = (0..200)
            .map(|index| format!("[log](t3-context://v1/terminal/{prefix}-{index})"))
            .collect::<Vec<_>>()
            .join(" ");
        (json!({"version":1,"records":records}), references)
    }
    let root =
        std::env::temp_dir().join(format!("o3-native-context-budget-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let (current, text) = context("current", "当");
    let (original, _) = context("original", "原");
    let (backup, backup_text) = context("backup", "備");
    let mut draft = json!({"text":text,"attachments":[]});
    let mut attempt = json!({"op":"queue","action":"edit","environment":"fixture","thread":"thread","requestId":uuid::Uuid::new_v4().to_string(),"runId":"queued","text":draft["text"],"attachments":[]});
    attempt["context"] = current.clone();
    draft["context"] = current;
    draft["attempt"] = attempt;
    let mut queued = json!({"runId":"queued","messageId":"message","backup":{"text":backup_text,"attachments":[]}});
    queued["context"] = original;
    queued["backup"]["context"] = backup;
    draft["queueEdit"] = queued;
    assert!(composer_context::serialized_bytes(&draft).unwrap() > 140 * 1024 * 1024);
    let mut child = Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
        .arg("watch")
        .env("HOME", root.join("home"))
        .env("T3CODE_HOME", root.join("native"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env_remove("OMARCHY_T3_FIXTURE_ENVIRONMENTS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut save = json!({"op":"draft-save","key":"fixture/thread","revision":"native-budget"});
    save["draft"] = draft;
    writeln!(input, "{save}").unwrap();
    drop(save);
    writeln!(
        input,
        "{}",
        json!({"op":"draft-load","key":"fixture/thread"})
    )
    .unwrap();
    writeln!(input, "{}", json!({"op":"quit"})).unwrap();
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    assert!(events
        .iter()
        .find(|event| event["type"] == "draft-saved")
        .unwrap()["data"]["error"]
        .is_null());
    let loaded = &events
        .iter()
        .find(|event| event["type"] == "draft-loaded")
        .unwrap()["data"]["draft"];
    for (slot, prefix, character) in [
        (&loaded["context"], "current", "当"),
        (&loaded["attempt"]["context"], "current", "当"),
        (&loaded["queueEdit"]["context"], "original", "原"),
        (&loaded["queueEdit"]["backup"]["context"], "backup", "備"),
    ] {
        let records = slot["records"].as_array().unwrap();
        assert_eq!(records.len(), 200);
        let expected = character.repeat(64_000);
        for (index, record) in records.iter().enumerate() {
            assert_eq!(record["contextId"], format!("{prefix}-{index}"));
            assert_eq!(record["text"], expected);
        }
    }
    // An admitted context does not increase the allowance for ordinary file metadata.
    let small = json!({"version":1,"records":[]});
    assert_eq!(
        drafts::clean(
            &json!({"text":"ordinary","context":small,"attachments":[{"name":"x".repeat(750_001)}]})
        )
        .unwrap_err(),
        "draft_too_large"
    );
}
