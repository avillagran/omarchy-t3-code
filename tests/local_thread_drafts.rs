use omarchy_t3_client::{local_drafts, write, Environment};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
fn isolated() {
    assert!(
        Path::new("/run/.containerenv").is_file(),
        "Private rootless runtime required"
    );
}
fn root(label: &str) -> PathBuf {
    isolated();
    let root = std::env::temp_dir().join(format!("local-draft-{label}-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    root
}
fn entry(reservation: &str) -> Value {
    json!({"draftId":"00000000-0000-4000-8000-000000000099","threadReservationId":reservation,"threadId":format!("omarchy-thread-{reservation}"),"environmentId":"fixture","projectId":"project","createdAt":"2026-11-07T12:00:00Z","title":"Local thread","runtimeMode":"approval-required","interactionMode":"default","modelSelection":{"instanceId":"codex","model":"fixture","options":[]},"workspaceStrategy":{"type":"root"}})
}
#[test]
fn metadata_is_separate_from_authored_content_and_preserves_model_intent() {
    isolated();
    let reservation = "00000000-0000-4000-8000-000000000001";
    let key = format!("fixture/omarchy-thread-{reservation}");
    let mut row = entry(reservation);
    row["text"] = json!("PRIVATE_PROMPT");
    row["credentials"] = json!("PRIVATE_CREDENTIAL");
    let clean =
        local_drafts::metadata(&json!({(key.clone()):row,"foreign/key":entry(reservation)}));
    assert_eq!(clean.as_object().unwrap().len(), 1);
    assert!(!clean.to_string().contains("PRIVATE"));
    assert_eq!(clean[&key]["threadReservationId"], reservation);
    let sticky = local_drafts::sticky(
        &json!({"codex":{"instanceId":"codex","model":"fixture","options":[{"id":"effort","value":"high"}]},"foreign":{"instanceId":"codex","model":"wrong"}}),
    );
    assert_eq!(sticky.as_object().unwrap().len(), 1);
    assert_eq!(sticky["codex"]["options"][0]["value"], "high");
    let draft=omarchy_t3_client::drafts::clean(&json!({"text":"Authored content","modelSelection":{"instanceId":"codex","model":"fixture","options":[]},"modelSelectionExplicit":true})).unwrap();
    assert_eq!(draft["modelSelectionExplicit"], true);
    assert_eq!(draft["text"], "Authored content");
    assert!(omarchy_t3_client::drafts::clean(
        &json!({"modelSelection":null,"modelSelectionExplicit":true})
    )
    .unwrap()
    .get("modelSelectionExplicit")
    .is_none());
}
fn worker(root: &Path, commands: &[Value]) -> Vec<Value> {
    let fixture = root.join("environments.json");
    fs::write(&fixture, "{\"environments\":[]}").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
        .arg("watch")
        .env("HOME", root.join("home"))
        .env("T3CODE_HOME", root.join("native"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("OMARCHY_T3_FIXTURE_ENVIRONMENTS", fixture)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for command in commands {
        writeln!(input, "{command}").unwrap();
    }
    writeln!(input, "{}", json!({"op":"quit"})).unwrap();
    drop(input);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("Owned private worker exceeded deadline")
        };
        thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}
#[test]
fn actual_worker_restart_retains_local_identity_explicit_model_and_uncertain_attempt() {
    let root = root("restart");
    let reservation = "00000000-0000-4000-8000-000000000001";
    let key = format!("fixture/omarchy-thread-{reservation}");
    let attempt = json!({"op":"manage","action":"new-thread","environment":"fixture","thread":format!("omarchy-thread-{reservation}"),"targetThreadId":format!("omarchy-thread-{reservation}"),"localDraftId":"00000000-0000-4000-8000-000000000099","requestId":"00000000-0000-4000-8000-000000000002","sendRequestId":"00000000-0000-4000-8000-000000000003","projectId":"project","text":"Retained authored content","attachments":[],"modelSelection":{"instanceId":"codex","model":"fixture","options":[]},"workspaceStrategy":{"type":"root"},"launchWithMessage":true,"generateTitle":true});
    let state = json!({"localDraftThreads":{(key.clone()):entry(reservation)},"stickyActiveProvider":"codex","stickyModelSelectionByProvider":{"codex":{"instanceId":"codex","model":"fixture","options":[]}},"stickyOptionsByModelByProvider":{"codex":{"fixture":[{"id":"effort","value":"high"}]}},"modelFallbackDismissedWindows":{"fixture":"codex:reset"}});
    let saved = worker(
        &root,
        &[
            json!({"op":"ui-state-set","state":state}),
            json!({"op":"draft-save","key":key,"revision":"revision","draft":{"text":"Retained authored content","localDraftId":"00000000-0000-4000-8000-000000000099","modelSelection":{"instanceId":"codex","model":"fixture","options":[]},"modelSelectionExplicit":true,"attempt":attempt}}),
        ],
    );
    assert!(
        saved
            .iter()
            .any(|v| v["type"] == "draft-saved" && v["data"]["error"].is_null()),
        "{saved:?}"
    );
    let restored = worker(
        &root,
        &[json!({"op":"draft-load","key":key,"projectId":"project"})],
    );
    let metadata = &restored.iter().find(|v| v["type"] == "ui-state").unwrap()["data"];
    let draft = &restored
        .iter()
        .find(|v| v["type"] == "draft-loaded")
        .unwrap()["data"]["draft"];
    assert_eq!(
        metadata["localDraftThreads"][&key]["threadId"],
        attempt["targetThreadId"]
    );
    assert_eq!(
        metadata["stickyOptionsByModelByProvider"]["codex"]["fixture"][0]["value"],
        "high"
    );
    assert_eq!(
        metadata["modelFallbackDismissedWindows"]["fixture"],
        "codex:reset"
    );
    assert!(!metadata.to_string().contains("Retained authored content"));
    assert_eq!(draft["text"], "Retained authored content");
    assert_eq!(draft["modelSelectionExplicit"], true);
    assert_eq!(draft["attempt"]["requestId"], attempt["requestId"]);
    assert_eq!(
        draft["attempt"]["targetThreadId"],
        attempt["targetThreadId"]
    );
    assert_eq!(draft["attempt"]["generateTitle"], true);
}
#[test]
fn actual_typed_first_launch_matches_native_wire_and_new_command_keeps_future_thread() {
    let root = root("wire");
    std::env::set_var("XDG_STATE_HOME", root.join("state"));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut calls = Vec::new();
        for index in 0..2 {
            let socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut ws = tungstenite::accept(socket).unwrap();
            loop {
                let message = ws.read().unwrap();
                if let tungstenite::Message::Text(text) = message {
                    let request: Value = serde_json::from_str(&text).unwrap();
                    if request["_tag"] == "Ping" {
                        ws.send(tungstenite::Message::Text(
                            json!({"_tag":"Pong"}).to_string().into(),
                        ))
                        .unwrap();
                        continue;
                    }
                    assert_eq!(request["tag"], "orchestration.launchThread");
                    calls.push(request["payload"].clone());
                    let exit = if index == 0 {
                        json!({"_tag":"Failure","cause":{"_tag":"Fail","error":{"_tag":"OrchestrationDispatchCommandError","message":"Synthetic admission refused"}}})
                    } else {
                        json!({"_tag":"Success","value":{"threadId":"thread","projectId":"project"}})
                    };
                    ws.send(tungstenite::Message::Text(
                        json!({"_tag":"Exit","requestId":request["id"],"exit":exit})
                            .to_string()
                            .into(),
                    ))
                    .unwrap();
                    break;
                }
            }
        }
        calls
    });
    let token = root.join("token");
    fs::write(&token, "private-fixture").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let environment = Environment {
        id: "fixture".into(),
        name: "Private".into(),
        base_url: base,
        token_file: token.clone(),
        write_token_file: Some(token),
        bearer: None,
        connection_error: None,
        connection: None,
    };
    let first = json!({"action":"new-thread","requestId":"00000000-0000-4000-8000-000000000001","sendRequestId":"00000000-0000-4000-8000-000000000002","thread":"thread","targetThreadId":"thread","projectId":"project","title":"Authored title","text":"Private message","attachments":[],"modelSelection":{"instanceId":"codex","model":"fixture","options":[]},"runtimeMode":"approval-required","interactionMode":"default","workspaceStrategy":{"type":"root"},"launchWithMessage":true,"generateTitle":true});
    assert!(write::manage(&environment, &first)
        .unwrap_err()
        .starts_with("rpc_rejected"));
    let mut changed = first.clone();
    changed["text"] = json!("Revised message");
    assert_eq!(
        write::manage(&environment, &changed).unwrap_err(),
        "retry_payload_changed"
    );
    changed["requestId"] = json!("00000000-0000-4000-8000-000000000003");
    changed["sendRequestId"] = json!("00000000-0000-4000-8000-000000000004");
    assert_eq!(
        write::manage(&environment, &changed).unwrap()["threadId"],
        "thread"
    );
    let calls = server.join().unwrap();
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/local-thread-draft-4301.json")).unwrap();
    let native = oracle["starts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["input"]["bootstrap"]["createThread"].is_object()
                && row["input"]["bootstrap"]["createThread"]["branch"].is_null()
                && row["input"]["bootstrap"]["createThread"]["worktreePath"].is_null()
                && row["input"]["bootstrap"]["prepareWorktree"].is_null()
                && row["input"]["titleSeed"].is_string()
                && row["input"]["modelSelection"].is_null()
                && row["input"]["message"]["attachments"]
                    .as_array()
                    .unwrap()
                    .is_empty()
        })
        .unwrap();
    let mut expected = native["calls"][0]["payload"].clone();
    expected["initialMessage"]["messageId"] = json!("omarchy-00000000-0000-4000-8000-000000000002");
    assert_eq!(calls[0], expected);
    assert_eq!(calls[0]["threadId"], calls[1]["threadId"]);
    assert_ne!(calls[0]["commandId"], calls[1]["commandId"]);
    assert_eq!(calls[1]["initialMessage"]["text"], "Revised message");
}

#[test]
fn published_settings_resolver_matches_native_workspace_observations() {
    isolated();
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/local-thread-draft-4301.json")).unwrap();
    let cases = fixture["workspaceCases"].as_array().unwrap();
    assert_eq!(cases.len(), 72);
    for (index, row) in cases.iter().enumerate() {
        let actual = local_drafts::workspace_defaults(
            &row["raw"],
            Some(&row["project"]),
            row.get("file").filter(|file| !file.is_null()),
        );
        for field in [
            "mode",
            "modeSource",
            "defaultModelSelection",
            "defaultRuntimeMode",
            "startFromOrigin",
        ] {
            assert_eq!(
                actual[field], row["expected"][field],
                "workspace case {index} field {field}"
            );
        }
    }
}
#[test]
fn local_owner_claims_never_replace_an_unrelated_saved_destination() {
    let root = root("ownership");
    std::env::set_var("XDG_STATE_HOME", root.join("state"));
    let key = "fixture/unclaimed";
    omarchy_t3_client::drafts::save(key, &json!({"text":"Unrelated authored draft"})).unwrap();
    let owner = "00000000-0000-4000-8000-000000000001";
    assert_eq!(
        omarchy_t3_client::drafts::save(key, &json!({"text":"Replacement","localDraftId":owner}))
            .unwrap_err(),
        "draft_destination_occupied"
    );
    assert_eq!(
        omarchy_t3_client::drafts::load(key).unwrap()["text"],
        "Unrelated authored draft"
    );
    let key = "fixture/owned";
    omarchy_t3_client::drafts::save(
        key,
        &json!({"text":"Owned authored draft","localDraftId":owner}),
    )
    .unwrap();
    assert_eq!(
        omarchy_t3_client::drafts::save(
            key,
            &json!({"text":"Replacement","localDraftId":"00000000-0000-4000-8000-000000000002"})
        )
        .unwrap_err(),
        "draft_owner_changed"
    );
    assert_eq!(
        omarchy_t3_client::drafts::load(key).unwrap()["text"],
        "Owned authored draft"
    );
    omarchy_t3_client::drafts::save(
        key,
        &json!({"text":"Edited through an existing ordinary draft consumer"}),
    )
    .unwrap();
    assert_eq!(
        omarchy_t3_client::drafts::load(key).unwrap()["localDraftId"],
        owner
    );
    let mut positions = serde_json::Map::new();
    for index in 0..128 {
        positions.insert(
            format!("{index:03}{}", "k".repeat(2100)),
            json!({"id":"i".repeat(2200),"offset":1,"atEnd":false}),
        );
    }
    let oversized = json!({"positions":positions,"boardIdeas":[{"key":"owned-note","text":"n".repeat(599000)}]});
    let events = worker(
        &root,
        &[
            json!({"op":"ui-state-set","state":{"query":"Previously retained state"}}),
            json!({"op":"ui-state-set","state":oversized}),
        ],
    );
    assert!(
        events.iter().any(|event| event["type"] == "ui-state-error"
            && event["data"]["error"] == "ui_state_too_large"),
        "{events:?}"
    );
    let restored = worker(&root, &[]);
    assert_eq!(
        restored
            .iter()
            .find(|event| event["type"] == "ui-state")
            .unwrap()["data"]["query"],
        "Previously retained state"
    );
}
