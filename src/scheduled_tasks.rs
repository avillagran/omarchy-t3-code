//! T3 scheduled tasks (scheduledTasks.*): list, create/edit, enable, run now, delete.
use crate::{json, protocol::api::scheduled_tasks::{self as tasks, TaskUpsert}, text, Environment, Result};
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
        "webhook" => {
            // Keep advanced webhook options when editing. The protocol reader validates
            // the signature and omits an absent secret, preserving the stored one.
            let mut out = s.clone();
            if out.get("signature").is_none() { out["signature"] = Value::Null; }
            Ok(out)
        },
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
    let strategy=input.get("workspaceStrategy").cloned().unwrap_or_else(||if input["worktree"]==true{json!({"type":"worktree","baseRef":input["baseRef"]})}else{json!({"type":"root"})});
    let workspace=crate::protocol::api::orchestration::WorkspaceStrategy::from_json(&strategy)?.to_json()?;
    let mode = text(input, "runtimeMode");
    let mut p = json!({"title":title,"prompt":prompt,"enabled":input["enabled"]!=false,"schedule":schedule(input)?,"projectId":project,"threadId":input.get("threadId").unwrap_or(&Value::Null),"workspaceStrategy":workspace,"modelSelection":input["modelSelection"],"runtimeMode":if mode.is_empty(){"approval-required"}else{mode},"interactionMode":input["interactionMode"].as_str().unwrap_or("default"),"createdBy":"user","creationSource":"web"});
    if input["id"].is_string() {
        p["id"] = json!(id(input)?);
        p["requireExisting"] = json!(true);
    } else {
        // T3 names a new task after its commandId (scheduled-task:<commandId>).
        p["commandId"] = json!(format!("omarchy-task-{}", uuid::Uuid::new_v4()));
    }
    Ok(p)
}
pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    let action = text(input, "action");
    let c = || e.connection(true);
    let result = match action {
        "list" => tasks::list(&c()?)?,
        "upsert" => {
            let task = TaskUpsert::from_json(&upsert_payload(input)?)?;
            // A rejected definition can contain the submitted secret in a server
            // diagnostic. Never forward that diagnostic to the UI or logs.
            tasks::upsert(&c()?, &task).map_err(|_| "scheduled_task_definition_rejected")?
        },
        "enable" => tasks::set_enabled(&c()?, &id(input)?, input["enabled"] == true)?,
        "delete" => tasks::delete(&c()?, &id(input)?)?,
        "run" => tasks::run_now(&c()?, &id(input)?)?,
        _ => return Err("invalid_command".into()),
    };
    // Every change answers with the fresh list, so the page never shows a stale row.
    let list = if action == "list" { result } else { tasks::list(&c()?)? };
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
    #[test]
    fn webhook_signatures_follow_t3_write_and_edit_contract() {
        let mut input = json!({"title":"Hook","prompt":"{{body.action}}","projectId":"p","modelSelection":{"instanceId":"codex","model":"m"},"schedule":{"type":"webhook","signature":{"header":"x-signature","encoding":"base64","prefix":"","secret":"fixture-signing-value"},"filters":[],"maxDeliveryAgeMinutes":90}});
        let payload = TaskUpsert::from_json(&upsert_payload(&input).unwrap()).unwrap().payload().unwrap();
        assert_eq!(payload["schedule"], input["schedule"]);
        input["schedule"]["signature"].as_object_mut().unwrap().remove("secret");
        let payload = TaskUpsert::from_json(&upsert_payload(&input).unwrap()).unwrap().payload().unwrap();
        assert!(payload["schedule"]["signature"].get("secret").is_none());
        assert_eq!(payload["schedule"]["maxDeliveryAgeMinutes"], 90);
        input["schedule"]["signature"]["encoding"] = json!("invalid");
        assert!(TaskUpsert::from_json(&upsert_payload(&input).unwrap()).unwrap().payload().is_err());
        input["schedule"] = json!({"type":"webhook"});
        assert_eq!(upsert_payload(&input).unwrap()["schedule"]["signature"], Value::Null);
    }
    #[test]
    fn task_edits_preserve_workspace_and_require_a_worktree_base() {
        let mut input=json!({"title":"Daily","prompt":"Review","projectId":"p","modelSelection":{"instanceId":"codex","model":"m"},"schedule":{"type":"interval","minutes":60},"workspaceStrategy":{"type":"worktree","baseRef":"develop","startFromOrigin":true},"interactionMode":"plan","threadId":"t"});
        let p=upsert_payload(&input).unwrap();assert_eq!(p["workspaceStrategy"],input["workspaceStrategy"]);assert_eq!(p["interactionMode"],"plan");assert_eq!(p["threadId"],"t");
        input["workspaceStrategy"]=json!({"type":"worktree","baseRef":""});assert!(upsert_payload(&input).is_err());
    }

}
