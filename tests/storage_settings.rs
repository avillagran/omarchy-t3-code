use omarchy_t3_client::{native_settings, scoped_settings, storage_settings as s};
use serde_json::{json, Value};
fn raw() -> Value {
    json!({"storageCleanup":{"worktreeKeepWhen":"uncommitted-changes","worktreeAfterDays":8,"worktreeOnMerge":true,"worktreeOnDelete":false,"worktreeUnchanged":true,"browserArtifactsAfterDays":null,"logsAfterDays":30},"worktreeCleanup":null,"worktreesDirectory":"","providerInstances":{"secret":"PRIVATE"}})
}
#[test]
fn matches_original_resolver_after_scope_folding() {
    let fixtures: Value =
        serde_json::from_str(include_str!("../data/storage-rule-cases.json")).unwrap();
    for c in fixtures["cases"].as_array().unwrap() {
        assert_eq!(s::resolved_rules(&c["settings"]).unwrap(), c["expected"]);
    }
}
#[test]
fn typed_partial_patches_preserve_siblings_and_exact_retention_bounds() {
    for days in [json!(null), json!(1), json!(1.0), json!(3650)] {
        assert_eq!(
            s::patch(&json!({"key":"storageCleanup.logsAfterDays","value":days})).unwrap(),
            json!({"patch":{"storageCleanup":{"logsAfterDays":days}}})
        );
    }
    for days in [
        json!(0),
        json!(3651),
        json!(-1),
        json!(1.5),
        json!("8"),
        json!(true),
    ] {
        assert!(s::patch(&json!({"key":"storageCleanup.logsAfterDays","value":days})).is_err());
    }
    assert_eq!(
        s::patch(&json!({"key":"worktreesDirectory","value":" ~/worktrees "})).unwrap()["patch"]
            ["worktreesDirectory"],
        "~/worktrees"
    );
    for (key, value) in [
        ("storageCleanup.deleteEverything", json!(true)),
        ("storageCleanup.worktreeOnMerge", json!("true")),
        ("storageCleanup.worktreeKeepWhen", json!("unknown")),
        ("storageCleanup", json!({})),
    ] {
        assert!(s::patch(&json!({"key":key,"value":value})).is_err());
    }
}
#[test]
fn project_rule_edit_keeps_other_rules_and_switching_off_removes_custom_shape() {
    let mut settings = raw();
    settings["projectSettingsOverrides"] = json!({"p":{"branchNamePrefix":"keep","worktreeCleanup":{"mode":"custom","rules":{"worktreeKeepWhen":"any-local-files","worktreeAfterDays":22,"worktreeOnMerge":false,"worktreeOnDelete":true,"worktreeUnchanged":false}}}});
    let project = json!({"id":"p"});
    let patch=s::patch(&json!({"key":"worktreeCleanup","value":{"mode":"custom","rules":{"worktreeOnMerge":true}}})).unwrap();
    let out = scoped_settings::project_patch(&settings, &project, "worktreeCleanup", &patch, false)
        .unwrap();
    let entry = &out["patch"]["projectSettingsOverrides"]["p"];
    assert_eq!(entry["branchNamePrefix"], "keep");
    assert_eq!(
        entry["worktreeCleanup"]["rules"],
        json!({"worktreeKeepWhen":"any-local-files","worktreeAfterDays":22,"worktreeOnMerge":true,"worktreeOnDelete":true,"worktreeUnchanged":false})
    );
    let off = scoped_settings::project_patch(
        &settings,
        &project,
        "worktreeCleanup",
        &json!({"patch":{"worktreeCleanup":{"mode":"off"}}}),
        false,
    )
    .unwrap();
    assert_eq!(
        off["patch"]["projectSettingsOverrides"]["p"]["worktreeCleanup"],
        json!({"mode":"off"})
    );
    let inherit = scoped_settings::project_patch(
        &settings,
        &project,
        "worktreeCleanup",
        &json!({"patch":{"worktreeCleanup":null}}),
        false,
    )
    .unwrap();
    assert_eq!(
        inherit["patch"]["projectSettingsOverrides"]["p"],
        json!({"branchNamePrefix":"keep"})
    );
}
#[test]
fn snapshots_are_secret_free_and_writes_require_advertised_capabilities() {
    let fields = s::fields(&raw());
    assert_eq!(fields.len(), 9);
    assert!(!json!(fields).to_string().contains("PRIVATE"));
    let days = fields
        .iter()
        .find(|f| f["key"] == "storageCleanup.logsAfterDays")
        .unwrap();
    assert_eq!(days["maximum"], 3650);
    assert_eq!(days["enabledValue"], 8);
    let input = json!({"key":"storageCleanup.worktreeOnDelete","value":true});
    assert!(native_settings::patch(&input, None).is_err());
    assert!(native_settings::patch(&input, Some(&json!({}))).is_err());
    assert!(native_settings::patch(
        &input,
        Some(&json!({"environment":{"capabilities":{"storageCleanup":true}}}))
    )
    .is_ok());
    assert!(native_settings::patch(
        &json!({"key":"storageCleanup.worktreeKeepWhen","value":"tracked-changes"}),
        Some(&json!({"environment":{"capabilities":{"storageCleanup":true}}}))
    )
    .is_err());
}

#[test]
fn environment_rule_edits_complete_a_root_policy_and_project_mixed_rules_stay_distinct() {
    let mut raw = raw();
    raw["worktreeCleanup"] = json!({"mode":"off"});
    let fields = s::fields(&raw);
    assert_eq!(
        fields
            .iter()
            .find(|f| f["key"] == "storageCleanup.worktreeOnMerge")
            .unwrap()["value"],
        false
    );
    let input = json!({"key":"storageCleanup.worktreeOnDelete","value":true});
    let result = s::environment_patch(
        &raw,
        "storageCleanup.worktreeOnDelete",
        &s::patch(&input).unwrap(),
    )
    .unwrap();
    assert_eq!(
        result["patch"]["worktreeCleanup"],
        json!({"mode":"custom","rules":{"worktreeKeepWhen":"uncommitted-changes","worktreeAfterDays":null,"worktreeOnMerge":false,"worktreeOnDelete":true,"worktreeUnchanged":false}})
    );
    let first = json!({"fields":[{"key":"worktreeCleanup","kind":"worktree-policy","value":null,"rules":{"worktreeAfterDays":8}}]});
    let mut second = first.clone();
    second["fields"][0]["rules"]["worktreeAfterDays"] = json!(22);
    let result = scoped_settings::aggregate(&[first.clone(), second]);
    assert_eq!(result["fields"][0]["mixed"], false);
    assert_eq!(result["fields"][0]["rulesMixed"]["worktreeAfterDays"], true);
}

#[test]
fn actual_native_rpc_uses_original_patch_and_returns_only_safe_descriptors() {
    use omarchy_t3_client::{protocol, Environment};
    use std::{
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let capture = seen.clone();
    let worker = thread::spawn(move || {
        let mut settings = raw();
        settings["worktreeCleanup"] = json!({"mode":"off"});
        for index in 0..3 {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(4)))
                .unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            capture.lock().unwrap().push(request.clone());
            let result = match index {
                0 => {
                    json!({"environment":{"capabilities":{"storageCleanup":true}},"settings":settings})
                }
                1 => {
                    assert_eq!(request["tag"], "server.updateSettings");
                    let patch = &request["payload"]["patch"];
                    settings["storageCleanup"]["worktreeOnDelete"] =
                        patch["storageCleanup"]["worktreeOnDelete"].clone();
                    settings["worktreeCleanup"] = patch["worktreeCleanup"].clone();
                    json!({})
                }
                _ => settings.clone(),
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
        protocol::Credential::bearer("synthetic-fixture").unwrap(),
    )
    .unwrap();
    let environment = Environment {
        id: "fixture".into(),
        name: "Fixture".into(),
        base_url: base,
        token_file: "/unused".into(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(connection)),
    };
    let view = native_settings::execute(
        &environment,
        &json!({"action":"update","key":"storageCleanup.worktreeOnDelete","value":true}),
    )
    .unwrap();
    worker.join().unwrap();
    let calls = seen.lock().unwrap();
    assert_eq!(calls[0]["tag"], "server.getConfig");
    assert_eq!(calls[2]["tag"], "server.getSettings");
    assert_eq!(
        calls[1]["payload"]["patch"]["worktreeCleanup"]["rules"]["worktreeOnDelete"],
        true
    );
    assert_eq!(
        calls[1]["payload"]["patch"]["worktreeCleanup"]["rules"]["worktreeOnMerge"],
        false
    );
    assert!(!view.to_string().contains("PRIVATE"));
    assert_eq!(
        view["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["key"] == "storageCleanup.worktreeOnDelete")
            .unwrap()["value"],
        true
    );
}
