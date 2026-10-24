//! Private desktop presentation state; never stores transcripts, drafts or credentials.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{fs, io::Read, os::unix::fs::PermissionsExt};
pub fn sanitize(input: &Value) -> Value {
    let mut out = json!({"schemaVersion":1,"positions":{}});
    for key in [
        "autoCompactEnabled",
        "quotaCollapsed",
        "usageExpanded",
        "queueExpanded",
        "tasksExpanded",
        "settledExpanded","snoozedExpanded","sidebarCompact","workingExpanded",
        "sidebarCollapsed",
        "english",
        "composerExpanded","detached",
        "screenshotButton","screenshotHidePanel","dictationButton",
        "fpsMeter",
        "omabridgeIndicator",
        "threadDetailsOpen",
    ] {
        if let Some(v) = input[key].as_bool() {
            out[key] = json!(v)
        }
    }
    for (key,min,max) in [("autoCompactFallbackMinutes",1,1440),("autoCompactMinTokens",1000,1000000)] {if let Some(n)=input[key].as_u64().filter(|n|*n>=min&&*n<=max){out[key]=json!(n)}}
    if let Some(map)=input["autoCompactProviderMinutes"].as_object(){let mut clean=serde_json::Map::new();for (key,value) in map.iter().take(64){if valid_id(key)||key.split_once('/').is_some_and(|(e,p)|valid_id(e)&&valid_id(p)){if let Some(n)=value.as_u64().filter(|n|*n>=1&&*n<=1440){clean.insert(key.clone(),json!(n));}}}out["autoCompactProviderMinutes"]=Value::Object(clean);}
    if let Some(list)=input["autoCompactExcludedThreads"].as_array(){out["autoCompactExcludedThreads"]=json!(list.iter().filter(|v|v.as_str().is_some_and(|s|s.split_once('/').is_some_and(|(e,t)|valid_id(e)&&valid_id(t)))).take(1024).collect::<Vec<_>>());}
    // Provider update notices the user closed: "env=driver:version|…" keys.
    if let Some(list)=input["providerUpdateDismissals"].as_array(){out["providerUpdateDismissals"]=json!(list.iter().filter(|v|v.as_str().is_some_and(|s|!s.is_empty()&&s.len()<=2048&&!s.chars().any(char::is_control))).take(32).collect::<Vec<_>>());}
    // Low disk space warning threshold in GB; null switches the warning off.
    match input.get("lowDiskThresholdGb"){Some(Value::Null)=>{out["lowDiskThresholdGb"]=Value::Null;},Some(v)=>{if let Some(n)=v.as_u64().filter(|n|(1..=1000).contains(n)){out["lowDiskThresholdGb"]=json!(n);}},None=>{}}
    // Automatic build output cleanup after this many idle days; null switches it off.
    for k in ["buildCleanupWorktreeDays","buildCleanupCheckoutDays"]{match input.get(k){Some(Value::Null)=>{out[k]=Value::Null;},Some(v)=>{if let Some(n)=v.as_u64().filter(|n|(1..=365).contains(n)){out[k]=json!(n);}},None=>{}}}
    for k in ["threadDetailsWidth","threadDetailsHeight"]{if let Some(n)=input[k].as_f64().filter(|n|n.is_finite()&&*n>=0.0&&*n<=4000.0){out[k]=json!(n.round() as i64);}}
    if let Some(v)=input["preferredEditor"].as_str().filter(|v|!v.is_empty()&&v.len()<=64&&v.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_')){out["preferredEditor"]=json!(v);}
    // Sub-threads the user expanded or collapsed under a parent thread ("env/thread").
    if let Some(map)=input["subthreadsExpanded"].as_object(){let mut clean=serde_json::Map::new();for(k,v)in map.iter().take(512){if k.split_once('/').is_some_and(|(e,t)|valid_id(e)&&valid_id(t)){if let Some(b)=v.as_bool(){clean.insert(k.clone(),json!(b));}}}out["subthreadsExpanded"]=Value::Object(clean);}
    if let Some(map)=input["sidebarProjectGroupingOverrides"].as_object(){let mut clean=serde_json::Map::new();for(k,v)in map.iter().take(512){if k.len()<=2200&&!k.chars().any(char::is_control)&&matches!(v.as_str(),Some("inherit"|"repository"|"repository_path"|"separate")){clean.insert(k.clone(),v.clone());}}out["sidebarProjectGroupingOverrides"]=Value::Object(clean);}
    if matches!(input["sidebarProjectGroupingMode"].as_str(),Some("repository"|"repository_path"|"separate")){out["sidebarProjectGroupingMode"]=input["sidebarProjectGroupingMode"].clone();}
    if matches!(input["starfieldMode"].as_str(),Some("auto"|"full"|"light"|"off")){out["starfieldMode"]=input["starfieldMode"].clone();}
    for (key,allowed) in [("starfieldSpeed",&["slow","normal","fast"][..]),("starfieldMeteors",&["none","few","normal","many"][..]),("threadWorkingEffect",&["comet","pulse","off"][..]),("threadInputEffect",&["glints","pulse","static","off"][..]),("threadChangeEffect",&["star","flash","off"][..]),("threadEffectIntensity",&["subtle","normal","vivid"][..]),("threadMoveEffect",&["arc","glide","fade","off"][..]),("threadMoveSpeed",&["slow","normal","fast"][..]),("htmlRenders",&["inline","side"][..]),("animationRate",&["adaptive","saver","max"][..]),("barIndicator",&["orbit","scanner","equalizer","heartbeat","ticks","logo"][..]),("threadNotifications",&["notifications","notifications-and-sound","sound","off"][..]),("transitionStyle",&["nebula","warp","fold","spring","instant"][..]),("transitionSpeed",&["slow","normal","fast"][..])] {
        if input[key].as_str().is_some_and(|v|allowed.contains(&v)){out[key]=input[key].clone();}
    }
    if matches!(input["windowMode"].as_str(),Some("panel"|"floating"|"tiled")){out["windowMode"]=input["windowMode"].clone();}
    if matches!(input["dictationModel"].as_str(),Some("base"|"small")){out["dictationModel"]=input["dictationModel"].clone();}
    if input["dictationLanguage"].as_str().is_some_and(|v|v=="auto"||(matches!(v.len(),2|3)&&v.chars().all(|c|c.is_ascii_lowercase()))){out["dictationLanguage"]=input["dictationLanguage"].clone();}
    for key in ["workingShelfHeight","snoozedShelfHeight","settledShelfHeight"] {
        if let Some(value)=input[key].as_f64().filter(|n|n.is_finite() && *n>=60.0 && *n<=1200.0){out[key]=json!(value);}
    }
    for key in ["panelWidth","panelHeight","sidebarWidth","browserPanelWidth","renderPanelWidth","windowWidth","windowHeight"] {
        if let Some(value)=input[key].as_f64().filter(|n|n.is_finite() && *n>=100.0 && *n<=10000.0){out[key]=json!(value);}
    }
    for (key, max) in [
        ("selectedEnvironment", 2048),
        ("selectedThread", 2048),
        ("projectFilter", 2200),
        ("query", 256),
        ("language", 32),
        ("screenshotCommand", 4096),
        ("dictationVocabulary", 600),
    ] {
        if let Some(v) = input[key]
            .as_str()
            .filter(|v| v.len() <= max && !v.chars().any(|c| c.is_control()))
        {
            out[key] = json!(v)
        }
    }
    if let Some(v) = input["statusFilter"]
        .as_str()
        .filter(|v| ["all", "working", "done", "failed", "pending", "settled"].contains(v))
    {
        out["statusFilter"] = json!(v)
    }
    if let Some(positions) = input["positions"].as_object() {
        for (key, p) in positions.iter().take(128).filter(|(k, _)| k.len() <= 2200) {
            let Some(id) = p["id"].as_str().filter(|id| id.len() <= 2200) else {
                continue;
            };
            let Some(offset) = p["offset"]
                .as_f64()
                .filter(|n| n.is_finite() && n.abs() <= 1_000_000.0)
            else {
                continue;
            };
            out["positions"][key] = json!({"id":id,"offset":offset,"atEnd":p["atEnd"]==true});
        }
    }
    out
}
fn path() -> std::path::PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("ui-state.json")
}
pub fn load() -> Value {
    let mut bytes = vec![];
    let value = fs::File::open(path())
        .and_then(|f| f.take(1024 * 1024 + 1).read_to_end(&mut bytes))
        .ok()
        .filter(|_| bytes.len() <= 1024 * 1024)
        .and_then(|_| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    sanitize(&value)
}
pub fn save(value: &Value) -> Result<()> {
    let path = path();
    let dir = path.parent().ok_or("local_error")?;
    fs::create_dir_all(dir).map_err(|_| "local_error")?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|_| "local_error")?;
    save_json(&path, &sanitize(value))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_roundtrip_excludes_content_and_bounds_scroll() {
        let root = std::env::temp_dir().join(format!("t3-ui-state-{}", uuid::Uuid::new_v4()));
        std::env::set_var("XDG_STATE_HOME", &root);
        let value = json!({"quotaCollapsed":true,"selectedEnvironment":"local","selectedThread":"thread-one","positions":{"local/thread-one":{"id":"message-1","offset":24.5,"atEnd":false},"bad":{"id":"x","offset":1e15}},"draft":"private draft","token":"private credential","statusFilter":"invalid"});
        save(&value).unwrap();
        let restored = load();
        assert_eq!(restored["quotaCollapsed"], true);
        assert_eq!(restored["selectedThread"], "thread-one");
        assert_eq!(restored["positions"]["local/thread-one"]["offset"], 24.5);
        assert!(restored["positions"].get("bad").is_none());
        assert!(!restored.to_string().contains("private"));
        assert_eq!(
            fs::metadata(path()).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::write(path(), "corrupt").unwrap();
        assert_eq!(load(), json!({"schemaVersion":1,"positions":{}}));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn animation_preferences_keep_known_choices_only() {
        let clean = sanitize(&json!({"starfieldSpeed":"fast","starfieldMeteors":"many","threadWorkingEffect":"pulse","threadInputEffect":"static","threadChangeEffect":"off","threadEffectIntensity":"turbo","transitionStyle":"fold","transitionSpeed":"warp","threadMoveEffect":"glide","threadMoveSpeed":"ludicrous","settledShelfHeight":480,"workingShelfHeight":5}));
        assert_eq!(clean["starfieldSpeed"], "fast");
        assert_eq!(clean["starfieldMeteors"], "many");
        assert_eq!(clean["threadWorkingEffect"], "pulse");
        assert_eq!(clean["threadInputEffect"], "static");
        assert_eq!(clean["threadChangeEffect"], "off");
        assert!(clean.get("threadEffectIntensity").is_none());
        assert_eq!(clean["transitionStyle"], "fold");
        assert!(clean.get("transitionSpeed").is_none());
        assert_eq!(clean["threadMoveEffect"], "glide");
        assert!(clean.get("threadMoveSpeed").is_none());
        assert_eq!(clean["settledShelfHeight"], 480.0);
        assert!(clean.get("workingShelfHeight").is_none());
    }
    #[test]
    fn subthread_expansion_keeps_thread_keys_and_booleans_only() {
        let clean = sanitize(&json!({"subthreadsExpanded":{"local/root":true,"local/other":false,"bad":true,"local/x":"yes"}}));
        assert_eq!(clean["subthreadsExpanded"], json!({"local/root":true,"local/other":false}));
    }
    #[test]
    fn details_and_update_preferences_are_bounded() {
        let clean = sanitize(&json!({"threadDetailsOpen":true,"preferredEditor":"vscode","providerUpdateDismissals":["local=claudeAgent:2.1.307","",3]}));
        assert_eq!(clean["threadDetailsOpen"], true);
        assert_eq!(clean["preferredEditor"], "vscode");
        assert_eq!(clean["providerUpdateDismissals"], json!(["local=claudeAgent:2.1.307"]));
        assert!(sanitize(&json!({"preferredEditor":"rm -rf"})).get("preferredEditor").is_none());
    }
    #[test]
    fn omabridge_indicator_is_an_opt_in_boolean() {
        assert_eq!(sanitize(&json!({"omabridgeIndicator":true}))["omabridgeIndicator"], true);
        assert!(sanitize(&json!({})).get("omabridgeIndicator").is_none());
        assert!(sanitize(&json!({"omabridgeIndicator":"yes"})).get("omabridgeIndicator").is_none());
    }
    #[test]
    fn screenshot_preferences_keep_single_line_commands() {
        let clean = sanitize(&json!({"screenshotButton":false,"screenshotHidePanel":true,"screenshotCommand":"grim -g \"$(slurp)\" {file}"}));
        assert_eq!(clean["screenshotButton"], false);
        assert_eq!(clean["screenshotHidePanel"], true);
        assert_eq!(clean["screenshotCommand"], "grim -g \"$(slurp)\" {file}");
        assert!(sanitize(&json!({"screenshotCommand":"grim\nrm -rf ~"})).get("screenshotCommand").is_none());
        assert!(sanitize(&json!({"screenshotCommand":"x".repeat(4097)})).get("screenshotCommand").is_none());
    }
    #[test]
    fn dictation_preferences_reject_arguments_and_unknown_models() {
        let dictation = sanitize(&json!({"dictationButton":false,"dictationModel":"small","dictationLanguage":"es","dictationVocabulary":"Omarchy, Hyprland"}));
        assert_eq!((dictation["dictationButton"].clone(), dictation["dictationModel"].clone(), dictation["dictationLanguage"].clone(), dictation["dictationVocabulary"].clone()), (json!(false), json!("small"), json!("es"), json!("Omarchy, Hyprland")));
        let invalid = sanitize(&json!({"dictationModel":"../model","dictationLanguage":"--prompt","dictationVocabulary":"a\nb"}));
        assert!(invalid.get("dictationModel").is_none() && invalid.get("dictationLanguage").is_none() && invalid.get("dictationVocabulary").is_none());
    }
}
