//! Import Claude and Codex sessions found on the environment into T3 projects
//! (agentSessions.scan / agentSessions.import), as in T3's import screen.
use crate::{json, protocol::api::{agent_sessions, projects::{self, ProjectMutation}}, text, Environment, Result};
use serde_json::Value;

pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    match text(input, "action") {
        "scan" => {
            let r = agent_sessions::scan(&e.connection(true)?)?;
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
                    // T3's ProjectMutation requires a commandId; the import used to send none.
                    let create = ProjectMutation::Create { command_id: format!("omarchy-{}", uuid::Uuid::new_v4()), project_id: id.clone(), title: title.into(), workspace_root: path.into(), create_workspace_root_if_missing: Some(false), default_model_selection: None, scripts: None };
                    projects::mutate(&e.connection(true)?, &create)?;
                    id
                }
            };
            let r = agent_sessions::import(&e.connection(true)?, &project, Some(path))?;
            Ok(json!({"projectId":project,"importedCount":r["importedCount"],"skippedCount":r["skippedCount"],"path":path}))
        }
        _ => Err("invalid_command".into()),
    }
}
