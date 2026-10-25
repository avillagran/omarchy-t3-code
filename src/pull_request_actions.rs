//! Pull request writes from T3's pull request page: comment, review verdicts and the
//! host actions (merge, close, reopen, ready, draft...). The page only offers what the
//! detail's viewerPermissions allow; the server checks again.
use crate::{json, protocol::api, text, write::dispatch_command, Environment, Result};
use serde_json::Value;
pub use t3_protocol::pull_requests::{payload, thread_command};

pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    // `payload` builds through `api::pull_requests`; the registry sends it under that function.
    let (tag, payload) = payload(input)?;
    let result = api::call(&e.connection(true)?, tag, &payload, None)?;
    Ok(json!({"kind":input["kind"],"action":input["action"],"result":result}))
}

/// Link, unlink or watch a pull request on a thread (thread.pull-request.* commands).
pub fn thread_link(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !crate::valid_id(thread) {
        return Err("invalid_id".into());
    }
    let command = thread_command(input)?;
    dispatch_command(e, &command)?;
    Ok(json!({"link":input["link"]}))
}

/// Check a pull request out (git.preparePullRequestThread) and start a thread on it,
/// locally or in its own worktree, as T3's "Open in a thread".
pub fn open_thread(e: &Environment, input: &Value) -> Result<Value> {
    let cwd = input["cwd"].as_str().filter(|c| c.starts_with('/') && c.len() <= 4096).ok_or("invalid_folder")?;
    let project = text(input, "projectId");
    let url = input["url"].as_str().filter(|u| u.starts_with("https://") && u.len() <= 2048).ok_or("invalid_pull_request")?;
    let mode = if input["mode"] == "worktree" { "worktree" } else { "local" };
    if !crate::valid_id(project) || !input["modelSelection"].is_object() {
        return Err("invalid_project".into());
    }
    let thread = format!("omarchy-thread-{}", uuid::Uuid::new_v4());
    let worktree_thread = (mode == "worktree").then_some(thread.as_str());
    let prepared = api::vcs::prepare_pull_request_thread(&e.connection(true)?, cwd, url, mode, worktree_thread)?;
    let title = format!("#{} {}", prepared["pullRequest"]["number"], text(&prepared["pullRequest"], "title")).chars().take(200).collect::<String>();
    dispatch_command(e, &json!({"type":"thread.create","commandId":format!("omarchy-{}",uuid::Uuid::new_v4()),"threadId":thread,"projectId":project,"title":title.trim(),"modelSelection":input["modelSelection"],"runtimeMode":"approval-required","interactionMode":"default","branch":prepared["branch"],"worktreePath":prepared["worktreePath"],"createdBy":"user","creationSource":"web"}))?;
    Ok(json!({"threadId":thread,"branch":prepared["branch"],"worktreePath":prepared["worktreePath"],"onHead":prepared["isOnPullRequestHead"]!=false}))
}
