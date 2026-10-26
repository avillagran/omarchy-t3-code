//! Subagent cards as ot3 draws them: `ui/js/Subagents.js` (status buckets,
//! labels, tones, detail line, elapsed time, the live overlay and sub-threads
//! seen as subagents) and the group header of `T3SubagentCards.qml`. Labels are
//! English source strings for the catalog (`i18n.t`).
use crate::js;
use serde_json::{json, Map, Value};

const LIVE: &[&str] = &["pending", "running", "waiting"];
const SETTLED: &[&str] = &["completed", "failed", "cancelled", "interrupted", "idle"];

fn status(s: &Value) -> Option<&str> {
    s["status"].as_str()
}
/// Queued, running or waiting.
pub fn live(s: &Value) -> bool {
    js::truthy(s) && status(s).is_some_and(|st| LIVE.contains(&st))
}
pub fn settled(s: &Value) -> bool {
    js::truthy(s) && status(s).is_some_and(|st| SETTLED.contains(&st))
}
/// The status label (English source string).
pub fn label(status: &str) -> &'static str {
    match status {
        "pending" => "Queued",
        "running" => "Running",
        "waiting" => "Waiting",
        "idle" => "Idle · resumable",
        "completed" => "Completed",
        "failed" => "Failed",
        "cancelled" | "interrupted" => "Stopped",
        _ => "Unknown",
    }
}
/// The sidebar state whose colour the status takes: waiting, working, done, failed or idle.
pub fn tone(status: &str) -> &'static str {
    match status {
        "waiting" => "waiting",
        s if LIVE.contains(&s) => "working",
        "completed" => "done",
        "failed" => "failed",
        _ => "idle",
    }
}
/// The detail line: progress first while live, the result first once settled.
pub fn detail(s: &Value) -> String {
    if !js::truthy(s) {
        return String::new();
    }
    let (a, b) = if live(s) { (&s["progress"], &s["result"]) } else { (&s["result"], &s["progress"]) };
    [a, b].into_iter().find(|v| js::truthy(v)).map(js::string).unwrap_or_default()
}
/// "1h 05m", "4m 09s" or "12s"; empty unless `ms` is positive.
pub fn compact_duration(ms: f64) -> String {
    // JavaScript's !(ms > 0): NaN, zero and negatives.
    if ms.is_nan() || ms <= 0.0 {
        return String::new();
    }
    let s = (ms / 1000.0).floor();
    let m = (s / 60.0).floor();
    let h = (m / 60.0).floor();
    let two = |n: f64| if n < 10.0 { format!("0{}", js::number(n)) } else { js::number(n) };
    if h > 0.0 {
        format!("{}h {}m", js::number(h), two(m % 60.0))
    } else if m > 0.0 {
        format!("{}m {}s", js::number(m), two(s % 60.0))
    } else {
        format!("{}s", js::number(s))
    }
}
/// JavaScript's ToNumber for the timestamps (milliseconds) this compares.
fn to_number(v: &Value) -> f64 {
    match v {
        Value::Null => 0.0,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => {
            let t = s.trim_matches(js::is_space);
            if t.is_empty() { 0.0 } else { t.parse::<f64>().ok().filter(|_| !t.contains(['i', 'n', 'N'])).unwrap_or(f64::NAN) }
        }
        _ => f64::NAN,
    }
}
/// `a > b` as JavaScript compares two JSON values.
fn greater(a: &Value, b: &Value) -> bool {
    match (a.as_str(), b.as_str()) {
        (Some(x), Some(y)) => x.encode_utf16().cmp(y.encode_utf16()).is_gt(),
        _ => to_number(a) > to_number(b),
    }
}
/// Time since it started: until `now` while live, else until it completed or last updated.
pub fn elapsed(s: &Value, now: i64) -> String {
    if !js::truthy(s) || !js::truthy(&s["startedAt"]) {
        return String::new();
    }
    let end = if live(s) {
        json!(now)
    } else {
        [&s["completedAt"], &s["updatedAt"]].into_iter().find(|v| js::truthy(v)).cloned().unwrap_or(json!(0))
    };
    if greater(&end, &s["startedAt"]) { compact_duration(to_number(&end) - to_number(&s["startedAt"])) } else { String::new() }
}
/// A card's copy from the timeline overlaid with the thread's live subagent of the same id.
pub fn merge(card: &Value, list: &[Value]) -> Value {
    let id = &card["id"];
    let fresh = (!id.is_null()).then(|| list.iter().find(|s| s["id"] == *id)).flatten();
    match (fresh, card.as_object()) {
        (Some(fresh), Some(base)) => {
            let mut out = base.clone();
            for (k, v) in fresh.as_object().into_iter().flatten() {
                out.insert(k.clone(), v.clone());
            }
            Value::Object(out)
        }
        _ => card.clone(),
    }
}
/// `{working, done, failed, stopped, idle}` counts (T3 summarizeSubagentStatuses).
pub fn buckets(list: &[Value]) -> Value {
    let mut b = [0u64; 5];
    for s in list {
        let i = if live(s) {
            0
        } else {
            match status(s) {
                Some("completed") => 1,
                Some("failed") => 2,
                Some("idle") => 4,
                _ => 3,
            }
        };
        b[i] += 1;
    }
    json!({"working":b[0],"done":b[1],"failed":b[2],"stopped":b[3],"idle":b[4]})
}
/// A sub-thread from the sidebar seen as a subagent (`fromThread`).
pub fn from_thread(t: &Value) -> Value {
    let st = if js::truthy(&t["working"]) {
        "running"
    } else if js::truthy(&t["pending"]) && t["reason"] == "waiting" {
        "waiting"
    } else if t["status"] == "failed" {
        "failed"
    } else if t["status"] == "completed" {
        "completed"
    } else {
        "idle"
    };
    let or = |v: &Value, d: Value| if js::truthy(v) { v.clone() } else { d };
    let key = t.get("key").map(js::string).unwrap_or_else(|| "undefined".into());
    let mut out = Map::new();
    out.insert("id".into(), json!(format!("thread:{key}")));
    out.insert("title".into(), or(&t["title"], json!("")));
    out.insert("status".into(), json!(st));
    // Members JavaScript leaves undefined are left out.
    if let Some(id) = t.get("id") {
        out.insert("childThreadId".into(), id.clone());
    }
    out.insert("startedAt".into(), if js::truthy(&t["working"]) { or(&t["startedAt"], json!(0)) } else { json!(0) });
    out.insert("updatedAt".into(), or(&t["updatedAt"], json!(0)));
    for (to, from) in [("providerInstanceId", "provider"), ("model", "model")] {
        if let Some(v) = t.get(from) {
            out.insert(to.into(), v.clone());
        }
    }
    out.insert("fromThread".into(), json!(true));
    Value::Object(out)
}
/// One subagent row: `{live, settled, label, tone, detail, elapsed}`. ot3
/// writes the row's status as the translated label, then " · " and `elapsed`
/// when there is one.
pub fn describe(s: &Value, now: i64) -> Value {
    let st = status(s).unwrap_or("");
    json!({"live":live(s),"settled":settled(s),"label":label(st),"tone":tone(st),"detail":detail(s),"elapsed":elapsed(s, now)})
}
/// The cards of one timeline subagents row (`T3SubagentCards`): each card
/// overlaid with the thread's live subagents and described, the group header
/// ("{n} subagents" and its "2 working · 1 done" parts), whether it is a group
/// and whether it opens expanded (one card, at most three, or any live).
pub fn cards(cards: &[Value], thread_subagents: &[Value], now: i64) -> Value {
    let subagents = cards.iter().map(|c| merge(c, thread_subagents)).collect::<Vec<_>>();
    let group = subagents.len() > 1;
    let any_live = subagents.iter().any(live);
    let counts = buckets(&subagents);
    let summary = ["working", "done", "failed", "stopped", "idle"]
        .iter()
        .filter(|k| counts[**k].as_u64().unwrap_or(0) > 0)
        .map(|k| json!({"source":format!("{{n}} {k}"),"args":{"n":counts[*k]}}))
        .collect::<Vec<_>>();
    let rows = subagents.iter().map(|s| json!({"subagent":s,"view":describe(s, now)})).collect::<Vec<_>>();
    json!({"subagents":rows,"group":group,"anyLive":any_live,"expanded":!group || subagents.len() <= 3 || any_live,
        "counts":counts,"header":{"source":"{n} subagents","args":{"n":subagents.len()}},"summary":summary})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ot3_subagent_rules() {
        assert_eq!(compact_duration(3_725_000.0), "1h 02m");
        assert_eq!(compact_duration(249_000.0), "4m 09s");
        assert_eq!(compact_duration(9_999.0), "9s");
        assert_eq!(compact_duration(0.0), "");
        let running = json!({"id":"a","status":"running","startedAt":1000,"progress":"step 2","result":"old"});
        assert_eq!((elapsed(&running, 250_000), detail(&running)), ("4m 09s".to_string(), "step 2".to_string()));
        let done = json!({"id":"b","status":"completed","startedAt":1000,"completedAt":61_000,"progress":"p","result":""});
        assert_eq!((elapsed(&done, 0), detail(&done), tone("completed"), label("cancelled")), ("1m 00s".to_string(), "p".to_string(), "done", "Stopped"));
        let list = [running.clone(), done, json!({"status":"idle"}), json!({"status":"cancelled"}), json!({"status":"waiting"})];
        assert_eq!(buckets(&list), json!({"working":2,"done":1,"failed":0,"stopped":1,"idle":1}));
        assert_eq!(merge(&json!({"id":"a","title":"T","status":"pending"}), &[json!({"id":"a","status":"running"})]), json!({"id":"a","title":"T","status":"running"}));
        assert_eq!(merge(&json!({"title":"T"}), &[json!({"status":"running"})]), json!({"title":"T"}));
        assert_eq!(from_thread(&json!({"key":"e/t","id":"t","working":true,"startedAt":5,"provider":"codex"})), json!({"id":"thread:e/t","title":"","status":"running","childThreadId":"t","startedAt":5,"updatedAt":0,"providerInstanceId":"codex","fromThread":true}));
        let c = cards(&list, &[], 250_000);
        assert_eq!((c["group"].as_bool(), c["expanded"].as_bool(), c["header"]["args"]["n"].as_u64()), (Some(true), Some(true), Some(5)));
        assert_eq!(c["summary"], json!([{"source":"{n} working","args":{"n":2}},{"source":"{n} done","args":{"n":1}},{"source":"{n} stopped","args":{"n":1}},{"source":"{n} idle","args":{"n":1}}]));
    }
}
