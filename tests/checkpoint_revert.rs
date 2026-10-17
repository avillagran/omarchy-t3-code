#[path = "../src/checkpoint_revert.rs"]
mod checkpoint_revert;
use serde_json::{json, Value};

fn projection() -> Value {
    json!({
        "thread":{"id":"thread:one","activeProviderThreadId":"provider:one","modelSelection":{"instanceId":"codex"},"worktreePath":"/fixture/worktree"},
        "providerThreads":[{"id":"provider:one","providerInstanceId":"codex","providerSessionId":"session:one"}],
        "checkpointScopes":[{"id":"scope:one","threadId":"thread:one","cwd":"/fixture/worktree","providerThreadId":"provider:one"}],
        "checkpoints":[
            {"id":"checkpoint:initial","threadId":"thread:one","scopeId":"scope:one","runId":null,"ordinalWithinScope":0,"appRunOrdinal":null,"status":"ready"},
            {"id":"checkpoint:1","threadId":"thread:one","scopeId":"scope:one","runId":"run:one","ordinalWithinScope":1,"appRunOrdinal":1,"status":"ready"},
            {"id":"checkpoint:2","threadId":"thread:one","scopeId":"scope:one","runId":"run:two","ordinalWithinScope":2,"appRunOrdinal":2,"status":"ready"}],
        "runs":[{"id":"run:one","ordinal":1,"status":"completed"},{"id":"run:two","ordinal":2,"status":"completed"}],
        "turnItems":[{"item":{"type":"user_message","messageId":"message:one","runId":"run:one","inputIntent":"turn_start"}},{"item":{"type":"user_message","messageId":"message:two","runId":"run:two","inputIntent":"queued_turn"}}],
        "messages":[
            {"id":"message:one","role":"user","runId":"run:one","text":"first","attachments":[],"context":[]},
            {"id":"message:two","role":"user","runId":"run:two","text":"<b>second</b>","attachments":[],"context":[]}]
    })
}

#[test]
fn native_user_action_targets_before_message_not_its_completed_checkpoint() {
    let p = projection();
    let first = checkpoint_revert::reference(&p, "message:one").unwrap();
    assert_eq!(first["checkpointId"], "checkpoint:initial");
    assert_eq!(first["turnCount"], 0);
    let second = checkpoint_revert::reference(&p, "message:two").unwrap();
    assert_eq!(second["checkpointId"], "checkpoint:1");
    assert_eq!(second["messageRunId"], "run:two");
    assert_eq!(second["checkpointRunId"], "run:one");
    assert_eq!(second["providerThreadId"], "provider:one");
    assert_eq!(second["scopeId"], "scope:one");
    assert_eq!(second["turnCount"], 1);
}

fn request(p: &Value) -> Value {
    json!({"requestId":"b3e86a43-744c-46ec-b5f8-65ae79316907","environment":"local",
        "thread":"thread:one","confirmed":true,"restoreFiles":false,
        "reference":checkpoint_revert::reference(p,"message:two").unwrap()})
}

#[test]
fn confirmed_builder_has_exact_native_payload_and_fail_closed_guards() {
    let p = projection();
    let input = request(&p);
    let safety = checkpoint_revert::Safety {
        can_operate: true,
        connected: true,
        supports_conversation_rollback: true,
        ..Default::default()
    };
    let command = checkpoint_revert::command(&input, &p, &safety).unwrap();
    assert_eq!(
        command,
        json!({"type":"checkpoint.rollback","commandId":"omarchy-b3e86a43-744c-46ec-b5f8-65ae79316907",
        "threadId":"thread:one","scopeId":"scope:one","checkpointId":"checkpoint:1","restoreFiles":false})
    );
    for field in ["confirmed", "restoreFiles"] {
        let mut invalid = input.clone();
        invalid.as_object_mut().unwrap().remove(field);
        assert!(checkpoint_revert::command(&invalid, &p, &safety).is_err());
    }
    let mut invalid = input.clone();
    invalid["confirmed"] = json!(false);
    assert!(checkpoint_revert::command(&invalid, &p, &safety).is_err());
    invalid = input.clone();
    invalid["reference"]["checkpointId"] = json!("different");
    assert_eq!(
        checkpoint_revert::command(&invalid, &p, &safety).unwrap_err(),
        "revert_reference_changed"
    );
    invalid = input.clone();
    invalid["thread"] = json!("other");
    assert!(checkpoint_revert::command(&invalid, &p, &safety).is_err());
    invalid = input.clone();
    invalid["requestId"] = json!("not-a-uuid");
    assert!(checkpoint_revert::command(&invalid, &p, &safety).is_err());
    for guarded in [
        checkpoint_revert::Safety {
            working: true,
            ..safety
        },
        checkpoint_revert::Safety {
            send_busy: true,
            ..safety
        },
        checkpoint_revert::Safety {
            connecting: true,
            ..safety
        },
        checkpoint_revert::Safety {
            pending_attachments: true,
            ..safety
        },
        checkpoint_revert::Safety {
            rewinding: true,
            ..safety
        },
        checkpoint_revert::Safety {
            connected: false,
            ..safety
        },
        checkpoint_revert::Safety {
            can_operate: false,
            ..safety
        },
        checkpoint_revert::Safety {
            supports_conversation_rollback: false,
            ..safety
        },
    ] {
        assert!(checkpoint_revert::command(&input, &p, &guarded).is_err());
    }
    let mut shared = p.clone();
    shared["thread"]["worktreePath"] = Value::Null;
    assert!(checkpoint_revert::command(&input, &shared, &safety).is_ok());
    let mut files = input.clone();
    files["restoreFiles"] = json!(true);
    assert_eq!(
        checkpoint_revert::command(&files, &shared, &safety).unwrap_err(),
        "revert_files_require_worktree"
    );
    assert_eq!(
        checkpoint_revert::command(&files, &p, &safety).unwrap()["restoreFiles"],
        true
    );
}

#[test]
fn completion_requires_correlated_native_state_not_ack_or_message_disappearance() {
    let mut p = projection();
    let input = request(&p);
    assert_eq!(
        checkpoint_revert::observation(&input, &p).unwrap()["state"],
        "pending"
    );
    p["messages"] = json!([]);
    assert_eq!(
        checkpoint_revert::observation(&input, &p).unwrap()["state"],
        "pending"
    );
    p["runs"][1]["status"] = json!("rolled_back");
    assert_eq!(
        checkpoint_revert::observation(&input, &p).unwrap()["state"],
        "pending"
    );
    let cid = "omarchy-b3e86a43-744c-46ec-b5f8-65ae79316907";
    p["thread"]["rollbackRequestId"] = json!(cid);
    assert_eq!(
        checkpoint_revert::observation(&input, &p).unwrap()["state"],
        "complete"
    );
    p["thread"]["rollbackFailure"] = json!({"requestId":"old","message":"old failure"});
    assert_eq!(
        checkpoint_revert::observation(&input, &p).unwrap()["state"],
        "complete"
    );
    p["thread"]["rollbackFailure"] = json!({"requestId":cid,"message":"provider refused"});
    let failed = checkpoint_revert::observation(&input, &p).unwrap();
    assert_eq!(failed["state"], "failed");
    assert_eq!(failed["error"], "provider refused");
    p["thread"]["rollbackRequestId"] = json!("new-operation");
    assert_eq!(
        checkpoint_revert::observation(&input, &p).unwrap()["state"],
        "superseded"
    );
    p["thread"]["id"] = json!("other-thread");
    assert_eq!(
        checkpoint_revert::observation(&input, &p).unwrap_err(),
        "revert_scope_mismatch"
    );
}

#[test]
fn recovery_retains_raw_message_context_and_appends_without_overwriting_draft() {
    let mut p = projection();
    p["messages"][1]["attachments"] = json!([{"id":"asset:one","type":"image","name":"image.png"}]);
    p["messages"][1]["context"] = json!([{"kind":"file","path":"src/main.rs"}]);
    let r = checkpoint_revert::reference(&p, "message:two").unwrap();
    let saved = checkpoint_revert::recovery_message(&p, &r).unwrap();
    assert_eq!(saved["text"], "<b>second</b>");
    assert_eq!(saved["attachments"], p["messages"][1]["attachments"]);
    assert_eq!(saved["context"], p["messages"][1]["context"]);
    assert_eq!(
        checkpoint_revert::append_prompt("unsent", "restored"),
        "unsent\n\nrestored"
    );
    assert_eq!(checkpoint_revert::append_prompt("unsent", ""), "unsent");
    assert_eq!(checkpoint_revert::append_prompt("", "restored"), "restored");
    assert_eq!(
        checkpoint_revert::append_prompt("  unsent  ", " restored "),
        "  unsent  \n\n restored "
    );
    let mut changed = r.clone();
    changed["messageRunId"] = json!("another");
    assert!(checkpoint_revert::recovery_message(&p, &changed).is_err());
}

#[test]
fn previous_rollback_is_baseline_not_a_superseding_operation() {
    let mut p = projection();
    p["thread"]["rollbackRequestId"] = json!("previous-operation");
    let input = request(&p);
    assert_eq!(
        checkpoint_revert::observation(&input, &p).unwrap()["state"],
        "pending"
    );
    p["thread"]["rollbackRequestId"] = json!("later-operation");
    assert_eq!(
        checkpoint_revert::observation(&input, &p).unwrap()["state"],
        "superseded"
    );
}

#[test]
fn hidden_history_steering_stale_or_missing_checkpoints_never_guess_a_target() {
    let p = projection();
    for intent in ["steer", "queued", "follow_up", "unknown"] {
        let mut changed = p.clone();
        changed["turnItems"][1]["item"]["inputIntent"] = json!(intent);
        assert!(checkpoint_revert::reference(&changed, "message:two").is_err());
    }
    for status in ["stale", "missing", "error"] {
        let mut changed = p.clone();
        changed["checkpoints"][1]["status"] = json!(status);
        assert!(checkpoint_revert::reference(&changed, "message:two").is_err());
        changed = p.clone();
        changed["checkpoints"][2]["status"] = json!(status);
        assert!(checkpoint_revert::reference(&changed, "message:two").is_err());
    }
    for collection in ["checkpoints", "checkpointScopes", "providerThreads", "runs"] {
        let mut changed = p.clone();
        changed[collection] = json!([]);
        assert!(checkpoint_revert::reference(&changed, "message:two").is_err());
    }
    let mut changed = p.clone();
    changed["messages"][1]["role"] = json!("assistant");
    assert!(checkpoint_revert::reference(&changed, "message:two").is_err());
    changed = p.clone();
    changed["checkpoints"][1]["threadId"] = json!("other");
    assert!(checkpoint_revert::reference(&changed, "message:two").is_err());
    changed = p.clone();
    changed["providerThreads"][0]["providerSessionId"] = Value::Null;
    assert!(checkpoint_revert::reference(&changed, "message:two").is_err());
    changed = p.clone();
    changed["thread"]["modelSelection"]["instanceId"] = json!("another-provider");
    assert!(checkpoint_revert::reference(&changed, "message:two").is_err());
    // Message ordering/visible-history cardinality cannot change the native ordinal.
    changed = p.clone();
    changed["messages"].as_array_mut().unwrap().remove(0);
    assert_eq!(
        checkpoint_revert::reference(&changed, "message:two").unwrap()["turnCount"],
        1
    );
    // Retry builder remains stable even after the target disappears from the projection.
    let input = request(&p);
    assert_eq!(
        checkpoint_revert::confirmed_command(&input).unwrap()["checkpointId"],
        "checkpoint:1"
    );
    changed["checkpoints"] = json!([]);
    let safe = checkpoint_revert::Safety {
        can_operate: true,
        connected: true,
        supports_conversation_rollback: true,
        ..Default::default()
    };
    assert!(checkpoint_revert::command(&input, &changed, &safe).is_err());
}

#[test]
fn native_pinned_source_contracts_have_not_drifted() {
    let native = std::path::Path::new("/home/avillagran/Work/t3code-nightly");
    if !native.is_dir() {
        eprintln!("native checkout absent: source contract check skipped");
        return;
    }
    let schema =
        std::fs::read_to_string(native.join("packages/contracts/src/orchestrationV2.ts")).unwrap();
    let block = schema
        .split("type: Schema.Literal(\"checkpoint.rollback\")")
        .nth(1)
        .unwrap()
        .split("}),")
        .next()
        .unwrap();
    for field in [
        "restoreFiles: Schema.optional(Schema.Boolean)",
        "commandId: CommandId",
        "threadId: ThreadId",
        "scopeId: CheckpointScopeId",
        "checkpointId: CheckpointId",
    ] {
        assert!(block.contains(field), "native command field drift: {field}");
    }
    assert!(schema.contains("dispatchCommand: \"orchestration.dispatchCommand\""));
    let logic = std::fs::read_to_string(native.join("apps/web/src/session-logic.ts")).unwrap();
    assert!(logic.contains("Math.max(0, checkpoint.checkpointTurnCount - 1)"));
    assert!(logic.contains("entry.message.inputIntent !== \"turn_start\" && entry.message.inputIntent !== \"queued_turn\""));
    let commands =
        std::fs::read_to_string(native.join("packages/client-runtime/src/operations/commands.ts"))
            .unwrap();
    assert!(
        commands.contains("candidate.ordinalWithinScope === 0 && candidate.appRunOrdinal === null")
    );
    assert!(commands.contains("candidate.appRunOrdinal === input.turnCount"));
    let view =
        std::fs::read_to_string(native.join("apps/web/src/components/ChatView.tsx")).unwrap();
    for contract in [
        "Edit from here?",
        "Revert files too",
        "Revert and keep changes",
        "pendingRevert.messageId, true",
        "pendingRevert.messageId, false",
    ] {
        assert!(
            view.contains(contract),
            "native confirmation drift: {contract}"
        );
    }
    let service =
        std::fs::read_to_string(native.join("apps/server/src/orchestration-v2/Orchestrator.ts"))
            .unwrap();
    assert!(service.contains("if (command.restoreFiles !== false)"));
    assert!(service.contains("rollbackRequestId: command.commandId"));
}

#[test]
fn outbox_envelope_binds_command_to_authenticated_environment_without_side_effects() {
    let p = projection();
    let input = request(&p);
    let safety = checkpoint_revert::Safety {
        can_operate: true,
        connected: true,
        supports_conversation_rollback: true,
        ..Default::default()
    };
    let entry = checkpoint_revert::outbox_envelope("local", &input, &p, &safety).unwrap();
    assert_eq!(entry["environment"], "local");
    assert_eq!(
        entry["command"],
        checkpoint_revert::confirmed_command(&input).unwrap()
    );
    assert_eq!(
        checkpoint_revert::outbox_envelope("another", &input, &p, &safety).unwrap_err(),
        "revert_scope_mismatch"
    );
}

#[test]
fn native_message_intent_is_joined_from_turn_items_and_not_guessed_from_conversation_rows() {
    let mut p = projection();
    assert!(p["messages"][1].get("inputIntent").is_none());
    assert!(checkpoint_revert::can_preview(&p, &p["messages"][1]));
    assert!(checkpoint_revert::reference(&p, "message:two").is_ok());
    p["messages"][1]["inputIntent"] = json!("turn_start");
    p["turnItems"][1]["item"]["inputIntent"] = json!("steer");
    assert!(!checkpoint_revert::can_preview(&p, &p["messages"][1]));
    assert!(checkpoint_revert::reference(&p, "message:two").is_err());
    p["turnItems"] = json!([]);
    assert!(checkpoint_revert::can_preview(&p, &p["messages"][1]));
    assert!(checkpoint_revert::reference(&p, "message:two").is_err());
}
