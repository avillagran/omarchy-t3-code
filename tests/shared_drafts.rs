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
