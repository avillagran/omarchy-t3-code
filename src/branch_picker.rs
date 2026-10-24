//! Thread branch picker: the composer's branch toolbar from T3 BranchToolbarBranchSelector.
//! Lists refs for the thread's checkout, switches or creates a branch through the server,
//! then records the branch (and worktree) on the thread.
use crate::{json, protocol, text, write::{dispatch_command, rpc}, Environment, Result};
use serde_json::Value;
pub use t3_protocol::branch_picker::{dedupe, derive_local, mismatch, sanitize, selection_target, temporary_branch};

fn thread_scope(e: &Environment, thread: &str) -> Result<Value> {
    let shell = crate::Client::shared()?.get(e, "/api/orchestration/shell", true)?;
    let t = shell["threads"]
        .as_array()
        .and_then(|ts| ts.iter().find(|t| t["id"] == thread && t["deletedAt"].is_null()))
        .ok_or("git_thread_unavailable")?;
    let p = shell["projects"]
        .as_array()
        .and_then(|ps| ps.iter().find(|p| p["id"] == t["projectId"] && p["deletedAt"].is_null()))
        .ok_or("git_project_unavailable")?;
    let root = p["workspaceRoot"].as_str().filter(|s| !s.is_empty()).ok_or("git_project_unavailable")?;
    let worktree = t["worktreePath"].as_str().filter(|s| !s.is_empty());
    Ok(json!({"projectCwd":root,"worktreePath":worktree,"cwd":worktree.unwrap_or(root),"branch":t["branch"],"working":!t["activeRunId"].is_null()}))
}

pub fn refs(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !crate::valid_id(thread) {
        return Err("invalid_id".into());
    }
    let query = sanitize(text(input, "query"));
    if query.encode_utf16().count() > 256 {
        return Err("invalid_review_query".into());
    }
    let scope = thread_scope(e, thread)?;
    let mut payload = json!({"cwd":scope["cwd"],"refKind":"all","includeMatchingRemoteRefs":true,"limit":100});
    if !query.is_empty() {
        payload["query"] = json!(query);
    }
    let raw = rpc(e, "vcs.listRefs", payload)?;
    let refs = raw["refs"].as_array().ok_or("invalid_review_refs")?;
    Ok(json!({"refs":dedupe(refs),"isRepo":raw["isRepo"],"query":query,"branch":scope["branch"],"worktreePath":scope["worktreePath"],"projectCwd":scope["projectCwd"]}))
}

pub fn switch(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !crate::valid_id(thread) {
        return Err("invalid_id".into());
    }
    let scope = thread_scope(e, thread)?;
    if scope["working"] == true {
        return Err("branch_change_while_working".into());
    }
    let project_cwd = text(&scope, "projectCwd");
    let worktree = scope["worktreePath"].as_str();
    let (branch, next_worktree) = if input["create"] == true {
        let name = sanitize(text(input, "refName"));
        if name.is_empty() || name.len() > 512 {
            return Err("invalid_review_ref".into());
        }
        let created = rpc(e, "vcs.createRef", json!({"cwd":scope["cwd"],"refName":name,"switchRef":true}))?;
        (created["refName"].as_str().unwrap_or(&name).to_string(), worktree.map(str::to_string))
    } else {
        let r = &input["ref"];
        let name = r["name"].as_str().filter(|s| !s.is_empty() && s.len() <= 512).ok_or("invalid_review_ref")?;
        let (checkout, next, reuse) = selection_target(project_cwd, worktree, r);
        if reuse {
            (name.to_string(), next)
        } else {
            let switched = rpc(e, "vcs.switchRef", json!({"cwd":checkout,"refName":name}))?;
            let local = if r["isRemote"] == true {
                switched["refName"].as_str().unwrap_or(derive_local(name)).to_string()
            } else {
                name.to_string()
            };
            (local, next)
        }
    };
    let id = uuid::Uuid::new_v4();
    dispatch_command(
        e,
        &json!({"type":"thread.metadata.update","commandId":format!("omarchy-branch-{id}"),"threadId":thread,"branch":branch,"worktreePath":next_worktree}),
    )?;
    Ok(json!({"branch":branch,"worktreePath":next_worktree}))
}

pub fn check(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !crate::valid_id(thread) {
        return Err("invalid_id".into());
    }
    let scope = thread_scope(e, thread)?;
    let status = protocol::api::vcs::refresh_status(&e.connection(true)?, text(&scope, "cwd"))?;
    if status["isRepo"] == false {
        return Ok(json!({"mismatch":null}));
    }
    Ok(json!({"mismatch":mismatch(scope["worktreePath"].as_str(), scope["branch"].as_str(), status["refName"].as_str())}))
}
/// Restore the thread's branch (local checkout) or adopt the checkout's branch.
pub fn resolve(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !crate::valid_id(thread) {
        return Err("invalid_id".into());
    }
    let scope = thread_scope(e, thread)?;
    let status = protocol::api::vcs::refresh_status(&e.connection(true)?, text(&scope, "cwd"))?;
    let Some(found) = mismatch(scope["worktreePath"].as_str(), scope["branch"].as_str(), status["refName"].as_str()).as_object().cloned() else {
        return Ok(json!({"mismatch":null}));
    };
    let branch = if input["restore"] == true {
        if found["canRestore"] != true {
            return Err("branch_restore_unavailable".into());
        }
        let switched = rpc(e, "vcs.switchRef", json!({"cwd":scope["cwd"],"refName":found["threadBranch"]}))?;
        switched["refName"].as_str().unwrap_or(text(&Value::Object(found.clone()), "threadBranch")).to_string()
    } else {
        text(&Value::Object(found.clone()), "currentBranch").to_string()
    };
    dispatch_command(e, &json!({"type":"thread.metadata.update","commandId":format!("omarchy-branch-{}",uuid::Uuid::new_v4()),"threadId":thread,"branch":branch}))?;
    Ok(json!({"mismatch":null,"branch":branch}))
}
