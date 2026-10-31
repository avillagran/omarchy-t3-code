use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
};
const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("o3-shared-drafts-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        // A test authority: records the exact request and returns a controlled response.
        let script = root.join("bridge");
        fs::write(
            &script,
            r#"#!/bin/sh
cat > "$XDG_STATE_HOME/request.json"
python3 - <<'PYTHON'
import json, os
from pathlib import Path
root = Path(os.environ['XDG_STATE_HOME'])
raw = (root / 'response.json').read_text()
try:
    response = json.loads(raw)
except ValueError:
    print(raw)
else:
    if isinstance(response, list):
        request = json.loads((root / 'request.json').read_text())
        response = response[1 if request['command'].get('offset') else 0]
    if isinstance(response, dict) and '__byOp' in response:
        request = json.loads((root / 'request.json').read_text())
        response = response['__byOp'][request['command']['op']]
        if response.get('mutationId') == '$mutation':
            response['mutationId'] = request['command']['mutationId']
    print(json.dumps(response))
PYTHON
"#,
        )
        .unwrap();
        fs::set_permissions(script, fs::Permissions::from_mode(0o700)).unwrap();
        Self(root)
    }
    fn run(&self, input: Value) -> std::process::Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
            .arg("shared-drafts")
            .env("XDG_STATE_HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("OT3_OMABRIDGE_BIN", self.0.join("bridge"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
    fn ok(&self, input: Value) -> Value {
        let output = self.run(input);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn response(&self, response: Value) {
        fs::write(self.0.join("response.json"), response.to_string()).unwrap();
    }
    fn input(&self, action: &str, id: &str) -> Value {
        json!({"action":action,"authority":OWNER,"journalId":id})
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn scope() -> Value {
    json!({"environment":"11111111-1111-4111-8111-111111111111","project":"22222222-2222-4222-8222-222222222222","kind":"thread","id":"thread-one"})
}
fn command() -> Value {
    json!({"op":"put","scope":scope(),"expected":null,"baseCursor":format!("{OWNER}:12"),"mutationId":format!("12.{}",uuid::Uuid::new_v4()),"payload":{"schema":1,"text":"Private offline branch","attachments":[]}})
}
#[test]
fn durable_stage_is_offline_retry_keeps_exact_identity_and_applied_result() {
    let f = Fixture::new();
    let id = uuid::Uuid::new_v4().to_string();
    let cmd = command();
    let mut stage = f.input("stage", &id);
    stage["command"] = cmd.clone();
    assert_eq!(f.ok(stage.clone())["command"], cmd);
    assert!(!f.0.join("request.json").exists());
    f.response(json!({"status":"applied","mutationId":cmd["mutationId"]}));
    let receipt = f.ok(f.input("retry", &id));
    assert_eq!(receipt["outcome"]["status"], "applied");
    let request: Value =
        serde_json::from_slice(&fs::read(f.0.join("request.json")).unwrap()).unwrap();
    assert_eq!(request["command"], cmd);
    assert_eq!(request["confirmed"], true);
    assert_eq!(request["peer"], OWNER);
    fs::remove_file(f.0.join("request.json")).unwrap();
    assert_eq!(f.ok(f.input("retry", &id)), receipt);
    assert!(!f.0.join("request.json").exists());
    stage["command"]["payload"]["text"] = json!("Changed branch");
    assert!(!f.run(stage).status.success());
    assert_eq!(f.ok(f.input("status", &id))["command"], cmd);
    let dir = f.0.join("omarchy-t3-code/shared-drafts");
    assert_eq!(
        fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for file in fs::read_dir(dir).unwrap() {
        assert_eq!(
            file.unwrap().metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[test]
fn resync_and_conflict_preserve_original_branch_without_replay() {
    for mut response in [
        json!({"error":"draft_resync_required","floor":30}),
        json!({"status":"conflict","conflicts":[{"latest":null}]}),
    ] {
        let f = Fixture::new();
        let id = uuid::Uuid::new_v4().to_string();
        let cmd = command();
        let mut stage = f.input("stage", &id);
        stage["command"] = cmd.clone();
        f.ok(stage);
        if response["status"] == "conflict" {
            response["mutationId"] = cmd["mutationId"].clone();
        }
        f.response(response.clone());
        let result = f.ok(f.input("retry", &id));
        assert_eq!(result["conflict"], true);
        assert_eq!(result["outcome"], response);
        fs::remove_file(f.0.join("request.json")).unwrap();
        let recovered = f.ok(f.input("retry", &id));
        assert_eq!(recovered["command"], cmd);
        assert!(!f.0.join("request.json").exists());
    }
}
#[test]
fn uncertain_response_retries_original_command_and_invalid_commands_never_send() {
    let f = Fixture::new();
    let id = uuid::Uuid::new_v4().to_string();
    let cmd = command();
    let mut stage = f.input("stage", &id);
    stage["command"] = cmd.clone();
    f.ok(stage);
    fs::write(f.0.join("response.json"), "broken transport response").unwrap();
    assert!(!f.run(f.input("retry", &id)).status.success());
    assert_eq!(f.ok(f.input("status", &id))["command"], cmd);
    f.response(json!({"status":"applied","mutationId":cmd["mutationId"]}));
    f.ok(f.input("retry", &id));
    let req: Value = serde_json::from_slice(&fs::read(f.0.join("request.json")).unwrap()).unwrap();
    assert_eq!(req["command"], cmd);
    fs::remove_file(f.0.join("request.json")).unwrap();
    for change in [
        json!({"baseCursor":format!("{OWNER}:13")}),
        json!({"scope":{"environment":"local","project":"/tmp/repo","kind":"thread","id":"x"}}),
        json!({"secret":"rejected"}),
    ] {
        let mut invalid = cmd.clone();
        for (key, value) in change.as_object().unwrap() {
            invalid[key] = value.clone()
        }
        let mut stage = f.input("stage", &uuid::Uuid::new_v4().to_string());
        stage["command"] = invalid;
        assert!(!f.run(stage).status.success());
    }
    assert!(!f.0.join("request.json").exists());
}
#[test]
fn snapshot_rejects_foreign_owner_and_private_journal_rejects_symlinks() {
    let f = Fixture::new();
    f.response(json!({"owner":"another-owner","items":[],"cursor":format!("{OWNER}:12"),"nextOffset":null}));
    assert!(!f.run(f.input("snapshot", "")).status.success());
    f.response(json!({"owner":OWNER,"items":[],"cursor":format!("{OWNER}:12"),"nextOffset":null}));
    assert_eq!(f.ok(f.input("snapshot", ""))["items"], json!([]));
    let parent = f.0.join("omarchy-t3-code");
    fs::create_dir_all(&parent).unwrap();
    std::os::unix::fs::symlink(&f.0, parent.join("shared-drafts")).unwrap();
    let mut stage = f.input("stage", &uuid::Uuid::new_v4().to_string());
    stage["command"] = command();
    assert!(!f.run(stage).status.success());
}

#[test]
fn snapshot_finishes_all_pages_with_one_cursor_and_rejects_changed_or_stuck_pages() {
    let f = Fixture::new();
    let cursor = format!("{OWNER}:12");
    let first = json!({"owner":OWNER,"items":[{"scope":scope(),"deleted":false}],"cursor":cursor,"nextOffset":1});
    let last = json!({"owner":OWNER,"items":[{"scope":scope(),"deleted":true}],"cursor":cursor,"nextOffset":null});
    f.response(json!([first, last]));
    let result = f.ok(f.input("snapshot", ""));
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    let request: Value =
        serde_json::from_slice(&fs::read(f.0.join("request.json")).unwrap()).unwrap();
    assert_eq!(
        request["command"],
        json!({"op":"list","offset":1,"snapshot":cursor})
    );
    assert_eq!(request["confirmed"], false);
    f.response(
        json!([first,{"owner":OWNER,"items":[],"cursor":format!("{OWNER}:13"),"nextOffset":null}]),
    );
    assert!(!f.run(f.input("snapshot", "")).status.success());
    f.response(json!([first,{"owner":OWNER,"items":[],"cursor":cursor,"nextOffset":1}]));
    assert!(!f.run(f.input("snapshot", "")).status.success());
}

#[test]
fn foreign_receipt_cannot_acknowledge_pending_command() {
    let f = Fixture::new();
    let id = uuid::Uuid::new_v4().to_string();
    let cmd = command();
    let mut stage = f.input("stage", &id);
    stage["command"] = cmd.clone();
    f.ok(stage);
    f.response(json!({"status":"applied","mutationId":format!("12.{}",uuid::Uuid::new_v4())}));
    assert!(!f.run(f.input("retry", &id)).status.success());
    let recovered = f.ok(f.input("status", &id));
    assert_eq!(recovered["command"], cmd);
    assert!(recovered["outcome"].is_null());
    f.response(json!({"status":"applied","mutationId":cmd["mutationId"]}));
    assert_eq!(f.ok(f.input("retry", &id))["outcome"]["status"], "applied");
}

fn editor_record(f: &Fixture, text: &str) {
    f.ok(json!({"action":"editor-record","authority":OWNER,"key":"local/thread-one","draft":{"text":text,"attachments":[],"delivery":"auto","attempt":null,"modelSelection":null}}));
}
fn editor_sync(f: &Fixture, resolution: &str) -> Value {
    f.ok(json!({"action":"editor-sync","authority":OWNER,"key":"local/thread-one","scope":scope(),"resolution":resolution}))
}
fn snapshot_response(items: Value, sequence: u64) -> Value {
    json!({"owner":OWNER,"items":items,"cursor":format!("{OWNER}:{sequence}")})
}
fn editor_state(f: &Fixture) -> Value {
    let dir = f.0.join("omarchy-t3-code/shared-editor");
    let file = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.extension().is_some_and(|s| s == "json")
                && !p
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("resolved-")
        })
        .unwrap();
    serde_json::from_slice(&fs::read(file).unwrap()).unwrap()
}
#[test]
fn migration_stages_create_once_and_preserves_legacy_cache() {
    let f = Fixture::new();
    editor_record(&f, "legacy branch");
    assert!(
        !f.0.join("request.json").exists(),
        "offline cache write must not access bridge"
    );
    f.response(snapshot_response(json!([]), 12));
    let first = editor_sync(&f, "");
    assert_eq!(first["pending"], true);
    let state = editor_state(&f);
    let pending = state["pending"].clone();
    assert_eq!(pending["command"]["scope"], scope());
    assert_eq!(pending["command"]["expected"], Value::Null);
    assert_eq!(pending["command"]["payload"]["schema"], 1);
    assert_eq!(state["draft"]["text"], "legacy branch");
    // A failed read before retry cannot regenerate the persisted identity.
    f.response(json!({"error":"draft_snapshot_required"}));
    let blocked = editor_sync(&f, "");
    assert_eq!(blocked["error"], "draft_snapshot_required");
    assert_eq!(editor_state(&f)["pending"], pending);
    assert_eq!(editor_state(&f)["draft"]["text"], "legacy branch");
}
#[test]
fn import_collision_keeps_both_and_resolution_archives_original() {
    let f = Fixture::new();
    editor_record(&f, "local branch");
    let doc = json!({"scope":scope(),"revision":"remote-revision","deleted":false,"payload":{"schema":1,"text":"remote branch","attachments":[],"preferences":{"deliveryMode":"send","interactionMode":"default"}}});
    f.response(snapshot_response(json!([doc]), 20));
    let result = editor_sync(&f, "");
    assert_eq!(result["conflict"], true);
    assert_eq!(result["draft"]["text"], "local branch");
    assert_eq!(result["remote"]["payload"]["text"], "remote branch");
    assert_eq!(result["pending"], false);
    let resolved = editor_sync(&f, "local");
    assert_eq!(resolved["pending"], true);
    let s = editor_state(&f);
    assert_eq!(s["pending"]["command"]["expected"], "remote-revision");
    assert_eq!(s["pending"]["command"]["baseCursor"], format!("{OWNER}:20"));
    assert!(fs::read_dir(f.0.join("omarchy-t3-code/shared-editor"))
        .unwrap()
        .any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("resolved-")));
}
#[test]
fn clean_empty_editor_does_not_create_a_shared_document() {
    let f = Fixture::new();
    editor_record(&f, "");
    f.response(snapshot_response(json!([]), 12));
    let result = editor_sync(&f, "");
    assert_eq!(result["pending"], false);
    assert_eq!(result["conflict"], false);
    assert!(editor_state(&f)["pending"].is_null());
}
#[test]
fn shared_payload_excludes_local_recovery_and_rejects_unfinished_uploads() {
    use omarchy_t3_client::shared_editor::payload;
    let p=payload(&json!({"text":"plain source","attachments":[],"questionnaires":{"private":"answer"},"approvals":{"private":true},"recoveryIds":["local-only"],"modelSelection":{"instanceId":"provider","model":"model","options":[{"id":"effort","value":"high"}]}}),"11111111-1111-4111-8111-111111111111").unwrap();
    assert_eq!(
        p,
        json!({"schema":1,"text":"plain source","attachments":[],"modelSelection":{"providerId":"provider","modelId":"model","effort":"high"},"preferences":{"deliveryMode":"send","interactionMode":"default"}})
    );
    assert!(payload(
        &json!({"text":"text","attachments":[{"uploading":true}]}),
        "env"
    )
    .is_err());
    assert!(payload(&json!({"text":"text","queueEdit":{}}), "env").is_err());
    assert!(payload(
        &json!({"text":"text","modelSelection":{"options":[{"id":"secret","value":"x"}]}}),
        "env"
    )
    .is_err());
}

#[test]
fn acknowledged_import_is_not_duplicated_and_send_clear_uses_submitted_revision() {
    let f = Fixture::new();
    editor_record(&f, "submitted text");
    f.response(snapshot_response(json!([]), 12));
    editor_sync(&f, "");
    let staged = editor_state(&f)["pending"].clone();
    let payload = staged["command"]["payload"].clone();
    let document =
        json!({"scope":scope(),"revision":"submitted-revision","deleted":false,"payload":payload});
    f.response(json!({"__byOp":{"list":snapshot_response(json!([document]),13),"put":{"status":"applied","mutationId":"$mutation","event":{"revision":"submitted-revision"}}}}));
    let ack = editor_sync(&f, "");
    assert_eq!(ack["pending"], false);
    assert_eq!(ack["conflict"], false);
    editor_sync(&f, "");
    assert!(
        editor_state(&f)["pending"].is_null(),
        "acknowledged import must not generate another command"
    );
    editor_record(&f, "");
    let clear = editor_sync(&f, "");
    assert_eq!(clear["pending"], true);
    let command = editor_state(&f)["pending"]["command"].clone();
    assert_eq!(command["op"], "delete");
    assert_eq!(command["expected"], "submitted-revision");
    assert!(command.get("payload").is_none());
}
#[test]
fn newer_remote_edit_survives_local_send_clear_as_visible_conflict() {
    let f = Fixture::new();
    editor_record(&f, "submitted text");
    f.response(snapshot_response(json!([]), 12));
    editor_sync(&f, "");
    let payload = editor_state(&f)["pending"]["command"]["payload"].clone();
    let doc = json!({"scope":scope(),"revision":"submitted","deleted":false,"payload":payload});
    f.response(json!({"__byOp":{"list":snapshot_response(json!([doc]),13),"put":{"status":"applied","mutationId":"$mutation","event":{"revision":"submitted"}}}}));
    editor_sync(&f, "");
    editor_record(&f, "");
    let doc = json!({"scope":scope(),"revision":"other-device","deleted":false,"payload":{"schema":1,"text":"newer unsent edit","attachments":[]}});
    f.response(snapshot_response(json!([doc]), 14));
    let clear = editor_sync(&f, "");
    assert_eq!(clear["conflict"], true);
    assert_eq!(clear["remote"]["payload"]["text"], "newer unsent edit");
    assert_eq!(clear["pending"], false);
}
#[test]
fn compaction_blocks_pending_create_even_when_fresh_snapshot_key_is_absent() {
    let f = Fixture::new();
    editor_record(&f, "offline create");
    f.response(snapshot_response(json!([]), 12));
    editor_sync(&f, "");
    let original = editor_state(&f)["pending"].clone();
    f.response(json!({"__byOp":{"list":snapshot_response(json!([]),100),"put":{"error":"draft_resync_required","status":"conflict","mutationId":"$mutation","floor":99}}}));
    let conflict = editor_sync(&f, "");
    assert_eq!(conflict["conflict"], true);
    assert_eq!(editor_state(&f)["pending"], original);
    editor_sync(&f, "");
    assert_eq!(
        editor_state(&f)["pending"],
        original,
        "resync cannot rebase or replay an expired create"
    );
    let resolve = editor_sync(&f, "local");
    assert_eq!(resolve["conflict"], false);
    let resolved = editor_state(&f)["pending"].clone();
    assert_ne!(resolved["id"], original["id"]);
    assert_eq!(resolved["command"]["baseCursor"], format!("{OWNER}:100"));
}

#[test]
fn restore_uses_atomic_transfer_and_backs_up_even_a_live_empty_target() {
    let f = Fixture::new();
    let mut source = scope();
    source["kind"] = json!("stash");
    source["id"] = json!(uuid::Uuid::new_v4().to_string());
    let payload = json!({"schema":1,"text":"stashed text","attachments":[]});
    let docs = json!([{ "scope":source,"revision":"source-revision","deleted":false,"payload":payload},{"scope":scope(),"revision":"live-empty-target","deleted":false,"payload":{"schema":1,"text":"","attachments":[],"preferences":{"deliveryMode":"send","interactionMode":"default"}}}]);
    f.response(json!({"__byOp":{"list":snapshot_response(docs,23),"transfer":{"status":"conflict","mutationId":"$mutation","conflicts":[]}}}));
    let request = uuid::Uuid::new_v4().to_string();
    let command = json!({"action":"restore","projectId":scope()["project"],"key":"local/thread-one","requestId":request,"id":source["id"]});
    let input = json!({"action":"editor-project","authority":OWNER,"environment":scope()["environment"],"command":command});
    assert!(!f.run(input.clone()).status.success());
    let staged = f.ok(f.input("status", &request))["command"].clone();
    assert_eq!(staged["op"], "transfer");
    assert_eq!(staged["scope"], source);
    assert_eq!(staged["target"], scope());
    assert_eq!(staged["targetExpected"], "live-empty-target");
    assert_eq!(staged["expected"], "source-revision");
    assert_eq!(staged["backup"]["kind"], "stash");
    assert_eq!(staged["backup"]["environment"], scope()["environment"]);
    assert_ne!(staged["backup"]["id"], source["id"]);
    assert!(!f.run(input).status.success());
    assert_eq!(f.ok(f.input("status", &request))["command"], staged);
}
