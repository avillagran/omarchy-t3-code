//! Thread details (T3 ThreadDetailsPanel): the editors that can open the open
//! thread's workspace, and opening it there through shell.openInEditor.
//! The folder always comes from the server's own thread and project records,
//! never from the panel.
use crate::*;
use serde_json::{json, Value};

/// Native editor ids and labels, in T3's order, limited to those the server found installed.
pub fn editors(config: &Value) -> Vec<Value> {
    let native: Vec<Value> = serde_json::from_str(include_str!("native_editors.json")).unwrap_or_default();
    let available = config["availableEditors"].as_array().cloned().unwrap_or_default();
    native.into_iter().filter(|e| available.iter().any(|id| id == &e["id"])).collect()
}

/// The thread's worktree, else its project's workspace root (T3 gitCwd).
pub fn workspace(shell: &Value, thread: &str) -> Result<String> {
    let t = shell["threads"].as_array().into_iter().flatten().find(|t| t["id"] == thread).ok_or("unknown_thread")?;
    let root = shell["projects"].as_array().into_iter().flatten().find(|p| p["id"] == t["projectId"]).and_then(|p| p["workspaceRoot"].as_str());
    let cwd = t["worktreePath"].as_str().filter(|p| !p.is_empty()).or(root).ok_or("invalid_folder")?;
    if !cwd.starts_with('/') || cwd.len() > 4096 || cwd.chars().any(char::is_control) {
        return Err("invalid_folder".into());
    }
    Ok(cwd.to_string())
}

fn shell(e: &Environment) -> Result<Value> {
    Client::new()?.get(e, "/api/orchestration/shell", true)
}

pub fn read(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !valid_id(thread) {
        return Err("invalid_id".into());
    }
    let config = write::rpc(e, "server.getConfig", json!({}))?;
    let cwd = workspace(&shell(e)?, thread).ok();
    Ok(json!({"environment":e.id,"thread":thread,"editors":editors(&config),"cwd":cwd}))
}

pub fn open(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !valid_id(thread) {
        return Err("invalid_id".into());
    }
    let config = write::rpc(e, "server.getConfig", json!({}))?;
    let list = editors(&config);
    let editor = match input["editor"].as_str().filter(|s| !s.is_empty()) {
        Some(id) => list.iter().find(|e| e["id"] == id),
        None => list.first(),
    }
    .ok_or("editor_unavailable")?;
    let cwd = workspace(&shell(e)?, thread)?;
    write::rpc(e, "shell.openInEditor", json!({"cwd":cwd,"editor":editor["id"]}))?;
    Ok(json!({"environment":e.id,"thread":thread,"editor":editor["id"],"opened":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_prefers_the_worktree_and_rejects_relative_folders() {
        let shell = json!({"projects":[{"id":"p","workspaceRoot":"/repo"}],"threads":[{"id":"a","projectId":"p","worktreePath":"/repo/.t3/wt"},{"id":"b","projectId":"p","worktreePath":null},{"id":"c","projectId":"q","worktreePath":"rel"}]});
        assert_eq!(workspace(&shell, "a").unwrap(), "/repo/.t3/wt");
        assert_eq!(workspace(&shell, "b").unwrap(), "/repo");
        assert!(workspace(&shell, "c").is_err());
        assert!(workspace(&shell, "missing").is_err());
    }
    #[test]
    fn only_installed_editors_are_offered_in_native_order() {
        let list = editors(&json!({"availableEditors":["zed","vscode"]}));
        let ids: Vec<_> = list.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["vscode", "zed"]);
    }
}
