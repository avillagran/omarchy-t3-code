//! Thread branch picker: the composer's branch toolbar from T3 BranchToolbarBranchSelector.
//! Lists refs for the thread's checkout, switches or creates a branch through the server,
//! then records the branch (and worktree) on the thread.
use crate::{json, text, write::rpc, Environment, Result};
use serde_json::Value;

fn thread_scope(e: &Environment, thread: &str) -> Result<Value> {
    let shell = crate::Client::new()?.get(e, "/api/orchestration/shell", true)?;
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

/// Port of dedupeRemoteBranchesWithLocalMatches: an origin ref is hidden when its
/// local branch is listed.
pub fn dedupe(refs: &[Value]) -> Vec<Value> {
    let local: std::collections::HashSet<&str> = refs
        .iter()
        .filter(|r| r["isRemote"] != true)
        .filter_map(|r| r["name"].as_str())
        .collect();
    refs.iter()
        .filter(|r| {
            if r["isRemote"] != true || r["remoteName"] != "origin" {
                return true;
            }
            let name = text(r, "name");
            !local.contains(derive_local(name))
        })
        .cloned()
        .collect()
}
/// Port of deriveLocalBranchNameFromRemoteRef.
pub fn derive_local(name: &str) -> &str {
    match name.find('/') {
        Some(i) if i > 0 && i < name.len() - 1 => &name[i + 1..],
        _ => name,
    }
}
/// Port of sanitizeNewRefName.
pub fn sanitize(name: &str) -> String {
    name.split([' ', '\t', '\n', '\r', '\u{c}', '\u{b}'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
/// Port of resolveBranchSelectionTarget.
pub fn selection_target(project_cwd: &str, worktree: Option<&str>, r: &Value) -> (String, Option<String>, bool) {
    if let Some(path) = r["worktreePath"].as_str().filter(|s| !s.is_empty()) {
        return (path.into(), (path != project_cwd).then(|| path.to_string()), true);
    }
    let next = worktree.filter(|_| r["isDefault"] != true).map(str::to_string);
    (next.clone().unwrap_or_else(|| project_cwd.into()), next, false)
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
    rpc(
        e,
        "orchestration.dispatchCommand",
        json!({"type":"thread.metadata.update","commandId":format!("omarchy-branch-{id}"),"threadId":thread,"branch":branch,"worktreePath":next_worktree}),
    )?;
    Ok(json!({"branch":branch,"worktreePath":next_worktree}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helpers_follow_native_branch_toolbar() {
        assert_eq!(derive_local("origin/feature/x"), "feature/x");
        assert_eq!(derive_local("main"), "main");
        assert_eq!(derive_local("origin/"), "origin/");
        assert_eq!(sanitize("  my new\tbranch "), "my-new-branch");
        let refs = vec![
            json!({"name":"main","current":true,"isDefault":true}),
            json!({"name":"origin/main","isRemote":true,"remoteName":"origin"}),
            json!({"name":"origin/feat","isRemote":true,"remoteName":"origin"}),
            json!({"name":"upstream/main","isRemote":true,"remoteName":"upstream"}),
        ];
        let names: Vec<_> = dedupe(&refs).iter().map(|r| text(r, "name").to_string()).collect();
        assert_eq!(names, ["main", "origin/feat", "upstream/main"]);
        assert_eq!(selection_target("/p", None, &json!({"worktreePath":"/w"})), ("/w".into(), Some("/w".into()), true));
        assert_eq!(selection_target("/p", None, &json!({"worktreePath":"/p"})), ("/p".into(), None, true));
        assert_eq!(selection_target("/p", Some("/w"), &json!({"isDefault":true})), ("/p".into(), None, false));
        assert_eq!(selection_target("/p", Some("/w"), &json!({"isDefault":false})), ("/w".into(), Some("/w".into()), false));
    }
}
