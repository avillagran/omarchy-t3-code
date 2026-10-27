use omarchy_t3_client::session_memory::{candidate, command, compaction_candidate, minutes};
use serde_json::{json, Value};
fn fixture() -> Value {
    json!({"thread":{"id":"t","activeRunId":null,"activeProviderThreadId":"pt","modelSelection":{"instanceId":"work"},"latestUserAuthoredMessageAt":1000},"runs":[{"status":"completed","completedAt":2000}],"providerThreads":[{"id":"pt","appThreadId":"t","providerInstanceId":"work","providerSessionId":"s","driver":"codex","nativeThreadRef":{"driver":"codex","nativeId":"saved"}}],"providerSessions":[{"id":"s","providerInstanceId":"work","driver":"codex","status":"ready","createdAt":1000}]})
}
#[test]
fn enabled_by_default_with_provider_opt_out() {
    assert_eq!(minutes(&json!({}), "e", "work"), Some(5));
    assert_eq!(
        minutes(&json!({"idleReleaseEnabled":false}), "e", "work"),
        None
    );
    assert_eq!(
        minutes(
            &json!({"idleReleaseMinutes":10,"idleReleaseProviderMinutes":{"e/work":null}}),
            "e",
            "work"
        ),
        None
    );
    assert_eq!(
        minutes(
            &json!({"idleReleaseProviderMinutes":{"e/work":1}}),
            "other",
            "work"
        ),
        Some(5)
    );
}
#[test]
fn release_preserves_conversation_and_does_not_compact() {
    let p = fixture();
    assert_eq!(
        candidate(&p, &json!({}), "e", 301999)["status"],
        "scheduled"
    );
    let due = candidate(&p, &json!({}), "e", 302000);
    assert_eq!(due["status"], "due");
    let c = command(&due);
    assert_eq!(c["type"], "provider-session.detach");
    assert_eq!(c["providerSessionId"], "s");
    assert!(c.get("text").is_none());
    assert!(c.get("nativeThreadRef").is_none());
    let mut renewed = p.clone();
    renewed["providerSessions"][0]["createdAt"] = json!(300000);
    assert_eq!(
        candidate(&renewed, &json!({}), "e", 302000)["status"],
        "scheduled"
    );
    assert_ne!(
        candidate(&renewed, &json!({}), "e", 700000)["episode"],
        due["episode"]
    );
}
#[test]
fn active_queued_pending_and_background_work_prevent_release() {
    for (field, value) in [
        ("runs", json!([{"status":"queued"}])),
        ("nodes", json!([{"status":"running"}])),
        ("subagents", json!([{"status":"waiting"}])),
        ("runtimeRequests", json!([{"status":"pending"}])),
        ("plans", json!([{"status":"proposed"}])),
    ] {
        let mut p = fixture();
        p[field] = value;
        assert_eq!(
            candidate(&p, &json!({}), "e", 900000)["status"],
            "busy",
            "{field}"
        );
    }
    let mut p = fixture();
    p["thread"]["activeRunId"] = json!("new-run");
    assert_eq!(candidate(&p, &json!({}), "e", 900000)["status"], "busy");
}
#[test]
fn unknown_acp_never_loses_an_unresumable_conversation() {
    let mut p = fixture();
    p["providerSessions"][0]["driver"] = json!("acpRegistry");
    assert_eq!(
        candidate(&p, &json!({}), "e", 900000)["status"],
        "unsupported"
    );
    p["providerSessions"][0]["capabilities"] = json!({"threads":{"canReadThreadSnapshot":true}});
    assert_eq!(candidate(&p, &json!({}), "e", 900000)["status"], "due");
    p["providerThreads"][0]["nativeThreadRef"] = Value::Null;
    assert_eq!(
        candidate(&p, &json!({}), "e", 900000)["status"],
        "unsupported"
    );
}
#[test]
fn codex_idle_compaction_is_harness_managed_even_for_custom_instances() {
    assert_eq!(
        compaction_candidate(&fixture(), &json!({}), 900000)["status"],
        "harness_managed"
    );
    let mut p = fixture();
    p["providerThreads"][0]["driver"] = json!("claudeAgent");
    p["providerThreads"][0]["nativeThreadRef"]["driver"] = json!("claudeAgent");
    assert_ne!(
        compaction_candidate(&p, &json!({}), 900000)["status"],
        "harness_managed"
    );
}
