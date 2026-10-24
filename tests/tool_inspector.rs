use omarchy_t3_client::{tool_inspector, Environment};
use serde_json::{json, Value};
use tool_inspector as inspector;

// Exact OrchestrationV2TurnItemBaseFields + command_execution union member.
fn item(kind: &str) -> Value {
    json!({"id":"item:source","threadId":"thread:ancestor","runId":"run:source",
        "nodeId":null,"providerThreadId":"provider-thread:source","providerTurnId":"turn:source",
        "nativeItemRef":null,"parentItemId":null,"ordinal":3,"status":"completed","title":null,
        "startedAt":"2026-10-17T12:00:00.000Z","completedAt":"2026-10-17T12:00:01.000Z",
        "updatedAt":"2026-10-17T12:00:01.000Z","type":kind,"input":"printf fixture","output":"fixture\n","exitCode":0})
}
fn reference(item: &Value) -> Value {
    let mut r = inspector::compact_reference(&json!({"position":3,"visibility":"inherited",
        "sourceThreadId":"thread:ancestor","sourceItemId":"item:source","item":item}))
    .unwrap();
    r["environment"] = json!("fixture");
    r["threadId"] = json!("thread:viewer");
    r
}
fn parsed(item: &Value) -> Value {
    inspector::parse(&reference(item), &json!({"item":item})).unwrap()
}
fn body<'a>(view: &'a Value, key: &str) -> &'a str {
    view["sections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == key)
        .unwrap()["text"]
        .as_str()
        .unwrap()
}
#[test]
fn native_request_uses_source_not_viewer_and_only_native_keys() {
    let i = item("command_execution");
    let r = reference(&i);
    let (method, payload) = inspector::request(&r).unwrap();
    assert_eq!(method, "orchestration.getTurnItem");
    assert_eq!(
        payload,
        json!({"threadId":"thread:ancestor","itemId":"item:source","revision":"2026-10-17T12:00:01.000Z"})
    );
    assert_eq!(inspector::scope(&r).unwrap()["threadId"], "thread:viewer");
    let wire = inspector::compact_reference(&json!({"position":3,"visibility":"inherited","sourceThreadId":"thread:ancestor","sourceItemId":"item:source","item":i})).unwrap();
    assert_eq!(wire.as_object().unwrap().len(), 6);
    assert_eq!(wire["runId"], "run:source");
    assert_eq!(wire["providerTurnId"], "turn:source");
    assert!(!wire.to_string().contains("printf"));
    assert!(!wire.to_string().contains("fixture"));
}
#[test]
fn compact_revisions_match_native_live_and_completed_rules() {
    let mut i = item("command_execution");
    for status in ["idle", "pending", "running", "waiting"] {
        i["status"] = json!(status);
        assert_eq!(
            inspector::compact_reference(&i).unwrap()["revision"],
            "live"
        );
    }
    i["status"] = json!("failed");
    assert_eq!(
        inspector::compact_reference(&i).unwrap()["revision"],
        i["updatedAt"]
    );
    // A projected wrapper cannot silently fall back to the viewer/item ID.
    assert!(inspector::compact_reference(&json!({"item":i})).is_err());
}
#[test]
fn response_envelope_null_and_mismatches_fail_closed() {
    let i = item("command_execution");
    let r = reference(&i);
    assert_eq!(
        inspector::parse(&r, &json!({"item":null})).unwrap()["available"],
        false
    );
    for malformed in [json!({"output":"wrong envelope"}), json!({"item":[]})] {
        assert!(inspector::parse(&r, &malformed).is_err());
    }
    for key in ["id", "threadId", "type", "runId", "providerTurnId"] {
        let mut wrong = i.clone();
        wrong[key] = json!("other");
        assert_eq!(
            inspector::parse(&r, &json!({"item":wrong})).unwrap_err(),
            "tool_scope_mismatch"
        );
    }
    let mut malformed = i.clone();
    malformed["input"] = json!({"command":"not native command schema"});
    assert_eq!(
        inspector::parse(&r, &json!({"item":malformed})).unwrap_err(),
        "malformed_tool_item"
    );
    for key in ["sourceItemId", "sourceThreadId", "revision"] {
        let mut wrong = r.clone();
        wrong[key] = json!("bad\nidentifier");
        assert!(inspector::request(&wrong).is_err());
    }
    let mut wrong = r.clone();
    wrong.as_object_mut().unwrap().remove("providerTurnId");
    assert!(inspector::request(&wrong).is_err());
}
#[test]
fn command_input_exit_and_legacy_stdout_are_not_guessed_fields() {
    let mut i = item("command_execution");
    i["output"] = json!(r#"{"stdout":"hello","stderr":"warning","interrupted":false}"#);
    i["exitCode"] = json!(2);
    i["status"] = json!("failed");
    let v = parsed(&i);
    assert_eq!(body(&v, "input"), "printf fixture");
    assert_eq!(body(&v, "output"), "hello\nwarning");
    assert_eq!(v["exitCode"], 2);
    assert_eq!(v["failed"], true);
    let raw: Value = serde_json::from_str(v["raw"].as_str().unwrap()).unwrap();
    assert_eq!(raw, i);
    let meta: Value = serde_json::from_str(v["metadata"].as_str().unwrap()).unwrap();
    assert_eq!(meta["providerTurnId"], "turn:source");
    i["output"] = json!("");
    assert_eq!(parsed(&i)["emptyOutput"], true);
    i["outputOmitted"] = json!(true);
    let withheld = parsed(&i);
    assert_eq!(withheld["emptyOutput"], false);
    assert_eq!(withheld["outputOmitted"], true);
}
#[test]
fn dynamic_mcp_blocks_structured_fallback_and_reasoning() {
    let mut i = item("dynamic_tool");
    i.as_object_mut().unwrap().remove("exitCode");
    i["toolName"] = json!("read_file");
    i["input"] = json!({"path":"README.md","clear":null,"empty":""});
    i["output"] = json!({"content":[{"type":"text","text":"first"},{"type":"resource","resource":{"uri":"fixture://resource","text":"second"}}],"isError":false});
    let v = parsed(&i);
    assert_eq!(v["title"], "read_file");
    assert_eq!(body(&v, "output"), "first\nsecond");
    assert!(body(&v, "input").contains("\"clear\": null"));
    i["output"] = json!({"data":{"value":42}});
    assert!(body(&parsed(&i), "output").contains("42"));
    i["output"] = json!({"content":[],"structuredContent":{"value":7}});
    assert!(body(&parsed(&i), "output").contains("7"));
    let mut reasoning = item("reasoning");
    reasoning.as_object_mut().unwrap().remove("input");
    reasoning.as_object_mut().unwrap().remove("output");
    reasoning.as_object_mut().unwrap().remove("exitCode");
    reasoning["text"] = json!("Actual reasoning from native item");
    reasoning["streaming"] = json!(false);
    assert_eq!(
        body(&parsed(&reasoning), "reasoning"),
        "Actual reasoning from native item"
    );
}
#[test]
fn bounded_unicode_display_raw_and_metadata_are_explicit() {
    let mut i = item("dynamic_tool");
    i["toolName"] = json!("fixture");
    i["input"] = json!({"nested":"á🙂".repeat(30000)});
    i["output"] = json!("🙂".repeat(100000));
    i["toolSource"] = json!({"name":"界".repeat(40000)});
    let v = parsed(&i);
    for section in v["sections"].as_array().unwrap() {
        assert!(section["text"].as_str().unwrap().len() <= inspector::SECTION_BYTES);
        assert_eq!(section["truncated"], true);
    }
    assert!(v["raw"].as_str().unwrap().len() <= inspector::RAW_BYTES);
    assert_eq!(v["rawTruncated"], true);
    assert!(v["metadata"].as_str().unwrap().len() <= inspector::SECTION_BYTES);
    assert_eq!(v["metadataTruncated"], true);
    assert!(serde_json::to_vec(&v).unwrap().len() < 1024 * 1024);
}
#[test]
fn read_executes_real_effect_rpc_against_loopback_only() {
    use std::{net::TcpListener, sync::mpsc, time::Duration};
    use tungstenite::{accept, Message};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let i = item("command_execution");
    let response = i.clone();
    let (tx, rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut ws = accept(stream).unwrap();
        let msg = ws.read().unwrap();
        let request: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        tx.send(request.clone()).unwrap();
        ws.send(Message::Text(json!({"_tag":"Exit","requestId":request["id"],"exit":{"_tag":"Success","value":{"item":response}}}).to_string().into())).unwrap();
    });
    let e = Environment {
        id: "fixture".into(),
        name: "Fixture".into(),
        base_url: format!("http://{address}"),
        token_file: "/nonexistent-read-token".into(),
        write_token_file: None,
        bearer: Some("fixture-token".into()),
        connection_error: None,
        connection: None,
    };
    let mut r = reference(&i);
    r["environment"] = json!("wrong");
    assert_eq!(
        inspector::execute(&e, &r).unwrap_err(),
        "tool_scope_mismatch"
    );
    r["environment"] = json!("fixture");
    let view = inspector::execute(&e, &r).unwrap();
    assert_eq!(body(&view, "output"), "fixture\n");
    let wire = rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(wire["_tag"], "Request");
    assert_eq!(wire["tag"], inspector::METHOD);
    assert_eq!(wire["payload"], inspector::request(&r).unwrap().1);
    assert_eq!(wire["headers"], json!([]));
    server.join().unwrap();
}
#[test]
fn existing_connection_errors_do_not_fall_back_to_other_credentials() {
    let e = Environment {
        id: "fixture".into(),
        name: String::new(),
        base_url: "http://127.0.0.1:1".into(),
        token_file: "/nonexistent".into(),
        write_token_file: None,
        bearer: Some("fixture-token".into()),
        connection_error: Some("native_connection_unavailable".into()),
        connection: None,
    };
    assert_eq!(
        inspector::execute(&e, &reference(&item("command_execution"))).unwrap_err(),
        "native_connection_unavailable"
    );
}

// This is a pinned local source-contract check, not a test against a live account.
#[test]
fn pinned_native_source_contract_matches_request_and_fixtures_when_available() {
    let root = std::path::Path::new("/home/avillagran/Work/t3code-nightly");
    if !root.exists() {
        return;
    }
    let source =
        std::fs::read_to_string(root.join("packages/contracts/src/orchestrationV2.ts")).unwrap();
    for text in ["getTurnItem: \"orchestration.getTurnItem\"","export const OrchestrationV2GetTurnItemInput = Schema.Struct({\n  threadId: ThreadId,\n  itemId: TurnItemId,","item: Schema.NullOr(OrchestrationV2TurnItem)","sourceThreadId: ThreadId,\n  sourceItemId: TurnItemId,","providerTurnId: Schema.NullOr(ProviderTurnId)","type: Schema.Literal(\"command_execution\"),\n    input: Schema.String,","type: Schema.Literal(\"dynamic_tool\")"] { assert!(source.contains(text),"Missing pinned contract fragment: {text}"); }
    let projection =
        std::fs::read_to_string(root.join("apps/server/src/orchestration-v2/WireProjection.ts"))
            .unwrap();
    assert!(projection.contains("const MAX_ON_DEMAND_BYTES = 256 * 1024"));
    assert!(projection.contains("output: boundToolOutput(item.output)"));
}
