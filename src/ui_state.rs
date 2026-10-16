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
    ] {
        if let Some(v) = input[key].as_bool() {
            out[key] = json!(v)
        }
    }
    for (key,min,max) in [("autoCompactFallbackMinutes",1,1440),("autoCompactMinTokens",1000,1000000)] {if let Some(n)=input[key].as_u64().filter(|n|*n>=min&&*n<=max){out[key]=json!(n)}}
    if let Some(map)=input["autoCompactProviderMinutes"].as_object(){let mut clean=serde_json::Map::new();for (key,value) in map.iter().take(64){if valid_id(key)||key.split_once('/').is_some_and(|(e,p)|valid_id(e)&&valid_id(p)){if let Some(n)=value.as_u64().filter(|n|*n>=1&&*n<=1440){clean.insert(key.clone(),json!(n));}}}out["autoCompactProviderMinutes"]=Value::Object(clean);}
    if let Some(list)=input["autoCompactExcludedThreads"].as_array(){out["autoCompactExcludedThreads"]=json!(list.iter().filter(|v|v.as_str().is_some_and(|s|s.split_once('/').is_some_and(|(e,t)|valid_id(e)&&valid_id(t)))).take(1024).collect::<Vec<_>>());}
    if let Some(map)=input["sidebarProjectGroupingOverrides"].as_object(){let mut clean=serde_json::Map::new();for(k,v)in map.iter().take(512){if k.len()<=2200&&!k.chars().any(char::is_control)&&matches!(v.as_str(),Some("inherit"|"repository"|"repository_path"|"separate")){clean.insert(k.clone(),v.clone());}}out["sidebarProjectGroupingOverrides"]=Value::Object(clean);}
    if matches!(input["sidebarProjectGroupingMode"].as_str(),Some("repository"|"repository_path"|"separate")){out["sidebarProjectGroupingMode"]=input["sidebarProjectGroupingMode"].clone();}
    for key in ["panelWidth","panelHeight","sidebarWidth","windowWidth","windowHeight"] {
        if let Some(value)=input[key].as_f64().filter(|n|n.is_finite() && *n>=100.0 && *n<=10000.0){out[key]=json!(value);}
    }
    for (key, max) in [
        ("selectedEnvironment", 2048),
        ("selectedThread", 2048),
        ("projectFilter", 2200),
        ("query", 256),
        ("language", 32),
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
}
