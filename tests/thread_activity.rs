use omarchy_t3_client::thread_activity;
use serde_json::{json, Value};
#[test]
fn native_whole_shell_presentation_keeps_activity_completion_boundaries() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/thread-activity-presentation-4301.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 144);
    for case in fixture["cases"].as_array().unwrap() {
        assert_eq!(
            thread_activity::shell_clock(&case["input"]),
            Some(case["presented"].clone()),
            "{case}"
        );
        let rows = omarchy_t3_client::normalize(
            &json!({"threads":[case["input"].clone()],"projects":[]}),
            "env",
            0,
        )
        .unwrap();
        assert_eq!(
            rows[0]["activityClock"], case["presented"],
            "real shell row producer: {case}"
        )
    }
    assert!(
        thread_activity::shell_clock(&json!({"updatedAt":100,"latestRunId":null})).is_none(),
        "legacy numeric clocks are not reinterpreted as native ISO records"
    );
}

#[test]
fn actual_live_shell_deltas_refresh_label_inputs_without_reinterpreting_history() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/thread-activity-presentation-4301.json"
    ))
    .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let before = cases
        .iter()
        .find(|case| {
            case["input"]["status"] == "running"
                && case["input"]["latestRunId"] == "run"
                && case["input"]["latestRunCompletedAt"].is_null()
                && case["input"]["latestRunRequestedAt"] == "2026-11-07T01:00:00.000Z"
                && case["input"]["latestUserMessageAt"].is_null()
        })
        .unwrap();
    let after = cases
        .iter()
        .find(|case| {
            case["input"]["status"] == "completed"
                && case["input"]["latestRunId"] == "run"
                && case["input"]["latestRunCompletedAt"] == "2026-11-07T03:00:00.000Z"
                && case["input"]["latestRunRequestedAt"] == "2026-11-07T01:00:00.000Z"
                && case["input"]["latestUserMessageAt"].is_null()
        })
        .unwrap();
    let environment = "thread-activity-stream-fixture";
    assert!(omarchy_t3_client::shell_state::apply(
        environment,
        &json!({"kind":"snapshot","snapshot":{"schemaVersion":1,"snapshotSequence":1,"threads":[before["input"].clone()],"archivedThreads":[],"projects":[]}})
    ));
    omarchy_t3_client::shell_state::set_live(environment, true);
    let rows = omarchy_t3_client::normalize(
        &omarchy_t3_client::shell_state::live(environment).unwrap(),
        environment,
        0,
    )
    .unwrap();
    assert_eq!(rows[0]["activityClock"], before["presented"]);
    assert!(omarchy_t3_client::shell_state::apply(
        environment,
        &json!({"kind":"thread.updated","sequence":2,"location":"active","thread":after["input"].clone()})
    ));
    let rows = omarchy_t3_client::normalize(
        &omarchy_t3_client::shell_state::live(environment).unwrap(),
        environment,
        0,
    )
    .unwrap();
    assert_eq!(rows[0]["activityClock"], after["presented"]);
    assert!(!omarchy_t3_client::shell_state::apply(
        environment,
        &json!({"kind":"thread.updated","sequence":1,"location":"active","thread":before["input"].clone()})
    ));
    let rows = omarchy_t3_client::normalize(
        &omarchy_t3_client::shell_state::live(environment).unwrap(),
        environment,
        0,
    )
    .unwrap();
    assert_eq!(
        rows[0]["activityClock"], after["presented"],
        "stale shell event cannot restore an older label clock"
    );
    omarchy_t3_client::shell_state::forget(environment);
}
