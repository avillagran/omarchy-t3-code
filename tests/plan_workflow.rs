use omarchy_t3_client::plan_workflow;
use serde_json::json;

fn projection() -> serde_json::Value {
    json!({"thread":{"id":"thread-1","interactionMode":"plan"},"updatedAt":"2026-10-17T10:00:00Z",
      "runs":[{"id":"run-older","ordinal":1,"status":"completed"},{"id":"run-latest","ordinal":2,"status":"completed"}],
      "turnItems":[{"item":{"type":"proposed_plan","planId":"plan-1"},"updatedAt":"2026-10-17T10:01:00Z"},
        {"item":{"type":"proposed_plan","planId":"plan-2"},"updatedAt":"2026-10-17T10:02:00Z"}],
      "plans":[{"id":"plan-1","threadId":"thread-1","runId":"run-older","nodeId":"node-1","status":"active","kind":"proposed_plan","markdown":"# Old\nOld"},
        {"id":"plan-2","threadId":"thread-1","runId":"run-latest","nodeId":"node-2","status":"active","kind":"proposed_plan","markdown":"# Native Plan\n\n## Summary\nOne\n\n## Steps\nTwo"},
        {"id":"list-1","threadId":"thread-1","runId":"run-latest","nodeId":"node-3","status":"active","kind":"todo_list","steps":[]}]})
}
fn project(
    p: &serde_json::Value,
    settled: bool,
    mode: &str,
    pending: usize,
    attachments: bool,
) -> serde_json::Value {
    plan_workflow::project(
        p,
        "thread-1",
        settled,
        mode,
        pending,
        attachments,
        omarchy_t3_client::rendered_markdown,
    )
    .unwrap()
}
#[test]
fn native_mode_gate_and_client_preference() {
    let provider = json!({"instanceId":"inst","showInteractionModeToggle":true});
    assert_eq!(
        plan_workflow::mode(true, Some(&provider), "plan"),
        json!({"enabled":true,"interactionMode":"plan"})
    );
    assert_eq!(
        plan_workflow::mode(false, Some(&provider), "plan")["interactionMode"],
        "default"
    );
    assert_eq!(plan_workflow::mode(true, None, "plan")["enabled"], false);
    assert_eq!(
        plan_workflow::mode(
            true,
            Some(&json!({"showInteractionModeToggle":false})),
            "plan"
        )["interactionMode"],
        "default"
    );
    assert_eq!(
        plan_workflow::mode(true, Some(&json!({})), "plan")["enabled"],
        true
    );
}
#[test]
fn completed_native_artifact_only_scoped_ready_after_settlement() {
    let p = projection();
    let view = project(&p, true, "plan", 0, false);
    assert_eq!(view["plans"].as_array().unwrap().len(), 2);
    assert_eq!(view["ready"]["id"], "plan-2");
    assert_eq!(view["ready"]["title"], "Native Plan");
    assert_eq!(view["ready"]["displayMarkdown"], "One\n\n## Steps\nTwo");
    assert!(view["ready"]["rendered"]
        .as_str()
        .unwrap()
        .contains("<h2>Steps</h2>"));
    for (settled, mode, pending, attachments) in [
        (false, "plan", 0, false),
        (true, "default", 0, false),
        (true, "plan", 1, false),
        (true, "plan", 0, true),
    ] {
        assert!(project(&p, settled, mode, pending, attachments)["ready"].is_null());
    }
    let mut other = p.clone();
    other["plans"][1]["status"] = json!("superseded");
    assert!(project(&other, true, "plan", 0, false)["ready"].is_null());
    let mut wrong = p.clone();
    wrong["plans"][1]["threadId"] = json!("thread-other");
    assert_eq!(
        plan_workflow::project(&wrong, "thread-1", true, "plan", 0, false, |s| s.into())
            .unwrap_err(),
        "invalid_plan_scope"
    );
    assert_eq!(
        plan_workflow::project(&p, "thread-other", true, "plan", 0, false, |s| s.into())
            .unwrap_err(),
        "invalid_plan_scope"
    );
}
#[test]
fn native_follow_up_contract_feedback_and_implementation() {
    let plan = project(&projection(), true, "plan", 0, false)["ready"].clone();
    assert_eq!(
        plan_workflow::follow_up(&plan, "thread-1", "  Change step 2  ").unwrap(),
        json!({"text":"Change step 2","interactionMode":"plan"})
    );
    let implementation = plan_workflow::follow_up(&plan, "thread-1", " \n").unwrap();
    assert_eq!(
        implementation,
        json!({"text":"PLEASE IMPLEMENT THIS PLAN:\n# Native Plan\n\n## Summary\nOne\n\n## Steps\nTwo",
        "interactionMode":"default","sourcePlanRef":{"threadId":"thread-1","planId":"plan-2"}})
    );
    let mut command = json!({"type":"message.dispatch","threadId":"thread-new","messageId":"m","text":implementation["text"],"attachments":[],"dispatchMode":{"type":"start_immediately"}});
    plan_workflow::attach_source_ref(&mut command, &implementation).unwrap();
    assert_eq!(
        command["sourcePlanRef"],
        json!({"threadId":"thread-1","planId":"plan-2"})
    );
    assert_eq!(command["dispatchMode"], json!({"type":"start_immediately"}));
    let mut launch = json!({"type":"thread.launch","threadId":"thread-new"});
    assert_eq!(
        plan_workflow::attach_source_ref(&mut launch, &implementation),
        Err("invalid_plan_command")
    );
    assert!(launch.get("sourcePlanRef").is_none());
    assert_eq!(
        plan_workflow::follow_up(&plan, "thread-other", "").unwrap_err(),
        "invalid_plan_scope"
    );
    let mut superseded = plan.clone();
    superseded["status"] = json!("superseded");
    assert_eq!(
        plan_workflow::follow_up(&superseded, "thread-1", "").unwrap_err(),
        "invalid_plan_scope"
    );
}
#[test]
fn bounded_untrusted_markdown_and_safe_render() {
    let mut p = projection();
    p["plans"][1]["markdown"]=json!("# <script>evil</script>\n![remote](https://example.test/x)\n[javascript](javascript:alert(1))");
    let view = project(&p, true, "plan", 0, false);
    let html = view["ready"]["rendered"].as_str().unwrap();
    assert!(!html.contains("<script>"));
    assert!(!html.contains("<img"));
    assert!(!html.contains("href=\"javascript:"));
    p["plans"][1]["markdown"] = json!("z".repeat(120_001));
    assert_eq!(
        plan_workflow::project(&p, "thread-1", true, "plan", 0, false, |s| s.into()).unwrap_err(),
        "plan_limit"
    );
    assert_eq!(
        plan_workflow::implementation_title("# Readable Plan\nSteps"),
        "Implement Readable Plan"
    );
}

#[test]
fn normalized_plan_cards_and_ready_state_follow_native_run_and_question_state() {
    let mut p = projection();
    assert!(plan_workflow::latest_run_settled(&p));
    let v = plan_workflow::view(&p, omarchy_t3_client::rendered_markdown);
    assert_eq!(v["ready"]["id"], "plan-2");
    p["runs"][1]["status"] = json!("running");
    assert!(!plan_workflow::latest_run_settled(&p));
    assert!(plan_workflow::view(&p, omarchy_t3_client::rendered_markdown)["ready"].is_null());
    p["runs"][1]["status"] = json!("waiting");
    assert!(!plan_workflow::latest_run_settled(&p));
    p["runs"][1]["status"] = json!("completed");
    p["messages"] = json!([]);
    let normalized = omarchy_t3_client::normalize_messages(&json!({"projection":p})).unwrap();
    assert_eq!(normalized["planView"]["ready"]["id"], "plan-2");
    assert!(normalized["timeline"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["role"] == "plan" && row["plan"]["id"] == "plan-2"));
}

#[test]
fn held_queue_and_out_of_order_runs_use_native_presented_run() {
    let mut p = projection();
    p["runs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"held","ordinal":3,"status":"queued","queueHeld":true}));
    p["runs"].as_array_mut().unwrap().reverse();
    assert!(plan_workflow::latest_run_settled(&p));
    assert_eq!(
        plan_workflow::view(&p, |s| s.into())["ready"]["id"],
        "plan-2"
    );
    p["runs"][0]["queueHeld"] = json!(false);
    assert!(!plan_workflow::latest_run_settled(&p));
    p["runs"][1]["status"] = json!("failed");
    p["runs"][1]["rootNodeId"] = json!("root");
    p["runs"][1]["completedAt"] = json!("2026-10-17T10:02:00Z");
    p["runs"][2]["completedAt"] = json!("2026-10-17T10:01:00Z");
    p["turnItems"].as_array_mut().unwrap().push(json!({"item":{"type":"error","status":"failed","runId":"run-latest","nodeId":"root","failure":{"class":"usage_limit","message":"limit"}}}));
    assert!(plan_workflow::latest_run_settled(&p));
    assert_eq!(
        plan_workflow::view(&p, |s| s.into())["ready"]["id"],
        "plan-2"
    );
    p["thread"]["providerInstanceId"] = json!("provider");
    p["providerSessions"] =
        json!([{"providerInstanceId":"provider","lastError":"a different failure"}]);
    assert!(!plan_workflow::latest_run_settled(&p));
}
