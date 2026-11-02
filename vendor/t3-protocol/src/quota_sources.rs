//! Public subscription observations only. Raw provider configuration and credentials
//! never cross this boundary. Native drivers retain ownership of credential reads.
use crate::util::text;
use serde_json::{json, Value};

fn string(v: &Value, key: &str, max: usize) -> Option<Value> {
    let s = v[key].as_str()?.trim();
    (!s.is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control))
        .then(|| json!(s))
}
fn copy(out: &mut Value, raw: &Value, keys: &[&str], max: usize) {
    for key in keys {
        if let Some(value) = string(raw, key, max) {
            out[*key] = value;
        }
    }
}
fn timestamp(v: &Value, key: &str) -> Option<Value> {
    let value = string(v, key, 64)?;
    chrono::DateTime::parse_from_rfc3339(value.as_str()?).ok()?;
    Some(value)
}
pub fn limits(raw: &Value) -> Option<Value> {
    let checked = timestamp(raw, "checkedAt")?;
    let mut windows = Vec::new();
    for w in raw["windows"].as_array()?.iter().take(256) {
        let (Some(id), Some(label), Some(used)) = (
            string(w, "id", 256),
            string(w, "label", 256),
            w["usedPercent"]
                .as_f64()
                .filter(|n| n.is_finite() && (0.0..=100.0).contains(n)),
        ) else {
            continue;
        };
        if !matches!(text(w, "kind"), "session" | "weekly" | "monthly" | "other") {
            continue;
        }
        let mut window = json!({"id":id,"label":label,"kind":w["kind"],"usedPercent":used});
        if let Some(reset) = timestamp(w, "resetsAt") {
            window["resetsAt"] = reset;
        }
        if let Some(duration) = w["windowDurationMins"].as_u64().filter(|d| *d <= 5256000) {
            window["windowDurationMins"] = json!(duration);
        }
        windows.push(window);
    }
    let mut out = json!({"checkedAt":checked,"windows":windows});
    copy(&mut out, raw, &["credentialFingerprint"], 256);
    if matches!(
        text(&raw["unavailable"], "reason"),
        "unsupported" | "probeFailed"
    ) {
        out["unavailable"] = json!({"reason":raw["unavailable"]["reason"]});
        copy(&mut out["unavailable"], &raw["unavailable"], &["message"], 1024);
    }
    if let Some(count) = raw["resetCredits"]["availableCount"].as_u64() {
        out["resetCredits"] = json!({"availableCount":count});
        if let Some(reset) = timestamp(&raw["resetCredits"], "nextExpiresAt") {
            out["resetCredits"]["nextExpiresAt"] = reset;
        }
        // Redemption is intentionally not exposed by the read-only panel.
    }
    if let (Some(label), Some(url)) = (
        string(&raw["externalUsage"], "label", 256),
        string(&raw["externalUsage"], "url", 2048),
    ) {
        if reqwest::Url::parse(url.as_str()?).is_ok_and(|u| {
            u.scheme() == "https" && u.username().is_empty() && u.password().is_none()
        }) {
            out["externalUsage"] = json!({"label":label,"url":url});
        }
    }
    Some(out)
}
pub fn providers(raw: &Value) -> Vec<Value> {
    raw.as_array().into_iter().flatten().take(2048).filter_map(|p| {
        let instance=string(p,"instanceId",256)?;
        let driver=string(p,"driver",128)?;
        let mut out=json!({"instanceId":instance,"driver":driver,"enabled":p["enabled"]==true,"installed":p["installed"]==true,"availability":if p["availability"]=="unavailable"{"unavailable"}else{"available"},"auth":{"status":if p["auth"]["status"]=="authenticated"{"authenticated"}else{"unauthenticated"}}});
        copy(&mut out,p,&["displayName","accentColor"],256);
        copy(&mut out["auth"],&p["auth"],&["email","workspaceId","label"],320);
        if let Some(l)=limits(&p["usageLimits"]) {out["usageLimits"]=l;}
        Some(out)
    }).collect()
}
pub fn sources(raw: &Value) -> Vec<Value> {
    raw.as_array()
        .into_iter()
        .flatten()
        .take(512)
        .filter_map(|s| {
            let id = string(s, "id", 256)?;
            let label = string(s, "label", 256).unwrap_or_else(|| id.clone());
            let accounts = s["accounts"]
                .as_array()
                .into_iter()
                .flatten()
                .take(4096)
                .filter_map(|a| {
                    let (id, driver, usage) = (
                        string(a, "id", 256)?,
                        string(a, "driver", 128)?,
                        limits(&a["usageLimits"])?,
                    );
                    let mut out = json!({"id":id,"driver":driver,"usageLimits":usage});
                    copy(&mut out, a, &["email", "workspaceId", "plan"], 320);
                    Some(out)
                })
                .collect::<Vec<_>>();
            let mut out = json!({"id":id,"label":label,"accounts":accounts});
            if s["error"].as_str().is_some_and(|s| !s.is_empty()) {
                out["error"] = json!("Could not read usage source.");
            }
            Some(out)
        })
        .collect()
}
pub fn apply(state: &mut Value, event: &Value) -> bool {
    if event["version"] != 1 {
        return false;
    }
    match text(event, "type") {
        "snapshot" => {
            *state = json!({"providers":providers(&event["config"]["providers"]),"usageLimitSources":[],"connected":true});
        }
        "providerStatuses" => {
            state["providers"] = json!(providers(&event["payload"]["providers"]));
        }
        "usageLimitSourcesUpdated" => {
            state["usageLimitSources"] = json!(sources(&event["payload"]["sources"]));
        }
        _ => return false,
    }
    true
}
