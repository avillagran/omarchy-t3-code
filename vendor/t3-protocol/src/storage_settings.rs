//! Storage preference contracts from T3 Code settings.ts/projectSettings.ts.
//! Only settings are projected here; cleanup execution belongs to the server.
use crate::util::{text, Result};
use serde_json::{json, Value};
const RULES: &[&str] = &[
    "worktreeKeepWhen",
    "worktreeAfterDays",
    "worktreeSettledAfterDays",
    "worktreeOnMerge",
    "worktreeOnDelete",
    "worktreeUnchanged",
];
fn valid(key: &str, value: &Value) -> bool {
    match key {
        "worktreeKeepWhen" => matches!(
            value.as_str(),
            Some("any-local-files" | "uncommitted-changes" | "tracked-changes")
        ),
        "worktreeAfterDays" | "browserArtifactsAfterDays" | "logsAfterDays" => {
            value.is_null()
                || value.as_f64().is_some_and(|n| {
                    n.is_finite() && n.fract() == 0. && (1.0..=3650.0).contains(&n)
                })
        }
        // Settled worktrees may go at the next cleanup, so 0 days is allowed.
        "worktreeSettledAfterDays" => {
            value.is_null()
                || value
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n.fract() == 0. && (0.0..=3650.0).contains(&n))
        }
        "worktreeOnMerge" | "worktreeOnDelete" | "worktreeUnchanged" => value.is_boolean(),
        _ => false,
    }
}
/// Added in T3 0.1; servers decode a missing value as null.
fn optional(key: &str) -> bool {
    key == "worktreeSettledAfterDays"
}
pub fn is_key(key: &str) -> bool {
    key == "worktreesDirectory" || key == "worktreeCleanup" || key.starts_with("storageCleanup.")
}
pub fn capability(key: &str) -> &'static str {
    if key == "worktreesDirectory" {
        "worktreesDirectory"
    } else {
        "storageCleanup"
    }
}
pub fn policy(value: &Value, partial: bool) -> Result<Value> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let object = value.as_object().ok_or("invalid_settings")?;
    if object.keys().any(|k| k != "mode" && k != "rules") {
        return Err("invalid_settings".into());
    }
    match text(value, "mode") {
        "off" if !object.contains_key("rules") => Ok(json!({"mode":"off"})),
        "custom" => {
            let rules = value["rules"].as_object().ok_or("invalid_settings")?;
            if rules
                .iter()
                .any(|(k, v)| !RULES.contains(&k.as_str()) || !valid(k, v))
                || !partial
                    && RULES
                        .iter()
                        .any(|k| !rules.contains_key(*k) && !optional(k))
            {
                return Err("invalid_settings".into());
            }
            Ok(json!({"mode":"custom","rules":rules}))
        }
        _ => Err("invalid_settings".into()),
    }
}
/// Port of original resolveWorktreeCleanup, applied after native scope folding.
pub fn resolved_rules(settings: &Value) -> Result<Value> {
    let mut rules = json!({});
    for key in RULES {
        let v = match settings["storageCleanup"].get(*key) {
            Some(v) => v,
            None if optional(key) => continue,
            None => return Err("invalid_settings".into()),
        };
        if !valid(key, v) {
            return Err("invalid_settings".into());
        }
        rules[*key] = v.clone();
    }
    match policy(
        settings.get("worktreeCleanup").unwrap_or(&Value::Null),
        false,
    )? {
        v if v["mode"] == "custom" => Ok(v["rules"].clone()),
        v if v["mode"] == "off" => {
            rules["worktreeAfterDays"] = Value::Null;
            rules["worktreeSettledAfterDays"] = Value::Null;
            for key in ["worktreeOnMerge", "worktreeOnDelete", "worktreeUnchanged"] {
                rules[key] = json!(false);
            }
            Ok(rules)
        }
        _ => Ok(rules),
    }
}
pub fn patch(input: &Value) -> Result<Value> {
    let key = text(input, "key");
    let value = input.get("value").ok_or("invalid_settings")?;
    if key == "worktreesDirectory" {
        let path = value
            .as_str()
            .filter(|s| !s.chars().any(char::is_control))
            .ok_or("invalid_settings")?;
        return Ok(json!({"patch":{"worktreesDirectory":path.trim()}}));
    }
    if key == "worktreeCleanup" {
        return Ok(json!({"patch":{"worktreeCleanup":policy(value,true)?}}));
    }
    let member = key
        .strip_prefix("storageCleanup.")
        .ok_or("invalid_settings")?;
    if !valid(member, value) {
        return Err("invalid_settings".into());
    }
    Ok(json!({"patch":{"storageCleanup":{member:value}}}))
}
/// Original scopedSettings environment-write completion: a legacy root policy
/// must be updated alongside worktree members of storageCleanup.
pub fn environment_patch(raw: &Value, key: &str, patch: &Value) -> Result<Value> {
    let mut patch = patch.clone();
    if let Some(member) = key
        .strip_prefix("storageCleanup.")
        .filter(|k| RULES.contains(k))
    {
        if raw.get("worktreeCleanup").is_some_and(|v| !v.is_null()) {
            let mut rules = resolved_rules(raw)?;
            rules[member] = patch["patch"]["storageCleanup"][member].clone();
            patch["patch"]["worktreeCleanup"] = json!({"mode":"custom","rules":rules});
        }
    }
    Ok(patch)
}
pub fn fields(raw: &Value) -> Vec<Value> {
    let mut fields = Vec::new();
    for (key, label) in [
        ("worktreeOnDelete", "Delete worktrees with deleted threads"),
        ("worktreeAfterDays", "Delete inactive worktrees"),
        ("worktreeSettledAfterDays", "Delete settled worktrees"),
        ("worktreeOnMerge", "Delete merged worktrees"),
        ("worktreeUnchanged", "Delete unchanged worktrees"),
        ("worktreeKeepWhen", "Keep worktrees with local changes"),
        ("browserArtifactsAfterDays", "Delete old browser artifacts"),
        ("logsAfterDays", "Delete old rotated logs"),
    ] {
        let effective = if RULES.contains(&key) {
            resolved_rules(raw).ok()
        } else {
            None
        };
        let value = if RULES.contains(&key) {
            effective.as_ref().and_then(|r| r.get(key))
        } else {
            raw["storageCleanup"].get(key)
        };
        // Only servers that know the setting report it.
        if optional(key) && raw["storageCleanup"].get(key).is_none() {
            continue;
        }
        if let Some(v) = value.filter(|v| valid(key, v)) {
            let mut f = json!({"key":format!("storageCleanup.{key}"),"label":label,"section":"storage","value":v,"capability":"storageCleanup","kind":if key.ends_with("Days"){"days"}else if key=="worktreeKeepWhen"{"choice"}else{"bool"}});
            if key.ends_with("Days") {
                f["minimum"] = json!(if optional(key) { 0 } else { 1 });
                f["maximum"] = json!(3650);
                f["enabledValue"] = json!(8);
            }
            if key == "worktreeKeepWhen" {
                f["capability"] = json!("storageCleanupRun");
                f["choices"] = json!([{"id":"any-local-files","name":"Any local files"},{"id":"uncommitted-changes","name":"Uncommitted changes"},{"id":"tracked-changes","name":"Edited tracked files"}]);
            }
            fields.push(f);
        }
    }
    if let Some(path) = raw["worktreesDirectory"].as_str() {
        fields.push(json!({"key":"worktreesDirectory","label":"Worktree location","section":"storage","kind":"text","value":path,"resetValue":"","capability":"worktreesDirectory"}));
    }
    if let Some(p) = raw
        .get("worktreeCleanup")
        .and_then(|v| policy(v, false).ok())
    {
        if let Ok(rules) = resolved_rules(raw) {
            fields.push(json!({"key":"worktreeCleanup","label":"Automatic worktree cleanup","section":"storage","kind":"worktree-policy","value":p,"rules":rules,"capability":"projectWorktreeCleanup"}));
        }
    }
    fields
}

/// Safe flags for native scope notices; never expose complete server config.
pub fn capabilities(config: &Value) -> Value {
    let mut flags = json!({});
    for key in [
        "storageCleanup",
        "storageCleanupRun",
        "worktreesDirectory",
        "projectWorktreeCleanup",
    ] {
        flags[key] = json!(config["environment"]["capabilities"][key] == true);
    }
    flags
}
