//! Preventive idle compaction policy. No processes, storage or timers; reusable by Omi.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use crate::util::text;
fn rows<'a>(v: &'a Value, k: &str) -> impl DoubleEndedIterator<Item = &'a Value> {
    v[k].as_array().into_iter().flatten()
}
pub fn timestamp(v: &Value) -> i64 {
    v.as_i64()
        .or_else(|| {
            v.as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp_millis())
        })
        .unwrap_or(0)
}
pub fn enabled(settings: &Value) -> bool {
    settings["autoCompactEnabled"] != false
}
pub fn candidate(p: &Value, settings: &Value, now: i64) -> Value {
    let skip = |reason: &str| json!({"status":reason});
    if !enabled(settings) {
        return skip("disabled");
    }
    let t = &p["thread"];
    // Rechecked on the fresh projection immediately before dispatch as well.
    if t["settledOverride"] == "settled"
        || t["settledOverride"] == true
        || (t["settledOverride"] != "active" && timestamp(&t["settledAt"]) > 0)
    {
        return skip("unavailable");
    }
    if t["archivedAt"].is_string()
        || t["deletedAt"].is_string()
        || t["lineage"]["relationshipToParent"] == "subagent"
    {
        return skip("unavailable");
    }
    if !t["activeRunId"].is_null()
        || rows(p, "runs").any(|r| {
            matches!(
                text(r, "status"),
                "running" | "preparing" | "starting" | "waiting" | "queued"
            )
        })
    {
        return skip("busy");
    }
    if rows(p, "runtimeRequests").any(|r| r["status"] == "pending")
        || rows(p, "plans").any(|r| matches!(text(r, "status"), "proposed" | "pending"))
        || rows(p, "subagents")
            .any(|r| matches!(text(r, "status"), "running" | "waiting" | "pending"))
    {
        return skip("blocked");
    }
    let Some(provider) = rows(p, "providerThreads")
        .find(|r| r["id"] == t["activeProviderThreadId"] && r["appThreadId"] == t["id"])
    else {
        return skip("unknown_context");
    };
    if provider["providerInstanceId"] != t["modelSelection"]["instanceId"] {
        return skip("provider_changed");
    }
    if rows(p, "runs")
        .rev()
        .find(|r| r["providerThreadId"] == provider["id"])
        .is_some_and(|r| matches!(text(r, "status"), "failed" | "interrupted"))
    {
        return skip("unavailable");
    }
    let Some(message) = rows(p, "messages").rev().find(|m| {
        m["role"] == "user"
            && m["createdBy"] == "user"
            && m["notification"].is_null()
            && m["delegatedCompletion"].is_null()
            && text(m, "text").trim().to_lowercase() != "/compact"
    }) else {
        return skip("unknown_context");
    };
    let report = rows(p, "providerTurns")
        .filter(|r| r["providerThreadId"] == provider["id"] && r["tokenUsage"].is_object())
        .max_by_key(|r| timestamp(&r["tokenUsage"]["updatedAt"]));
    let Some(turn) = report else {
        return skip("unknown_context");
    };
    let usage = &turn["tokenUsage"];
    let updated = timestamp(&usage["updatedAt"]);
    if updated <= 0 || updated > now || timestamp(&message["createdAt"]) > updated {
        return skip("unknown_context");
    }
    // Wait until the whole native run is idle; tool execution time never keeps a stale cache warm.
    let end = rows(p, "runs")
        .filter(|r| r["providerThreadId"] == provider["id"])
        .map(|r| timestamp(&r["completedAt"]))
        .max()
        .unwrap_or(0)
        .max(updated);
    let tokens = usage["usedTokens"].as_u64().unwrap_or(0);
    let minimum = settings["autoCompactMinTokens"]
        .as_u64()
        .unwrap_or(20_000)
        .clamp(1_000, 1_000_000);
    let instance = text(provider, "providerInstanceId");
    let scoped = format!("{}/{}", text(settings, "autoCompactEnvironment"), instance);
    let configured = settings["autoCompactProviderMinutes"][&scoped]
        .as_i64()
        .or(settings["autoCompactProviderMinutes"][instance].as_i64())
        .or(settings["autoCompactFallbackMinutes"].as_i64())
        .unwrap_or(60)
        .clamp(1, 1440)
        * 60_000;
    let reported = usage["promptCacheTtlMs"]
        .as_i64()
        .filter(|v| *v > 0 && *v <= 86_400_000);
    let ttl = reported.unwrap_or(configured);
    let expires = updated.saturating_add(ttl);
    let margin = (ttl / 10).clamp(15_000, 300_000);
    let due = (expires - margin).max(end + 30_000);
    let episode = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}\0{}\0{}",
                text(t, "id"),
                text(provider, "id"),
                text(message, "id")
            )
            .as_bytes()
        )
    );
    let mut result = json!({"status":"scheduled","episode":episode,"provider":instance,"modelSelection":t["modelSelection"],"tokens":tokens,"reportAt":updated,"dueAt":due,"expiresAt":expires,"estimated":reported.is_none(),"ttlMs":ttl});
    result["status"] = json!(if tokens < minimum {
        "too_small"
    } else if now >= expires || due >= expires {
        "expired"
    } else if now < due {
        "scheduled"
    } else {
        "due"
    });
    result
}
pub fn same_episode(a: &Value, b: &Value) -> bool {
    a["episode"].is_string() && a["episode"] == b["episode"]
}
pub fn consumed(v: &Value) -> bool {
    matches!(
        text(v, "status"),
        "claimed" | "accepted" | "uncertain" | "failed" | "cancelled"
    )
}
/// Explicit start mode, no deliveryIntent: never steer or restart a human's run.
pub fn command(thread: &str, id: &str, selection: &Value) -> Value {
    json!({"type":"message.dispatch","commandId":format!("omarchy-compact-{id}"),"threadId":thread,"messageId":format!("omarchy-compact-{id}"),"text":"/compact","attachments":[],"createdBy":"user","creationSource":"web","modelSelection":selection,"dispatchMode":{"type":"start_immediately"}})
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        json!({"thread":{"id":"t","activeRunId":null,"activeProviderThreadId":"pt","modelSelection":{"instanceId":"codex","model":"m"}},"providerThreads":[{"id":"pt","appThreadId":"t","providerInstanceId":"codex"}],"messages":[{"id":"u","role":"user","createdBy":"user","text":"task","createdAt":1000}],"providerTurns":[{"id":"v","providerThreadId":"pt","tokenUsage":{"usedTokens":50_000,"updatedAt":10_000,"promptCacheTtlMs":3_600_000}}],"runs":[{"id":"r","providerThreadId":"pt","status":"completed","completedAt":20_000}]})
    }
    #[test]
    fn deadlines_known_estimated_and_no_expired_work() {
        let p = fixture();
        let v = candidate(&p, &json!({}), 30_000);
        assert_eq!(v["estimated"], false);
        assert_eq!(
            candidate(
                &p,
                &json!({"autoCompactProviderMinutes":{"codex":5}}),
                30_000
            )["ttlMs"],
            3_600_000
        );
        assert_eq!(v["dueAt"], 3_310_000);
        assert_eq!(candidate(&p, &json!({}), 3_320_000)["status"], "due");
        assert_eq!(candidate(&p, &json!({}), 3_610_000)["status"], "expired");
        let mut q = p.clone();
        q["providerTurns"][0]["tokenUsage"]
            .as_object_mut()
            .unwrap()
            .remove("promptCacheTtlMs");
        let v = candidate(
            &q,
            &json!({"autoCompactProviderMinutes":{"codex":5}}),
            30_000,
        );
        assert_eq!(v["estimated"], true);
        assert_eq!(v["ttlMs"], 300_000);
        assert_eq!(v["dueAt"], 280_000);
        assert_eq!(
            candidate(&p, &json!({"autoCompactEnabled":false}), 40_000)["status"],
            "disabled"
        );
        assert_eq!(
            candidate(&p, &json!({"autoCompactMinTokens":100_000}), 40_000)["status"],
            "too_small"
        );
    }
    #[test]
    fn no_active_queue_requests_subagents_or_wrong_provider() {
        for status in ["running", "queued", "preparing", "waiting"] {
            let mut p = fixture();
            p["runs"][0]["status"] = json!(status);
            assert_eq!(candidate(&p, &json!({}), 3_320_000)["status"], "busy")
        }
        let mut p = fixture();
        p["runtimeRequests"] = json!([{"status":"pending"}]);
        assert_eq!(candidate(&p, &json!({}), 3_320_000)["status"], "blocked");
        p = fixture();
        p["providerThreads"][0]["providerInstanceId"] = json!("other");
        assert_eq!(
            candidate(&p, &json!({}), 3_320_000)["status"],
            "provider_changed"
        );
        p = fixture();
        p["providerTurns"] = json!([]);
        assert_eq!(
            candidate(&p, &json!({}), 3_320_000)["status"],
            "unknown_context"
        );
    }
    #[test]
    fn settled_threads_stay_asleep_until_explicitly_active() {
        for override_value in [json!("settled"), json!(true), Value::Null] {
            let mut p = fixture();
            p["thread"]["settledOverride"] = override_value;
            p["thread"]["settledAt"] = json!(25_000);
            assert_eq!(
                candidate(&p, &json!({}), 3_320_000)["status"],
                "unavailable"
            );
            p["thread"]["settledOverride"] = json!("active");
            assert_eq!(candidate(&p, &json!({}), 3_320_000)["status"], "due");
        }
        let mut p = fixture();
        p["thread"]["settledOverride"] = json!("settled");
        assert_eq!(
            candidate(&p, &json!({}), 3_320_000)["status"],
            "unavailable"
        );
    }
    #[test]
    fn own_compaction_never_rearms_same_human_episode() {
        let mut p = fixture();
        let before = candidate(&p, &json!({}), 3_320_000);
        p["messages"].as_array_mut().unwrap().push(json!({"id":"compact","role":"user","createdBy":"user","text":"/compact","createdAt":3_320_000}));
        assert!(same_episode(&before, &candidate(&p, &json!({}), 3_320_000)));
        p["messages"].as_array_mut().unwrap().push(json!({"id":"next","role":"user","createdBy":"user","text":"continue","createdAt":5000}));
        assert!(!same_episode(
            &before,
            &candidate(&p, &json!({}), 3_320_000)
        ));
        let c = command("t", "stable", &json!({"instanceId":"codex","model":"m"}));
        assert_eq!(c["text"], "/compact");
        assert!(c.get("deliveryIntent").is_none());
        assert_eq!(c["dispatchMode"]["type"], "start_immediately");
        assert!(consumed(&json!({"status":"uncertain"})));
    }
}

/// Server-started idle compactions are quiet maintenance, separate from manual `/compact`.
pub fn idle_run(projection: &Value, id: &str) -> bool {
    !id.is_empty() && rows(projection,"messages").any(|m| text(m,"runId")==id && matches!(text(m,"idleCompaction"),"beforeCacheExpires"|"onReturn"))
}
/// Replace an automatic compaction's prompt, response and tool log with one work row.
/// Failures remain visible; unrelated human messages are never collapsed.
pub fn timeline(projection: &Value, timeline: &mut Vec<Value>) {
    let mut runs=std::collections::BTreeMap::new();
    for message in rows(projection,"messages") {
        if matches!(text(message,"idleCompaction"),"beforeCacheExpires"|"onReturn") && !text(message,"runId").is_empty() {
            runs.insert(text(message,"runId"),message);
        }
    }
    if runs.is_empty(){return}
    let message_runs=rows(projection,"messages").map(|m|(text(m,"id"),text(m,"runId"))).collect::<std::collections::HashMap<_,_>>();
    timeline.retain(|row| {
        if row["role"]=="error" {return true}
        let run=message_runs.get(text(row,"id")).copied().unwrap_or_else(||text(&row["activity"],"runId"));
        !runs.contains_key(run)
    });
    for (run_id,message) in runs {
        let run=rows(projection,"runs").find(|r|text(r,"id")==run_id);
        let status=run.map(|r|text(r,"status")).unwrap_or("completed");
        let active=matches!(status,"preparing"|"starting"|"running"|"waiting");
        let activity=json!({"id":format!("compact/{run_id}"),"type":"compaction","category":"compact","runId":run_id,"status":status,"active":active,"failed":status=="failed","count":1,"entries":[]});
        timeline.push(json!({"id":format!("compact/{run_id}"),"role":"activity","createdAt":crate::util::epoch(&message["createdAt"]),"text":"","activity":activity}));
    }
    timeline.sort_by_key(|row|crate::util::epoch(&row["createdAt"]));
}
