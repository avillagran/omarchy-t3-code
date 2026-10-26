//! Native user-message rewind. Pure builders only: dispatch belongs to the shared durable outbox.
//! No filesystem access, RPC, implicit retry or mutation is performed here.
use serde_json::{json, Value};

/// Correlated native read-back, not the dispatch acknowledgement. Absence of a
/// message is not evidence: bounded history, pagination and pruning also remove it.
pub fn observation(input: &Value, p: &Value) -> Result<Value> {
    let cmd = confirmed_command(input)?;
    if p["thread"]["id"] != cmd["threadId"] {
        return Err("revert_scope_mismatch".into());
    }
    let accepted = &p["thread"]["rollbackRequestId"];
    if accepted.as_str().is_some()
        && *accepted != cmd["commandId"]
        && *accepted != input["reference"]["priorRollbackRequestId"]
    {
        return Ok(json!({"state":"superseded"}));
    }
    let failure = &p["thread"]["rollbackFailure"];
    if *accepted == cmd["commandId"] && failure["requestId"] == cmd["commandId"] {
        return Ok(json!({"state":"failed","error":failure["message"]}));
    }
    let r = &input["reference"];
    let rolled_back = rows(p, "runs").iter().any(|run| {
        run["id"] == r["messageRunId"]
            && run["status"] == "rolled_back"
            && run["ordinal"]
                .as_u64()
                .zip(r["turnCount"].as_u64())
                .is_some_and(|(a, b)| a > b)
    });
    Ok(
        json!({"state":if *accepted == cmd["commandId"] && rolled_back {"complete"} else {"pending"}}),
    )
}

/// Capture before dispatch; after rollback the source may disappear from bounded
/// history. Parent owns attachment preflight and durable, once-only draft recovery.
pub fn recovery_message(p: &Value, r: &Value) -> Result<Value> {
    if reference(p, id(r, "messageId")?)? != *r {
        return Err("revert_reference_changed".into());
    }
    let m = rows(p, "messages")
        .iter()
        .find(|m| m["id"] == r["messageId"])
        .ok_or("revert_message_unavailable")?;
    if !m["text"].is_string() || !m["attachments"].is_array() {
        return Err("revert_message_invalid".into());
    }
    Ok(json!({"id":m["id"],"runId":m["runId"],"text":m["text"],
        "attachments":m["attachments"],"context":m["context"]}))
}

/// Supply the native recallable prompt as `restored`; do not pass Qt-rendered HTML.
pub fn append_prompt(current: &str, restored: &str) -> String {
    if restored.is_empty() {
        current.to_owned()
    } else if current.is_empty() {
        restored.to_owned()
    } else {
        format!("{current}\n\n{restored}")
    }
}

type Result<T> = std::result::Result<T, String>;

/// First-admission envelope for the EXISTING shared outbox. `environment` must
/// be the selected authenticated Environment.id, not another input field.
pub fn outbox_envelope(
    environment: &str,
    input: &Value,
    p: &Value,
    safety: &Safety,
) -> Result<Value> {
    if input["environment"] != environment {
        return Err("revert_scope_mismatch".into());
    }
    Ok(json!({"environment":environment,"command":command(input,p,safety)?}))
}

/// Populate from the authenticated environment and current composer/run state.
/// Unknown capability/permission/connectivity is false, never guessed from a driver name.
#[derive(Clone, Copy, Debug, Default)]
pub struct Safety {
    pub can_operate: bool,
    pub connected: bool,
    pub supports_conversation_rollback: bool,
    pub working: bool,
    pub send_busy: bool,
    pub connecting: bool,
    pub pending_attachments: bool,
    pub rewinding: bool,
}

/// `Safety`'s members as JSON, camelCase.
const SAFETY_KEYS: [&str; 8] = ["canOperate", "connected", "supportsConversationRollback", "working", "sendBusy", "connecting", "pendingAttachments", "rewinding"];

impl Safety {
    /// From `{canOperate, connected, supportsConversationRollback, working,
    /// sendBusy, connecting, pendingAttachments, rewinding}`; anything but `true`
    /// is false, so an unknown capability never enables a rewind.
    pub fn from_json(v: &Value) -> Safety {
        let on = |key: &str| v[key] == true;
        Safety {
            can_operate: on(SAFETY_KEYS[0]),
            connected: on(SAFETY_KEYS[1]),
            supports_conversation_rollback: on(SAFETY_KEYS[2]),
            working: on(SAFETY_KEYS[3]),
            send_busy: on(SAFETY_KEYS[4]),
            connecting: on(SAFETY_KEYS[5]),
            pending_attachments: on(SAFETY_KEYS[6]),
            rewinding: on(SAFETY_KEYS[7]),
        }
    }
    pub fn to_json(&self) -> Value {
        let values = [self.can_operate, self.connected, self.supports_conversation_rollback, self.working, self.send_busy, self.connecting, self.pending_attachments, self.rewinding];
        Value::Object(SAFETY_KEYS.iter().zip(values).map(|(k, v)| (k.to_string(), json!(v))).collect())
    }
    pub fn check(&self) -> Result<()> {
        if !self.can_operate {
            return Err("revert_operate_required".into());
        }
        if !self.connected {
            return Err("revert_disconnected".into());
        }
        if !self.supports_conversation_rollback {
            return Err("revert_unsupported".into());
        }
        if self.working || self.send_busy || self.connecting {
            return Err("revert_interrupt_first".into());
        }
        if self.pending_attachments {
            return Err("revert_attachments_pending".into());
        }
        if self.rewinding {
            return Err("revert_already_pending".into());
        }
        Ok(())
    }
}

/// Stable command builder. For FIRST admission use `command` with fresh projection.
/// This function alone is only suitable for comparing an ALREADY persisted identical
/// outbox command on explicit retry; it cannot prove current availability or authority.
pub fn confirmed_command(input: &Value) -> Result<Value> {
    if input["confirmed"] != true {
        return Err("revert_confirmation_required".into());
    }
    let restore = input["restoreFiles"]
        .as_bool()
        .ok_or("revert_file_choice_required")?;
    let request_id = id(input, "requestId")?;
    uuid::Uuid::parse_str(request_id).map_err(|_| "invalid_request_id")?;
    id(input, "environment")?;
    let thread = id(input, "thread")?;
    let r = &input["reference"];
    for key in [
        "threadId",
        "messageId",
        "messageRunId",
        "messageCheckpointId",
        "checkpointId",
        "scopeId",
        "providerThreadId",
        "providerInstanceId",
    ] {
        id(r, key)?;
    }
    nullable_id(r, "checkpointRunId")?;
    nullable_id(r, "priorRollbackRequestId")?;
    if r["turnCount"].as_u64().is_none() || r["threadId"] != thread {
        return Err("revert_scope_mismatch".into());
    }
    Ok(
        json!({"type":"checkpoint.rollback","commandId":format!("omarchy-{request_id}"),
        "threadId":thread,"scopeId":r["scopeId"],"checkpointId":r["checkpointId"],"restoreFiles":restore}),
    )
}

/// First admission: explicit confirmation, live safety and exact reference freshness.
/// Persist this returned value with the environment BEFORE shared RPC dispatch.
pub fn command(input: &Value, p: &Value, safety: &Safety) -> Result<Value> {
    safety.check()?;
    let command = confirmed_command(input)?;
    let fresh = reference(p, id(&input["reference"], "messageId")?)?;
    if fresh != input["reference"] {
        return Err("revert_reference_changed".into());
    }
    if input["restoreFiles"] == true
        && p["thread"]["worktreePath"]
            .as_str()
            .is_none_or(str::is_empty)
    {
        return Err("revert_files_require_worktree".into());
    }
    // A non-null worktree is NOT an isolation proof. Native server checks realpaths,
    // overlapping archived/live threads and provider sessions at admission/execution.
    Ok(command)
}

fn rows<'a>(p: &'a Value, key: &str) -> &'a [Value] {
    p[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn id<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .filter(|s| {
            !s.is_empty() && s.trim() == *s && s.len() <= 4096 && !s.chars().any(char::is_control)
        })
        .ok_or_else(|| format!("invalid_revert_{key}"))
}
fn nullable_id(v: &Value, key: &str) -> Result<()> {
    if v.get(key) == Some(&Value::Null) {
        Ok(())
    } else {
        id(v, key).map(|_| ())
    }
}

/// ConversationMessage deliberately has no inputIntent. Native timeline obtains
/// it from the projected user_message turn item (session-logic.ts:651-683).
pub fn message_item<'a>(p: &'a Value, message: &Value) -> Option<&'a Value> {
    rows(p, "turnItems")
        .iter()
        .rev()
        .map(|row| row.get("item").unwrap_or(row))
        .find(|item| item["type"] == "user_message" && item["messageId"] == message["id"])
}
/// A bounded snapshot may omit the turn item/initial checkpoint scope. The
/// button can request full native context; only reference() admits a known intent.
pub fn can_preview(p: &Value, message: &Value) -> bool {
    message["role"] == "user"
        && message["runId"].is_string()
        && rows(p, "checkpoints").iter().any(|c| {
            c["runId"] == message["runId"]
                && c["status"] == "ready"
                && c["appRunOrdinal"].as_u64().is_some_and(|n| n > 0)
        })
        && message_item(p, message).is_none_or(|item| {
            item["runId"] == message["runId"]
                && matches!(
                    item["inputIntent"].as_str(),
                    Some("turn_start" | "queued_turn")
                )
        })
}
/// The checkpoint turn count just before `run`: the ready checkpoint of the previous
/// visible run (rolled-back and inherited history excluded), or 0 at the thread start.
fn previous_visible_turn_count(p: &Value, run: &str) -> Option<u64> {
    let mut seen: Vec<&str> = vec![];
    for row in rows(p, "turnItems") {
        let item = row.get("item").unwrap_or(row);
        if item["visibility"] == "inherited" { continue }
        let Some(id) = item["runId"].as_str() else { continue };
        if rows(p, "runs").iter().any(|r| r["id"] == id && r["status"] == "rolled_back") { continue }
        if !seen.contains(&id) { seen.push(id) }
    }
    let at = seen.iter().position(|r| *r == run)?;
    if at == 0 {
        let start = rows(p, "checkpoints").iter().any(|c| c["ordinalWithinScope"] == 0 && c["appRunOrdinal"].is_null() && c["status"] == "ready");
        return start.then_some(0);
    }
    let previous = seen[at - 1];
    rows(p, "checkpoints").iter().rev()
        .find(|c| c["runId"] == previous && c["status"] == "ready" && c["appRunOrdinal"].as_u64().is_some_and(|n| n > 0))
        .and_then(|c| c["appRunOrdinal"].as_u64())
}
/// Resolve BEFORE the clicked user message, mirroring session-logic.ts and commands.ts.
/// Call on the raw native projection before compacting; never count visible messages.
/// Missing bounded-history entities fail closed (fetch native checkpoint context first).
pub fn reference(p: &Value, message_id: &str) -> Result<Value> {
    let thread = id(&p["thread"], "id")?;
    let message = rows(p, "messages")
        .iter()
        .find(|m| m["id"] == message_id)
        .ok_or("revert_message_unavailable")?;
    let item = message_item(p, message).ok_or("revert_message_not_turn_start")?;
    if message["role"] != "user"
        || item["runId"] != message["runId"]
        || !matches!(
            item["inputIntent"].as_str(),
            Some("turn_start" | "queued_turn")
        )
    {
        return Err("revert_message_not_turn_start".into());
    }
    let run = id(message, "runId")?;
    let message_checkpoint = rows(p, "checkpoints")
        .iter()
        .rev()
        .find(|c| {
            c["runId"] == run
                && c["status"] == "ready"
                && c["appRunOrdinal"].as_u64().is_some_and(|n| n > 0)
        })
        .ok_or("revert_checkpoint_unavailable")?;
    let ordinal = message_checkpoint["appRunOrdinal"].as_u64().unwrap();
    // T3 deriveRevertTurnCountByUserMessageId (#18982): run ordinals keep growing after a
    // rewind, so the target is the previous *visible* run's ready checkpoint, not ordinal-1.
    let turn_count = previous_visible_turn_count(p, run).ok_or("revert_checkpoint_unavailable")?;
    // Match native findLast first, THEN readiness. Never silently skip a stale target.
    let target = rows(p, "checkpoints")
        .iter()
        .rev()
        .find(|c| {
            if turn_count == 0 {
                c["ordinalWithinScope"] == 0 && c["appRunOrdinal"].is_null()
            } else {
                c["appRunOrdinal"].as_u64() == Some(turn_count)
            }
        })
        .filter(|c| c["status"] == "ready")
        .ok_or("revert_checkpoint_unavailable")?;
    let checkpoint = id(target, "id")?;
    let scope_id = id(target, "scopeId")?;
    let scope = rows(p, "checkpointScopes")
        .iter()
        .find(|s| s["id"] == scope_id)
        .ok_or("revert_scope_unavailable")?;
    if target["threadId"] != thread
        || message_checkpoint["threadId"] != thread
        || scope["threadId"] != thread
    {
        return Err("revert_scope_mismatch".into());
    }
    let provider_id = id(&p["thread"], "activeProviderThreadId")?;
    let provider = rows(p, "providerThreads")
        .iter()
        .find(|r| r["id"] == provider_id)
        .ok_or("revert_provider_unavailable")?;
    id(provider, "providerSessionId")?;
    if provider["providerInstanceId"] != p["thread"]["modelSelection"]["instanceId"] {
        return Err("revert_provider_changed".into());
    }
    let message_run = rows(p, "runs")
        .iter()
        .find(|r| r["id"] == run && r["ordinal"].as_u64() == Some(ordinal))
        .ok_or("revert_run_unavailable")?;
    if message_run["status"] == "rolled_back" {
        return Err("revert_message_unavailable".into());
    }
    nullable_id(target, "runId")?;
    Ok(
        json!({"threadId":thread,"messageId":message_id,"messageRunId":run,
        "messageCheckpointId":id(message_checkpoint,"id")?,"checkpointId":checkpoint,
        "checkpointRunId":target["runId"],"scopeId":scope_id,"providerThreadId":provider_id,
        "providerInstanceId":id(provider,"providerInstanceId")?,"turnCount":turn_count,
        "priorRollbackRequestId":p["thread"]["rollbackRequestId"]}),
    )
}
