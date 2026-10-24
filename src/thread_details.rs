//! Thread details (T3 ThreadDetailsPanel): the editors that can open the open
//! thread's workspace, and opening it there through shell.openInEditor.
//! The folder always comes from the server's own thread and project records,
//! never from the panel.
use crate::*;
use serde_json::{json, Value};
pub use t3_protocol::thread_details::{editors, workspace};

fn shell(e: &Environment) -> Result<Value> {
    Client::shared()?.get(e, "/api/orchestration/shell", true)
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

