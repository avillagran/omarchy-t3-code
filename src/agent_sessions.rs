//! Import Claude and Codex sessions found on the environment into T3 projects
//! (agentSessions.scan / agentSessions.import), as in T3's import screen.
use crate::{json, text, write::rpc, Environment, Result};
use serde_json::Value;

pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    match text(input, "action") {
        "scan" => {
            let r = rpc(e, "agentSessions.scan", json!({}))?;
            Ok(json!({"candidates":r["candidates"],"truncated":r["truncated"]==true}))
        }
        "import" => {
            let path = input["path"].as_str().filter(|p| p.starts_with('/') && p.len() <= 4096 && !p.chars().any(char::is_control)).ok_or("invalid_folder")?;
            let project = match input["projectId"].as_str().filter(|p| crate::valid_id(p)) {
                Some(p) => p.to_string(),
                None => {
                    // A folder without a project becomes one first, like Add project.
                    let id = format!("omarchy-project-{}", uuid::Uuid::new_v4());
                    let title = path.trim_end_matches('/').rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("Project");
                    rpc(e, "projects.mutate", json!({"type":"project.create","projectId":id,"title":title,"workspaceRoot":path,"createWorkspaceRootIfMissing":false}))?;
                    id
                }
            };
            let r = rpc(e, "agentSessions.import", json!({"projectId":project,"expectedWorkspaceRoot":path}))?;
            Ok(json!({"projectId":project,"importedCount":r["importedCount"],"skippedCount":r["skippedCount"],"path":path}))
        }
        _ => Err("invalid_command".into()),
    }
}
