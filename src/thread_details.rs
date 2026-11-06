//! Thread details (T3 ThreadDetailsPanel): the editors that can open the open
//! thread's workspace, and opening it there through shell.openInEditor.
//! Workspace actions derive their folder from server records. Explicit terminal
//! links use a source-resolved target after fresh thread, grant and metadata checks.
use crate::*;
use serde_json::{json, Value};
pub use t3_protocol::thread_details::{editors, workspace};

fn shell(e: &Environment) -> Result<Value> {
    Client::shared()?.get(e, "/api/orchestration/shell", true)
}

fn terminal_target(input: &Value) -> Result<Option<&str>> {
    input
        .get("target")
        .map(|value| {
            let target = value.as_str().ok_or("invalid_terminal_link")?;
            protocol::api::ids::path("target", target)
        })
        .transpose()
}
fn terminal_physical_path(target: &str) -> &str {
    let mut path = target;
    for _ in 0..2 {
        let Some((parent, position)) = path.rsplit_once(':') else {
            break;
        };
        if position.is_empty() || !position.bytes().all(|b| b.is_ascii_digit()) {
            break;
        }
        path = parent;
    }
    path
}
fn terminal_metadata(
    e: &Environment,
    target: &str,
    snapshot: &Value,
    thread: &str,
) -> Result<Value> {
    if !snapshot["threads"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|row| row["id"] == thread)
    {
        return Err("unknown_thread".into());
    }
    let connection = e.connection(true)?;
    let session = connection.request_json("GET", "/api/auth/session", None, false)?;
    let scopes = session
        .get("permissions")
        .or_else(|| session.get("scopes"))
        .and_then(Value::as_array);
    if session["authenticated"] != true
        || !["orchestration:operate", "filesystem:read"]
            .iter()
            .all(|scope| scopes.is_some_and(|s| s.iter().any(|v| v == scope)))
    {
        return Err("permission_denied".into());
    }
    let path = terminal_physical_path(target);
    let metadata = protocol::api::filesystem::metadata(&connection, &[path.to_owned()])?;
    let kind = metadata["entries"][0]["kind"]
        .as_str()
        .filter(|kind| matches!(*kind, "file" | "directory"));
    Ok(json!({"target":target,"path":path,"kind":kind}))
}

pub fn read(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !valid_id(thread) {
        return Err("invalid_id".into());
    }
    let config = protocol::api::server::config(&e.connection(true)?)?;
    let snapshot = shell(e)?;
    let cwd = workspace(&snapshot, thread).ok();
    let mut result =
        json!({"environment":e.id,"thread":thread,"editors":editors(&config),"cwd":cwd});
    if let Some(target) = terminal_target(input)? {
        result["terminalLink"] = terminal_metadata(e, target, &snapshot, thread)?;
    }
    Ok(result)
}

pub fn open(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !valid_id(thread) {
        return Err("invalid_id".into());
    }
    let config = protocol::api::server::config(&e.connection(true)?)?;
    let list = editors(&config);
    let editor = match input["editor"].as_str().filter(|s| !s.is_empty()) {
        Some(id) => list.iter().find(|e| e["id"] == id),
        None => list.first(),
    }
    .ok_or("editor_unavailable")?;
    let snapshot = shell(e)?;
    let target = terminal_target(input)?;
    let cwd = if let Some(target) = target {
        let metadata = terminal_metadata(e, target, &snapshot, thread)?;
        if metadata["kind"].is_null() {
            return Err("terminal_link_unavailable".into());
        }
        target.to_owned()
    } else {
        workspace(&snapshot, thread)?
    };
    protocol::api::shell::open_in_editor(&e.connection(true)?, &cwd, text(editor, "id"), None)?;
    let mut result =
        json!({"environment":e.id,"thread":thread,"editor":editor["id"],"opened":true});
    if let Some(target) = target {
        result["target"] = json!(target);
    }
    Ok(result)
}
