//! Usage pool presentation, port of Desktop NativeUsageLimits accountKeyResolver,
//! collectLimitAccounts and poolWindows (T3 MIT). No credential/config fields.
use crate::{util::text, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
fn rows<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v[k].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn time(v: &Value) -> Option<i64> {
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp_millis())
}
fn base(a: &Value) -> Option<String> {
    let email = text(a, "email").trim().to_lowercase();
    let driver = text(a, "driver");
    if !email.is_empty() {
        Some(format!("{driver}:{email}"))
    } else {
        let fp = text(&a["usageLimits"], "credentialFingerprint");
        (!fp.is_empty()).then(|| format!("{driver}:credential:{fp}"))
    }
}
fn elapsed(w: &Value, now: i64) -> Option<f64> {
    let length = w["windowDurationMins"].as_f64()? * 60000.;
    if length <= 0. {
        None
    } else {
        Some(((length - (time(&w["resetsAt"])? - now) as f64) / length).clamp(0., 1.))
    }
}
fn notice(l: &Value) -> Option<&str> {
    match text(&l["unavailable"], "reason") {
        "unsupported" => Some(
            l["unavailable"]["message"]
                .as_str()
                .unwrap_or("This account has no subscription limits."),
        ),
        "probeFailed" => Some(
            l["unavailable"]["message"]
                .as_str()
                .unwrap_or("Could not read limits."),
        ),
        _ if rows(l, "windows").is_empty() => Some("No limits reported."),
        _ => None,
    }
}
fn reset(w: &Value, now: i64) -> Value {
    let Some(at) = time(&w["resetsAt"]) else {
        return Value::Null;
    };
    let ms = (at - now).max(0);
    let duration = if ms >= 86400000 {
        format!("{}d {}h", ms / 86400000, ms % 86400000 / 3600000)
    } else if ms >= 3600000 {
        format!("{}h {}m", ms / 3600000, ms % 3600000 / 60000)
    } else {
        format!("{}m", ms / 60000)
    };
    json!({"key":if at<=now{"resets now"}else if ms<60000{"Resets momentarily"}else{"resets in {time}"},"args":{"time":duration}})
}
fn kind(w: &Value) -> usize {
    ["session", "weekly", "monthly", "other"]
        .iter()
        .position(|k| w["kind"] == *k)
        .unwrap_or(4)
}
fn pool_key(driver: &str, w: &Value) -> String {
    if driver == "codex" {
        text(w, "kind").to_string()
    } else {
        format!("{}:{}", text(w, "kind"), text(w, "id"))
    }
}
fn effective(a: &Value, w: &Value, now: i64) -> Value {
    let driver = text(a, "driver");
    let spent = rows(&a["usageLimits"], "windows").iter().find(|x| {
        x["usedPercent"].as_f64().unwrap_or(0.) >= 100.
            && ((driver == "claudeAgent" && x["id"] == "seven_day")
                || (driver == "codex" && x["kind"] == "weekly"))
    });
    let mut w = w.clone();
    if w["kind"] == "session" {
        if let Some(weekly) = spent {
            let cutoff = now.max(time(&a["usageLimits"]["checkedAt"]).unwrap_or(now));
            let wr = time(&weekly["resetsAt"]);
            let sr = time(&w["resetsAt"]);
            let at = match (wr, sr) {
                (Some(wr), Some(sr)) if wr > cutoff && sr > cutoff => {
                    if sr <= wr || w["usedPercent"].as_f64().unwrap_or(0.) < 100. {
                        weekly["resetsAt"].clone()
                    } else {
                        w["resetsAt"].clone()
                    }
                }
                _ => Value::Null,
            };
            w["usedPercent"] = json!(100);
            w["resetsAt"] = at;
            w["windowDurationMins"] = Value::Null;
        }
    }
    w
}
pub fn present(environments: &[Value], now: i64) -> Result<Value> {
    let mut reports = vec![];
    let mut notices = vec![];
    let mut external = BTreeMap::new();
    for raw in environments {
        if !raw.is_object() || !raw["state"].is_object() {
            return Err("invalid_environment".into());
        }
        crate::api::ids::id("environmentId", text(raw, "id"))?;
        let mut e = raw.clone();
        e["state"]["providers"] =
            json!(crate::quota_sources::providers(&raw["state"]["providers"]));
        e["state"]["usageLimitSources"] = json!(crate::quota_sources::sources(
            &raw["state"]["usageLimitSources"]
        ));
        let e = &e;
        let name = e["name"].as_str().unwrap_or(text(e, "id"));
        for (flag, key) in [
            (
                e["connected"] == false,
                "Disconnected; readings may be outdated",
            ),
            (e["refreshFailed"] == true, "Could not refresh limits"),
            (
                e["error"] == "unauthorized" || e["error"] == "permission_denied",
                "Access denied",
            ),
        ] {
            if flag {
                notices.push(json!({"key":key,"environmentId":e["id"],"host":name}));
            }
        }
        for p in rows(&e["state"], "providers").iter().filter(|p| {
            p["enabled"] == true && p["installed"] == true && p["availability"] != "unavailable"
        }) {
            let l = &p["usageLimits"];
            if let Some(n) = notice(l) {
                if l["unavailable"]["reason"] != "unsupported" {
                    notices.push(json!({"key":n,"host":name,"provider":p["displayName"]}));
                }
            }
            if p["auth"]["status"] == "authenticated" {
                if let Some(url) = l["externalUsage"]["url"].as_str() {
                    external.insert(url.to_string(), l["externalUsage"].clone());
                }
            }
            if notice(l).is_none() {
                reports.push(json!({"fallback":format!("{}:{}",text(e,"id"),text(p,"instanceId")),"driver":p["driver"],"email":p["auth"]["email"],"workspaceId":p["auth"]["workspaceId"],"plan":p["auth"]["label"],"label":p["displayName"],"hosts":[name],"usageLimits":l}));
            }
        }
        for s in rows(&e["state"], "usageLimitSources") {
            if s["error"].is_string() {
                notices.push(
                    json!({"key":"Could not read usage source.","host":name,"source":s["label"]}),
                );
            }
            for a in rows(s, "accounts") {
                if notice(&a["usageLimits"]).is_none() {
                    let mut report = a.clone();
                    report["fallback"] = json!(format!(
                        "{}:{}:{}",
                        text(s, "id"),
                        text(a, "id"),
                        text(a, "workspaceId")
                    ));
                    report["hosts"] = json!([]);
                    report["label"] = a.get("email").cloned().unwrap_or(a["id"].clone());
                    reports.push(report);
                }
            }
        }
    }
    let mut workspaces: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for a in &reports {
        if let Some(key) = base(a) {
            if !text(a, "workspaceId").is_empty() {
                workspaces
                    .entry(key)
                    .or_default()
                    .insert(text(a, "workspaceId").into());
            }
        }
    }
    let mut accounts: BTreeMap<String, Value> = BTreeMap::new();
    for a in reports {
        let key = base(&a)
            .and_then(|base| {
                let ws = text(&a, "workspaceId");
                if !ws.is_empty() {
                    Some(format!("{base}:{ws}"))
                } else {
                    match workspaces.get(&base) {
                        None => Some(base),
                        Some(known) if known.len() == 1 => {
                            Some(format!("{base}:{}", known.first().unwrap()))
                        }
                        _ => None,
                    }
                }
            })
            .unwrap_or_else(|| text(&a, "fallback").to_string());
        if let Some(prev) = accounts.get_mut(&key) {
            let hosts = rows(prev, "hosts")
                .iter()
                .chain(rows(&a, "hosts"))
                .cloned()
                .map(ValueKey)
                .collect::<BTreeSet<ValueKey>>();
            let host_values = hosts.into_iter().map(|v| v.0).collect::<Vec<_>>();
            if time(&a["usageLimits"]["checkedAt"]) > time(&prev["usageLimits"]["checkedAt"]) {
                *prev = a;
            }
            prev["hosts"] = json!(host_values);
        } else {
            accounts.insert(key, a);
        }
    }
    let mut drivers: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (key, mut a) in accounts {
        a["key"] = json!(key);
        drivers
            .entry(text(&a, "driver").into())
            .or_default()
            .push(a);
    }
    let mut providers = vec![];
    for (driver, mut accounts) in drivers {
        accounts.sort_by_key(|a| {
            let first = rows(&a["usageLimits"], "windows")
                .iter()
                .min_by_key(|w| kind(w));
            (
                first
                    .and_then(|w| time(&effective(a, w, now)["resetsAt"]))
                    .unwrap_or(i64::MAX),
                text(a, "label").to_lowercase(),
                text(a, "key").to_string(),
            )
        });
        let mut pools: BTreeMap<String, Vec<(usize, Value)>> = BTreeMap::new();
        for (i, a) in accounts.iter().enumerate() {
            for w in rows(&a["usageLimits"], "windows") {
                let w = effective(a, w, now);
                let mut key = pool_key(&driver, &w);
                if pools
                    .get(&key)
                    .is_some_and(|ms| ms.iter().any(|(j, _)| *j == i))
                {
                    key = format!("{}:{}", text(&w, "kind"), text(&w, "id"));
                }
                pools.entry(key).or_default().push((i, w));
            }
        }
        let mut windows = vec![];
        for (key, members) in pools {
            let first = &members[0].1;
            let used = members
                .iter()
                .map(|(_, w)| w["usedPercent"].as_f64().unwrap_or(0.))
                .sum::<f64>()
                / members.len() as f64;
            let timed = members
                .iter()
                .filter_map(|(_, w)| {
                    elapsed(w, now)
                        .map(|elapsed| (w["usedPercent"].as_f64().unwrap_or(0.), elapsed))
                })
                .collect::<Vec<_>>();
            let expected = (!timed.is_empty()).then(|| {
                (1. - timed.iter().map(|(_, e)| e).sum::<f64>() / timed.len() as f64) * 100.
            });
            let pace = expected.map(|e| {
                let gap =
                    timed.iter().map(|(u, _)| u).sum::<f64>() / timed.len() as f64 - (100. - e);
                if gap > 5. {
                    "ahead"
                } else if gap < -5. {
                    "under"
                } else {
                    "on"
                }
            });
            let segments=accounts.iter().enumerate().map(|(i,a)|members.iter().find(|(j,_)|*j==i).map(|(_,w)|json!({"key":a["key"],"availablePercent":(100.-w["usedPercent"].as_f64().unwrap_or(0.)).round(),"expectedAvailable":elapsed(w,now).map(|e|(1.-e)*100.),"resetsAt":w["resetsAt"],"reset":reset(w,now)})).unwrap_or(Value::Null)).collect::<Vec<_>>();
            let next = members
                .iter()
                .filter(|(_, w)| time(&w["resetsAt"]).is_some())
                .min_by_key(|(_, w)| time(&w["resetsAt"]));
            windows.push(json!({"id":if driver=="codex"{key.as_str()}else{text(first,"id")},"kind":first["kind"],"label":first["label"],"availablePercent":(100.-used).round(),"expectedAvailable":expected,"pace":pace,"segments":segments,"nextReset":next.map(|(_,w)|reset(w,now))}));
        }
        windows.sort_by_key(kind);
        if driver == "cursor"
            && windows.iter().any(|w| w["id"] == "autoPercentUsed")
            && windows.iter().any(|w| w["id"] == "apiPercentUsed")
        {
            windows.retain(|w| w["id"] != "totalPercentUsed");
        }
        if driver == "cursor" {
            windows.sort_by_key(|w| {
                ["totalPercentUsed", "autoPercentUsed", "apiPercentUsed"]
                    .iter()
                    .position(|id| w["id"] == *id)
                    .unwrap_or(3)
            });
        }
        if driver == "antigravity" {
            windows.sort_by_key(|w| {
                let id = text(w, "id");
                let stem = ["-5h", "-weekly", "-quota"]
                    .iter()
                    .find_map(|suffix| id.strip_suffix(suffix))
                    .unwrap_or(id);
                (!id.starts_with("gemini-"), stem.to_string(), kind(w))
            });
        }
        let account_views=accounts.iter().map(|a|json!({"key":a["key"],"label":a["label"].as_str().filter(|s|!s.is_empty()).or(a["email"].as_str()).unwrap_or("Account"),"plan":a["plan"],"hosts":rows(a,"hosts").iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),"resetCredits":a["usageLimits"]["resetCredits"],"windows":rows(&a["usageLimits"],"windows").iter().map(|w|json!({"label":w["label"],"availablePercent":(100.-w["usedPercent"].as_f64().unwrap_or(0.)).round(),"reset":reset(w,now),"resetsAt":w["resetsAt"]})).collect::<Vec<_>>()})).collect::<Vec<_>>();
        let label = match driver.as_str() {
            "codex" => "Codex",
            "claudeAgent" => "Claude",
            "opencode" => "OpenCode Go",
            "claudeCode" | "claude" => "Claude",
            "cursor" => "Cursor",
            "antigravity" => "Antigravity",
            "kimi-coding" => "Kimi",
            "nous" => "Nous",
            "grok" => "Grok",
            "pi" => "Pi",
            _ => &driver,
        };
        providers.push(json!({"provider":driver,"label":label,"accounts":account_views,"weekly":windows.iter().find(|w|w["kind"]=="weekly"),"shortWindow":windows.iter().find(|w|w["kind"]=="session"),"windows":windows}));
    }
    Ok(
        json!({"providers":providers,"notices":notices,"external":external.into_values().collect::<Vec<_>>()}),
    )
}
// Host labels are strings; avoid a serde_json::Value ordering dependency.
#[derive(Eq, PartialEq)]
struct ValueKey(Value);
impl Ord for ValueKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.as_str().cmp(&other.0.as_str())
    }
}
impl PartialOrd for ValueKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl From<Value> for ValueKey {
    fn from(v: Value) -> Self {
        Self(v)
    }
}
