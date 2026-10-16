//! Allowlisted native preferences shared with Omi. Never exports credentials.
use serde_json::{json, Value};

const BOOLS: &[(&str, &str, &str)] = &[
    ("enableProviderUpdateChecks", "Check provider updates", "general"),
    ("continueThreadsAfterServerUpdate", "Resume after server updates", "general"),
    ("sidebarAutoSettleKeepPullRequests", "Keep threads with pull requests", "general"),
    ("sidebarAutoSettleOnMerge", "Settle merged pull requests", "general"),
    ("snoozeLimitedThreads", "Snooze rate-limited threads", "general"),
    ("autoResumeLimitedThreads", "Resume rate-limited threads automatically", "general"),
    ("defaultAutoPull", "Pull changes automatically", "source-control"),
    ("newWorktreesStartFromOrigin", "Start new worktrees from origin", "source-control"),
    ("removeAgentCreditsOnMerge", "Remove agent credits on merge", "source-control"),
    ("reviewGuidesEnabled", "Enable review guides", "source-control"),
    ("sourceControlWritingStyle.followChangeRequestTemplates", "Follow pull request templates", "source-control"),
];
const TEXTS: &[(&str, &str, usize)] = &[
    ("branchNamePrefix", "Branch name prefix", 256),
    ("branchNameInstructions", "Branch naming instructions", 16384),
    ("sourceControlWritingStyle.customInstructions", "Writing instructions", 16384),
    ("reviewGuideInstructions", "Review guide instructions", 16384),
];
fn value<'a>(raw: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(raw, |v, part| v.get(part))
}
fn choices(key: &str) -> Option<(&'static str, &'static str, Value)> {
    Some(match key {
        "responseStreamingMode" => ("Response streaming", "general", json!([
            {"id":"turn","name":"Turn"},{"id":"paragraph","name":"Paragraph"}])),
        "backgroundActivityProfile" => ("Background activity", "general", json!([
            {"id":"balanced","name":"Balanced"},{"id":"performance","name":"Performance"},
            {"id":"battery-saver","name":"Battery saver"},{"id":"custom","name":"Custom"}])),
        "branchNamingMode" => ("Branch naming", "source-control", json!([
            {"id":"static","name":"Static"},{"id":"semantic","name":"Semantic"},{"id":"custom","name":"Custom"}])),
        "sourceControlWritingStyle.mode" => ("Writing style", "source-control", json!([
            {"id":"repo_conventions","name":"Repository conventions"},
            {"id":"conventional_commits","name":"Conventional commits"},{"id":"custom","name":"Custom"}])),
        "pullRequestMergeMethod" => ("Pull request merge method", "source-control", json!([
            {"id":null,"name":"Last used"},{"id":"merge","name":"Merge"},
            {"id":"squash","name":"Squash"},{"id":"rebase","name":"Rebase"}])),
        "worktreeSubmodules" => ("Worktree submodules", "source-control", json!([
            {"id":null,"name":"Inherit"},{"id":"recursive","name":"Recursive"},
            {"id":"top-level","name":"Top level"},{"id":"none","name":"None"}])),
        _ => return None,
    })
}
const CHOICES: &[&str] = &[
    "responseStreamingMode", "backgroundActivityProfile", "branchNamingMode",
    "sourceControlWritingStyle.mode", "pullRequestMergeMethod", "worktreeSubmodules",
];
pub fn snapshot(raw: &Value) -> Value {
    let mut fields = Vec::new();
    for (key, label, section) in BOOLS {
        if let Some(value) = value(raw, key).and_then(Value::as_bool) {
            fields.push(json!({"key":key,"label":label,"section":section,"kind":"bool","value":value}));
        }
    }
    for key in CHOICES {
        let v = if *key == "backgroundActivityProfile" {
            value(raw, "backgroundActivity.profile").or_else(|| value(raw, key))
        } else { value(raw, key) };
        let (label, section, choices) = choices(key).unwrap();
        if let Some(v) = v.filter(|v| choices.as_array().unwrap().iter().any(|c| c["id"] == **v)) {
            fields.push(json!({"key":key,"label":label,"section":section,"kind":"choice","value":v,"choices":choices}));
        }
    }
    if let Some(v) = value(raw, "sidebarAutoSettleAfterDays").filter(|v| valid_days(v)) {
        fields.push(json!({"key":"sidebarAutoSettleAfterDays","label":"Auto-settle after inactivity (days)","section":"general","kind":"days","value":v,"minimum":1,"maximum":90}));
    }
    for (key, label, limit) in TEXTS {
        if let Some(v) = value(raw, key).and_then(Value::as_str) {
            fields.push(json!({"key":key,"label":label,"section":"source-control","kind":"text","value":v,"multiline":*limit>256,"maximumLength":limit}));
        }
    }
    json!({"fields":fields})
}
fn valid_days(v: &Value) -> bool {
    v.is_null() || v.as_u64().is_some_and(|n| (1..=90).contains(&n))
}
pub fn patch(input: &Value) -> Result<Value, String> {
    let key = input["key"].as_str().ok_or("invalid_settings")?;
    let v = input.get("value").ok_or("invalid_settings")?;
    let valid = if BOOLS.iter().any(|(k, _, _)| *k == key) {
        v.is_boolean()
    } else if let Some((_, _, options)) = choices(key) {
        options.as_array().unwrap().iter().any(|c| c["id"] == *v)
    } else if key == "sidebarAutoSettleAfterDays" {
        valid_days(v)
    } else if let Some((_, _, limit)) = TEXTS.iter().find(|(k, _, _)| *k == key) {
        v.as_str().is_some_and(|s| s.chars().count() <= *limit && !s.contains('\0'))
    } else { false };
    if !valid { return Err("invalid_settings".into()); }
    let v = if let Some(text) = v.as_str() { json!(text.trim()) } else { v.clone() };
    let mut patch = json!({});
    if key == "backgroundActivityProfile" {
        patch["backgroundActivity"] = json!({"schemaVersion":1,"profile":v});
    } else if let Some((parent, child)) = key.split_once('.') {
        // Native settings merge these members; never overwrite their siblings.
        patch[parent] = json!({child:v});
    } else {
        patch[key] = v;
    }
    Ok(json!({"patch":patch}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_snapshot_omits_secrets_and_unknown_choices() {
        let s = snapshot(&json!({"enableProviderUpdateChecks":true,"providerInstances":{"secret":"do-not-export"},"responseStreamingMode":"paragraph","branchNamingMode":"future-mode","sourceControlWritingStyle":{"mode":"custom","customInstructions":"short"},"worktreeSubmodules":null}));
        assert_eq!(s["fields"].as_array().unwrap().len(),5);
        assert!(!s.to_string().contains("do-not-export"));
        assert!(!s.to_string().contains("future-mode"));
    }
    #[test]
    fn patches_are_typed_bounded_and_do_not_replace_nested_siblings() {
        assert_eq!(patch(&json!({"key":"sourceControlWritingStyle.mode","value":"custom"})).unwrap(),json!({"patch":{"sourceControlWritingStyle":{"mode":"custom"}}}));
        assert_eq!(patch(&json!({"key":"backgroundActivityProfile","value":"battery-saver"})).unwrap(),json!({"patch":{"backgroundActivity":{"schemaVersion":1,"profile":"battery-saver"}}}));
        assert_eq!(patch(&json!({"key":"branchNamePrefix","value":" topic "})).unwrap(),json!({"patch":{"branchNamePrefix":"topic"}}));
        for invalid in [json!({"key":"providerInstances","value":{}}),json!({"key":"sourceControlWritingStyle","value":{}}),json!({"key":"enableProviderUpdateChecks","value":"true"}),json!({"key":"pullRequestMergeMethod","value":"delete"}),json!({"key":"branchNamePrefix","value":"x".repeat(257)}),json!({"key":"branchNameInstructions","value":"bad\0value"}),json!({"key":"worktreeSubmodules"})] { assert!(patch(&invalid).is_err(),"{invalid}"); }
    }
    #[test]
    fn nullable_preferences_preserve_off_and_inherit() {
        for v in [json!(null),json!(1),json!(90)] {
            assert_eq!(patch(&json!({"key":"sidebarAutoSettleAfterDays","value":v})).unwrap()["patch"]["sidebarAutoSettleAfterDays"],v);
        }
        for v in [json!(0),json!(-1),json!(91),json!(1.5),json!("3")] {
            assert!(patch(&json!({"key":"sidebarAutoSettleAfterDays","value":v})).is_err());
        }
        for key in ["pullRequestMergeMethod","worktreeSubmodules"] {
            assert!(patch(&json!({"key":key,"value":null})).unwrap()["patch"][key].is_null());
        }
    }
}
