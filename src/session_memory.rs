//! OT3's idle runtime policy. Saved provider conversations remain owned by their harness.
use crate::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub fn minutes(settings: &Value, environment: &str, provider: &str) -> Option<u64> {
    if settings["idleReleaseEnabled"] == false {
        return None;
    }
    let key = format!("{environment}/{provider}");
    let value = settings["idleReleaseProviderMinutes"]
        .get(&key)
        .or_else(|| settings.get("idleReleaseMinutes"));
    match value {
        Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().filter(|n| (1..=1440).contains(n)).unwrap_or(5)),
        None => Some(5),
    }
}
fn rows<'a>(p: &'a Value, key: &str) -> impl DoubleEndedIterator<Item = &'a Value> {
    p[key].as_array().into_iter().flatten()
}
fn active(v: &Value) -> bool {
    matches!(
        text(v, "status"),
        "preparing" | "starting" | "running" | "waiting" | "pending" | "queued"
    )
}
pub fn candidate(p: &Value, settings: &Value, environment: &str, now: i64) -> Value {
    let skip = |status| json!({"status":status});
    let t = &p["thread"];
    let provider_id = text(&t["modelSelection"], "instanceId");
    let Some(minutes) = minutes(settings, environment, provider_id) else {
        return skip("disabled");
    };
    if !valid_id(text(t, "id"))
        || !t["archivedAt"].is_null()
        || !t["deletedAt"].is_null()
        || t["lineage"]["relationshipToParent"] == "subagent"
    {
        return skip("unavailable");
    }
    if !t["activeRunId"].is_null()
        || rows(p, "runs").any(active)
        || rows(p, "runtimeRequests").any(|r| r["status"] == "pending")
        || rows(p, "subagents").any(active)
        || rows(p, "nodes").any(active)
        || rows(p, "plans").any(|r| matches!(text(r, "status"), "proposed" | "pending"))
        || rows(p, "turnItems")
            .any(|r| active(r) && matches!(text(r, "type"), "command_execution" | "tool_call"))
    {
        return skip("busy");
    }
    let Some(provider) = rows(p, "providerThreads")
        .find(|r| r["id"] == t["activeProviderThreadId"] && r["appThreadId"] == t["id"])
    else {
        return skip("unavailable");
    };
    if provider["providerInstanceId"] != t["modelSelection"]["instanceId"] {
        return skip("unavailable");
    }
    let Some(session) = rows(p, "providerSessions").find(|r| {
        r["id"] == provider["providerSessionId"]
            && r["providerInstanceId"] == provider["providerInstanceId"]
    }) else {
        return skip("released");
    };
    if session["status"] != "ready" {
        return skip("busy");
    }
    let driver = text(session, "driver");
    // Generic ACP agents must demonstrate durable load support through the
    // adapter's snapshot capability. Never guess from an instance display name.
    let resumable = matches!(
        driver,
        "codex" | "claudeAgent" | "cursor" | "opencode" | "opencode2" | "pi" | "muse" | "grok"
    ) || (driver == "acpRegistry"
        && session["capabilities"]["threads"]["canReadThreadSnapshot"] == true);
    if !resumable || text(&provider["nativeThreadRef"], "nativeId").is_empty() {
        return skip("unsupported");
    }
    let at = rows(p, "runs")
        .map(|r| protocol::compaction::timestamp(&r["completedAt"]))
        .max()
        .unwrap_or(0)
        .max(protocol::compaction::timestamp(
            &t["latestUserAuthoredMessageAt"],
        ))
        .max(protocol::compaction::timestamp(&session["createdAt"]));
    if at <= 0 || at > now {
        return skip("unavailable");
    }
    let due = at.saturating_add(minutes as i64 * 60_000);
    let episode = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}\0{}\0{}\0{}\0{}",
                text(t, "id"),
                text(session, "id"),
                provider["nativeThreadRef"],
                at,
                session["createdAt"]
            )
            .as_bytes()
        )
    );
    json!({"status":if now >= due {"due"} else {"scheduled"},"episode":episode,"dueAt":due,"sessionId":session["id"],"driver":driver,"provider":provider_id,"threadId":t["id"]})
}
pub fn command(plan: &Value) -> Value {
    json!({"type":"provider-session.detach","commandId":format!("ot3-idle-{}",text(plan,"episode")),"threadId":plan["threadId"],"providerSessionId":plan["sessionId"],"reason":"OT3 idle memory release; resume saved conversation on next message"})
}

/// Idle cache expiry and context-window compaction are separate policies.
/// Codex restores its durable conversation and owns its context-window limit.
pub fn compaction_candidate(p: &Value, settings: &Value, now: i64) -> Value {
    if !protocol::compaction::enabled(settings) {
        return json!({"status":"disabled"});
    }
    let provider =
        rows(p, "providerThreads").find(|r| r["id"] == p["thread"]["activeProviderThreadId"]);
    let driver = provider.map(|r| text(r, "driver")).unwrap_or("");
    if driver == "codex" || provider.is_some_and(|r| r["nativeThreadRef"]["driver"] == "codex") {
        return json!({"status":"harness_managed","provider":p["thread"]["modelSelection"]["instanceId"]});
    }
    protocol::compaction::candidate(p, settings, now)
}
