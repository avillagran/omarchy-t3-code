//! Thread workspace/editor actions and native cached terminal metadata queries.
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
fn metadata(e: &Environment, input: &Value, target: &str, thread: &str) -> Result<Value> {
    let reference = file_metadata_cache::ThreadRef {
        environment: e.id.clone(),
        thread: thread.into(),
    };
    let Some(query) =
        file_metadata_admission::query(&e.id, target, input["cwd"].as_str(), Some(&reference))
    else {
        return Ok(json!({"target":target,"path":null,"kind":null}));
    };
    let path = text(&query["input"], "path");
    if input.get("path").is_some_and(|value| value != path) {
        return Err("invalid_terminal_link".into());
    }
    let lease = input["leaseId"].as_str().filter(|value| !value.is_empty());
    let owner = text(input, "owner");
    let mut queries = file_metadata_runtime::global().query(&e.id, &[path.into()], lease, owner)?;
    let answer = queries.remove(0).wait()?;
    let kind = answer
        .value
        .as_ref()
        .map(|value| value.kind.as_str())
        .filter(|kind| matches!(*kind, "file" | "directory"));
    Ok(
        json!({"target":target,"path":answer.path,"kind":kind,"sessionId":answer.session,"revision":answer.revision}),
    )
}
pub fn read(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !valid_id(thread) {
        return Err("invalid_id".into());
    }
    if let Some(target) = terminal_target(input)? {
        // Generic metadata requires only filesystem:read. The actual terminal
        // hover/click additionally requires orchestration:operate in the UI.
        return Ok(
            json!({"environment":e.id,"thread":thread,"terminalLink":metadata(e,input,target,thread)?}),
        );
    }
    let config = protocol::api::server::config(&e.connection(false)?)?;
    let snapshot = shell(e)?;
    Ok(
        json!({"environment":e.id,"thread":thread,"editors":editors(&config),"cwd":workspace(&snapshot,thread).ok()}),
    )
}
pub fn open(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !valid_id(thread) {
        return Err("invalid_id".into());
    }
    let connection = e.connection(true)?;
    file_metadata_runtime::authorize(&connection, "orchestration:operate")?;
    let config = protocol::api::server::config(&connection)?;
    let list = editors(&config);
    let editor = match input["editor"].as_str().filter(|value| !value.is_empty()) {
        Some(id) => list.iter().find(|editor| editor["id"] == id),
        None => list.first(),
    }
    .ok_or("editor_unavailable")?;
    let snapshot = shell(e)?;
    if !snapshot["threads"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|row| row["id"] == thread)
    {
        return Err("unknown_thread".into());
    }
    let target = terminal_target(input)?;
    let cwd = if let Some(target) = target {
        file_metadata_runtime::authorize(&connection, "filesystem:read")?;
        let metadata = metadata(e, input, target, thread)?;
        if metadata["kind"].is_null() {
            return Err("terminal_link_unavailable".into());
        }
        target.to_owned()
    } else {
        workspace(&snapshot, thread)?
    };
    protocol::api::shell::open_in_editor(&connection, &cwd, text(editor, "id"), None)?;
    let mut result =
        json!({"environment":e.id,"thread":thread,"editor":editor["id"],"opened":true});
    if let Some(target) = target {
        result["target"] = json!(target)
    }
    Ok(result)
}
