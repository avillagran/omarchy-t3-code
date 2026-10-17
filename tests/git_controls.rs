// Owned module compiles without touching the parent's shared lib.rs registry.
pub use omarchy_t3_client::{environment_token, protocol, Environment};
#[path = "../src/git_controls.rs"]
mod git_controls;
use serde_json::{json, Value};

fn shell() -> Value {
    json!({"projects":[{"id":"p","workspaceRoot":"/remote/project","scripts":[]}],
        "threads":[{"id":"t","projectId":"p","worktreePath":"/remote/tree with spaces"}]})
}
fn input() -> Value {
    json!({"requestId":"r1","environment":"remote","thread":"t","projectId":"p","cwd":"/remote/tree with spaces"})
}
fn status() -> Value {
    json!({"isRepo":true,"hasPrimaryRemote":true,"isDefaultRef":false,"refName":"feature/x",
        "hasWorkingTreeChanges":true,"hasUpstream":true,"aheadCount":1,"behindCount":0,"pr":null,
        "workingTree":{"files":[{"path":"file with spaces.rs","insertions":2,"deletions":1}],"insertions":2,"deletions":1}})
}
fn script(id: &str) -> Value {
    json!({"id":id,"name":"Test <b>plain</b>","command":"npm test && printf 'done'","icon":"test","runOnWorktreeCreate":false})
}
fn snapshot() -> Value {
    git_controls::present(
        &git_controls::scope(&shell(), &input()).unwrap(),
        &status(),
        &json!([script("test")]),
    )
    .unwrap()
}
fn intent(current: &Value, operation: &str) -> Value {
    json!({"scope":current["scope"],"revision":current["revision"],"operation":operation,
        "actionId":"dd38bad7-c625-438d-9f12-165f25521753","confirmed":true})
}

#[test]
fn native_script_resolution_precedence_including_empty_override_and_legacy_null() {
    let project = json!({"id":"p","scripts":[script("aggregate")]});
    let mut settings = json!({"defaultProjectScripts":[script("default")],"projectScriptOverrides":{},"projectSettingsOverrides":{},"projectSettingsFolded":false});
    assert_eq!(
        git_controls::resolve_scripts(&settings, &project).unwrap()[0]["id"],
        "aggregate"
    );
    settings["projectScriptOverrides"]["p"] = Value::Null;
    assert_eq!(
        git_controls::resolve_scripts(&settings, &project).unwrap()[0]["id"],
        "default"
    );
    settings["projectScriptOverrides"]["p"] = json!([script("legacy")]);
    assert_eq!(
        git_controls::resolve_scripts(&settings, &project).unwrap()[0]["id"],
        "legacy"
    );
    settings["projectSettingsFolded"] = json!(true);
    assert_eq!(
        git_controls::resolve_scripts(&settings, &project).unwrap()[0]["id"],
        "default"
    );
    settings["projectSettingsOverrides"]["p"] = json!({"defaultProjectScripts":[]});
    assert_eq!(
        git_controls::resolve_scripts(&settings, &project).unwrap(),
        json!([])
    );
}

#[test]
fn checkout_identity_deleted_and_late_results() {
    let current = snapshot();
    assert_eq!(current["scope"]["cwd"], "/remote/tree with spaces");
    assert_eq!(current["scope"]["projectRoot"], "/remote/project");
    let mut receipt = input();
    receipt["result"] = current;
    assert!(git_controls::scoped_result(&input(), &receipt)
        .unwrap()
        .is_ok());
    for k in ["requestId", "environment", "thread", "cwd", "projectId"] {
        let mut late = receipt.clone();
        late[k] = json!("elsewhere");
        assert!(git_controls::scoped_result(&input(), &late).is_none());
    }
    receipt["error"] = json!("unauthorized");
    assert_eq!(
        git_controls::scoped_result(&input(), &receipt)
            .unwrap()
            .unwrap_err(),
        "unauthorized"
    );
    let mut changed = input();
    changed["cwd"] = json!("/local/wrong");
    assert_eq!(
        git_controls::scope(&shell(), &changed).unwrap_err(),
        "git_scope_changed"
    );
    let mut deleted = shell();
    deleted["threads"][0]["deletedAt"] = json!("2026-10-17");
    assert_eq!(
        git_controls::scope(&deleted, &input()).unwrap_err(),
        "git_thread_unavailable"
    );
}

#[test]
fn native_mutation_payloads_confirmation_default_branch_and_selected_files() {
    let current = snapshot();
    let mut i = intent(&current, "commit_push_pr");
    i["commitMessage"] = json!("Keep the supplied message");
    i["filePaths"] = json!(["file with spaces.rs"]);
    let plan = git_controls::build_action(&current, &i).unwrap();
    assert_eq!(
        plan["steps"],
        json!([{"tag":"git.runStackedAction","payload":{
        "actionId":i["actionId"],"cwd":"/remote/tree with spaces","action":"commit_push_pr","threadId":"t",
        "featureBranch":false,"commitMessage":"Keep the supplied message","filePaths":["file with spaces.rs"]}}])
    );
    assert_eq!(plan["retryAutomatically"], false);
    i["confirmed"] = json!(false);
    assert_eq!(
        git_controls::build_action(&current, &i).unwrap_err(),
        "git_confirmation_required"
    );
    i["confirmed"] = json!(true);
    i["filePaths"] = json!(["not-observed"]);
    assert_eq!(
        git_controls::build_action(&current, &i).unwrap_err(),
        "invalid_git_files"
    );
    let mut default_status = status();
    default_status["isDefaultRef"] = json!(true);
    let default =
        git_controls::present(&current["scope"], &default_status, &current["scripts"]).unwrap();
    let mut i = intent(&default, "push");
    assert_eq!(
        git_controls::build_action(&default, &i).unwrap_err(),
        "git_default_branch_confirmation_required"
    );
    i["defaultBranchConfirmed"] = json!(true);
    assert!(git_controls::build_action(&default, &i).is_ok());
    let mut stale = i.clone();
    stale["revision"] = json!("old");
    assert_eq!(
        git_controls::build_action(&default, &stale).unwrap_err(),
        "git_stale_result"
    );
    let mut malformed = i;
    malformed["actionId"] = json!("repair-me");
    assert_eq!(
        git_controls::build_action(&default, &malformed).unwrap_err(),
        "invalid_git_actionId"
    );
}

#[test]
fn scripts_use_native_open_then_write_with_exact_runtime_environment_and_no_shell_runner() {
    let current = snapshot();
    let mut i = intent(&current, "script");
    i["scriptId"] = json!("test");
    i["command"] = json!("must never be used");
    let plan = git_controls::build_action(&current, &i).unwrap();
    let steps = plan["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["tag"], "terminal.open");
    assert_eq!(
        steps[0]["payload"]["env"],
        json!({"T3CODE_PROJECT_ROOT":"/remote/project","T3CODE_WORKTREE_PATH":"/remote/tree with spaces"})
    );
    assert_eq!(
        steps[0]["payload"]["worktreePath"],
        "/remote/tree with spaces"
    );
    assert_eq!(steps[1]["tag"], "terminal.write");
    assert_eq!(
        steps[1]["payload"]["terminalId"],
        steps[0]["payload"]["terminalId"]
    );
    assert_eq!(steps[1]["payload"]["data"], "npm test && printf 'done'\r");
    i["scriptId"] = json!("missing");
    assert_eq!(
        git_controls::build_action(&current, &i).unwrap_err(),
        "git_script_unavailable"
    );
}

#[test]
fn native_action_gates_detached_diverged_no_remote_clean_and_pull() {
    let mut s = status();
    s["refName"] = Value::Null;
    assert_eq!(git_controls::actions(&s)["commit"], true);
    assert_eq!(git_controls::actions(&s)["push"], false);
    assert_eq!(git_controls::actions(&s)["commit_push_pr"], false);
    s = status();
    s["behindCount"] = json!(2);
    for op in ["push", "pull", "commit_push", "create_pr"] {
        assert_eq!(git_controls::actions(&s)[op], false);
    }
    s["aheadCount"] = json!(0);
    s["hasWorkingTreeChanges"] = json!(false);
    let current = git_controls::present(&snapshot()["scope"], &s, &json!([])).unwrap();
    assert_eq!(
        git_controls::build_action(&current, &intent(&current, "pull")).unwrap()["steps"],
        json!([{"tag":"vcs.pull","payload":{"cwd":"/remote/tree with spaces"}}])
    );
    s = status();
    s["hasPrimaryRemote"] = json!(false);
    s["hasUpstream"] = json!(false);
    assert_eq!(git_controls::actions(&s)["commit"], true);
    assert_eq!(git_controls::actions(&s)["push"], false);
    s = status();
    s["hasWorkingTreeChanges"] = json!(false);
    s["aheadCount"] = json!(0);
    assert_eq!(git_controls::actions(&s)["commit"], false);
}

#[test]
fn malformed_and_oversized_results_fail_closed() {
    let mut s = status();
    s["aheadCount"] = json!(-1);
    assert!(git_controls::validate_status(&s).is_err());
    s = status();
    s["workingTree"]["files"] = json!(vec![
        json!({"path":"a","insertions":0,"deletions":0});
        git_controls::MAX_FILES + 1
    ]);
    assert_eq!(
        git_controls::validate_status(&s).unwrap_err(),
        "git_files_limit"
    );
    let settings = json!({"defaultProjectScripts":vec![script("same");git_controls::MAX_SCRIPTS+1],"projectSettingsFolded":true});
    assert_eq!(
        git_controls::resolve_scripts(&settings, &json!({"id":"p"})).unwrap_err(),
        "git_scripts_limit"
    );
    s = status();
    s["refName"] = json!("x".repeat(1024 * 1024));
    assert_eq!(
        git_controls::validate_status(&s).unwrap_err(),
        "git_result_limit"
    );
}

#[test]
fn fresh_read_compares_content_not_request_identity_and_preserves_literal_filenames() {
    let current = snapshot();
    let mut fresh_scope = current["scope"].clone();
    fresh_scope["requestId"] = json!("verification-read");
    let fresh =
        git_controls::present(&fresh_scope, &current["status"], &current["scripts"]).unwrap();
    assert!(
        git_controls::build_verified_action(&current, &fresh, &intent(&current, "commit")).is_ok()
    );
    let mut changed = current["status"].clone();
    changed["refName"] = json!("another-branch");
    let changed = git_controls::present(&fresh_scope, &changed, &current["scripts"]).unwrap();
    assert_eq!(
        git_controls::build_verified_action(&current, &changed, &intent(&current, "commit"))
            .unwrap_err(),
        "git_stale_result"
    );
    let mut status = status();
    status["workingTree"]["files"][0]["path"] = json!(" leading and trailing ");
    let literal = git_controls::present(&fresh_scope, &status, &current["scripts"]).unwrap();
    let mut i = intent(&literal, "commit");
    i["filePaths"] = json!([" leading and trailing "]);
    assert_eq!(
        git_controls::build_action(&literal, &i).unwrap()["steps"][0]["payload"]["filePaths"],
        i["filePaths"]
    );
}

#[test]
fn production_read_adapter_uses_actual_native_http_and_rpc_contracts_only() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        thread,
        time::Duration,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        let mut header = Vec::new();
        let mut byte = [0];
        while !header.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
            assert!(header.len() < 16384);
        }
        assert!(String::from_utf8(header)
            .unwrap()
            .starts_with("GET /api/orchestration/shell HTTP/1.1"));
        let body = shell().to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        drop(stream);
        for (tag, payload, result) in [
            (
                "server.getSettings",
                json!({}),
                json!({"defaultProjectScripts":[script("test")],"projectSettingsFolded":true}),
            ),
            (
                "vcs.refreshStatus",
                json!({"cwd":"/remote/tree with spaces"}),
                status(),
            ),
        ] {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(request["tag"], tag);
            assert_eq!(request["payload"], payload);
            ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":request["id"].as_str().unwrap_or("1"),"exit":{"_tag":"Success","value":result}}).to_string().into())).unwrap();
        }
    });
    let connection = protocol::Connection::new(
        &base,
        None,
        protocol::Credential::bearer("fixture-only").unwrap(),
    )
    .unwrap();
    let e = Environment {
        id: "remote".into(),
        name: "Fixture".into(),
        base_url: base,
        token_file: "/unused".into(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(connection)),
    };
    let output = git_controls::execute(&e, &input()).unwrap();
    server.join().unwrap();
    assert_eq!(output["scripts"][0]["id"], "test");
    assert_eq!(output["status"]["refName"], "feature/x");
    assert_eq!(output["scope"]["environment"], "remote");
    let mut wrong = input();
    wrong["environment"] = json!("other");
    assert_eq!(
        git_controls::execute(&e, &wrong).unwrap_err(),
        "git_scope_changed"
    );
}
