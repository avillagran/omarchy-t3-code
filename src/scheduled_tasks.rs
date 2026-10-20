//! T3 scheduled tasks (scheduledTasks.*): list, create/edit, enable, run now, delete.
use crate::{json, text, write::rpc, Environment, Result};
use serde_json::Value;

fn id(input: &Value) -> Result<String> {
    input["id"].as_str().filter(|s| !s.is_empty() && s.len() <= 256).map(str::to_string).ok_or("invalid_id".into())
}
pub fn schedule(input: &Value) -> Result<Value> {
    let s = &input["schedule"];
    match text(s, "type") {
        "interval" => {
            let minutes = s["minutes"].as_u64().filter(|m| *m >= 1 && *m <= 60 * 24 * 365).ok_or("invalid_schedule")?;
            Ok(json!({"type":"interval","everyMs":minutes*60_000}))
        }
        "fixed_time" => {
            let time = text(s, "timeOfDay");
            let valid = time.len() == 5
                && time.as_bytes()[2] == b':'
                && time[..2].parse::<u8>().is_ok_and(|h| h < 24)
                && time[3..].parse::<u8>().is_ok_and(|m| m < 60);
            if !valid {
                return Err("invalid_schedule".into());
            }
            let mut out = json!({"type":"fixed_time","timeOfDay":time});
            if let Some(days) = s["weekdays"].as_array() {
                let days: Vec<u64> = days.iter().filter_map(Value::as_u64).filter(|d| *d <= 6).collect();
                if !days.is_empty() && days.len() < 7 {
                    out["weekdays"] = json!(days);
                }
            }
            Ok(out)
        }
        "webhook" => Ok(json!({"type":"webhook","signature":null})),
        _ => Err("invalid_schedule".into()),
    }
}
pub fn upsert_payload(input: &Value) -> Result<Value> {
    let title = text(input, "title").trim();
    let prompt = text(input, "prompt").trim();
    if title.is_empty() || title.chars().count() > 200 || prompt.is_empty() || prompt.len() > 64 * 1024 {
        return Err("invalid_task".into());
    }
    let project = text(input, "projectId");
    if !crate::valid_id(project) || !input["modelSelection"].is_object() {
        return Err("invalid_project".into());
    }
    let mode = text(input, "runtimeMode");
    let mut p = json!({"title":title,"prompt":prompt,"enabled":input["enabled"]!=false,"schedule":schedule(input)?,"projectId":project,"threadId":null,"workspaceStrategy":{"type":if input["worktree"]==true{"worktree"}else{"root"}},"modelSelection":input["modelSelection"],"runtimeMode":if mode.is_empty(){"approval-required"}else{mode},"interactionMode":"default","createdBy":"user","creationSource":"web"});
    if input["id"].is_string() {
        p["id"] = json!(id(input)?);
        p["requireExisting"] = json!(true);
    }
    Ok(p)
}
pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    let action = text(input, "action");
    let result = match action {
        "list" => rpc(e, "scheduledTasks.list", json!({}))?,
        "upsert" => rpc(e, "scheduledTasks.upsert", upsert_payload(input)?)?,
        "enable" => rpc(e, "scheduledTasks.setEnabled", json!({"id":id(input)?,"enabled":input["enabled"]==true}))?,
        "delete" => rpc(e, "scheduledTasks.delete", json!({"id":id(input)?}))?,
        "run" => rpc(e, "scheduledTasks.runNow", json!({"id":id(input)?}))?,
        _ => return Err("invalid_command".into()),
    };
    // Every change answers with the fresh list, so the page never shows a stale row.
    let list = if action == "list" { result } else { rpc(e, "scheduledTasks.list", json!({}))? };
    Ok(json!({"action":action,"tasks":list["tasks"]}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schedules_and_payloads_follow_the_contract() {
        assert_eq!(schedule(&json!({"schedule":{"type":"interval","minutes":30}})).unwrap(), json!({"type":"interval","everyMs":1_800_000}));
        assert!(schedule(&json!({"schedule":{"type":"interval","minutes":0}})).is_err());
        assert_eq!(schedule(&json!({"schedule":{"type":"fixed_time","timeOfDay":"09:30","weekdays":[1,2,3,4,5]}})).unwrap()["weekdays"], json!([1,2,3,4,5]));
        assert!(schedule(&json!({"schedule":{"type":"fixed_time","timeOfDay":"25:00"}})).is_err());
        assert!(schedule(&json!({"schedule":{"type":"fixed_time","timeOfDay":"09:00","weekdays":[0,1,2,3,4,5,6]}})).unwrap().get("weekdays").is_none());
        let p = upsert_payload(&json!({"title":" Daily ","prompt":"Check CI","projectId":"p","modelSelection":{"instanceId":"codex","model":"m"},"schedule":{"type":"interval","minutes":60}})).unwrap();
        assert_eq!((p["title"].as_str(), p["workspaceStrategy"]["type"].as_str(), p["enabled"].as_bool()), (Some("Daily"), Some("root"), Some(true)));
        assert!(p.get("id").is_none());
        let p = upsert_payload(&json!({"id":"task-1","title":"x","prompt":"y","projectId":"p","modelSelection":{},"schedule":{"type":"webhook"}})).unwrap();
        assert_eq!(p["requireExisting"], true);
        assert!(upsert_payload(&json!({"title":"","prompt":"y","projectId":"p","modelSelection":{},"schedule":{"type":"webhook"}})).is_err());
    }
}
