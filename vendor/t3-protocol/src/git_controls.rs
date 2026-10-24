//! Native Git header and project-script contracts. Reads only; mutation builders never send.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const MAX_FILES: usize = 2048;
pub const MAX_SCRIPTS: usize = 256;
const MAX_RESULT_BYTES: usize = 1024 * 1024;
type Result<T> = std::result::Result<T, String>;

fn string<'a>(v: &'a Value, key: &str, max: usize) -> Result<&'a str> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty() && s.trim() == *s && s.len() <= max && !s.contains('\0'))
        .ok_or_else(|| format!("invalid_git_{key}"))
}
fn path(v: &Value, key: &str) -> Result<String> {
    let s = string(v, key, 8192)?;
    if !s.starts_with('/') || s.chars().any(char::is_control) {
        return Err("invalid_git_path".into());
    }
    Ok(s.into())
}
fn bounded(v: &Value) -> Result<()> {
    if serde_json::to_vec(v)
        .map_err(|_| "invalid_git_result")?
        .len()
        > MAX_RESULT_BYTES
    {
        return Err("git_result_limit".into());
    }
    Ok(())
}

/// Resolve against the native shell, never a client-supplied local directory. Project threads
/// only: scratch/draft threads without a project deliberately return unavailable.
pub fn scope(shell: &Value, input: &Value) -> Result<Value> {
    let environment = string(input, "environment", 2048)?;
    let thread = string(input, "thread", 2048)?;
    let request = string(input, "requestId", 128)?;
    let t = shell["threads"]
        .as_array()
        .and_then(|ts| {
            ts.iter()
                .find(|t| t["id"] == thread && t["deletedAt"].is_null())
        })
        .ok_or("git_thread_unavailable")?;
    let project_id = string(t, "projectId", 2048)?;
    let p = shell["projects"]
        .as_array()
        .and_then(|ps| {
            ps.iter()
                .find(|p| p["id"] == project_id && p["deletedAt"].is_null())
        })
        .ok_or("git_project_unavailable")?;
    let root = path(p, "workspaceRoot")?;
    let cwd = if t["worktreePath"].is_null() {
        root.clone()
    } else {
        path(t, "worktreePath")?
    };
    if input["projectId"] != project_id || input["cwd"] != cwd {
        return Err("git_scope_changed".into());
    }
    Ok(
        json!({"environment":environment,"thread":thread,"requestId":request,"projectId":project_id,"cwd":cwd,"projectRoot":root,"worktreePath":t["worktreePath"]}),
    )
}

/// Exact shared/projectScripts.ts precedence, including explicit empty overrides and legacy null.
pub fn resolve_scripts(settings: &Value, project: &Value) -> Result<Value> {
    let id = string(project, "id", 2048)?;
    let override_ = &settings["projectSettingsOverrides"][id]["defaultProjectScripts"];
    let defaults = &settings["defaultProjectScripts"];
    let legacy = settings["projectScriptOverrides"].get(id);
    let scripts = if !override_.is_null() {
        override_
    } else if settings["projectSettingsFolded"] == true || legacy == Some(&Value::Null) {
        defaults
    } else if let Some(v) = legacy {
        v
    } else if project["scripts"].as_array().is_some_and(|a| !a.is_empty()) {
        &project["scripts"]
    } else {
        defaults
    };
    let rows = scripts.as_array().ok_or("invalid_git_scripts")?;
    if rows.len() > MAX_SCRIPTS {
        return Err("git_scripts_limit".into());
    }
    let mut ids = HashSet::new();
    for s in rows {
        if !ids.insert(string(s, "id", 2048)?) {
            return Err("invalid_git_scripts".into());
        }
        string(s, "name", 2048)?;
        string(s, "command", 65535)?;
        if !matches!(
            s["icon"].as_str(),
            Some("play" | "test" | "lint" | "configure" | "build" | "debug")
        ) || !s["runOnWorktreeCreate"].is_boolean()
        {
            return Err("invalid_git_scripts".into());
        }
        for k in ["runOnSettle", "async", "autoOpenPreview"] {
            if s.get(k).is_some_and(|v| !v.is_boolean()) {
                return Err("invalid_git_scripts".into());
            }
        }
        if s.get("previewUrl").is_some() {
            string(s, "previewUrl", 8192)?;
        }
    }
    bounded(scripts)?;
    Ok(scripts.clone())
}

pub fn validate_status(status: &Value) -> Result<()> {
    bounded(status)?;
    for k in [
        "isRepo",
        "hasPrimaryRemote",
        "isDefaultRef",
        "hasWorkingTreeChanges",
        "hasUpstream",
    ] {
        if !status[k].is_boolean() {
            return Err("invalid_git_status".into());
        }
    }
    if !status["refName"].is_null() {
        string(status, "refName", 8192)?;
    }
    for k in ["aheadCount", "behindCount"] {
        if status[k].as_u64().is_none() {
            return Err("invalid_git_status".into());
        }
    }
    if status
        .get("aheadOfDefaultCount")
        .is_some_and(|v| v.as_u64().is_none())
    {
        return Err("invalid_git_status".into());
    }
    for k in ["insertions", "deletions"] {
        if status["workingTree"][k].as_u64().is_none() {
            return Err("invalid_git_status".into());
        }
    }
    let files = status["workingTree"]["files"]
        .as_array()
        .ok_or("invalid_git_status")?;
    if files.len() > MAX_FILES {
        return Err("git_files_limit".into());
    }
    let mut paths = HashSet::new();
    for f in files {
        // GitFilePath is nonempty, not trimmed: filenames may start/end with spaces.
        let name = f["path"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 8192 && !s.contains('\0'))
            .ok_or("invalid_git_status")?;
        if !paths.insert(name)
            || f["insertions"].as_u64().is_none()
            || f["deletions"].as_u64().is_none()
        {
            return Err("invalid_git_status".into());
        }
    }
    if !status["pr"].is_null() {
        let pr = &status["pr"];
        if pr["number"].as_u64().is_none_or(|n| n == 0)
            || !matches!(pr["state"].as_str(), Some("open" | "closed" | "merged"))
        {
            return Err("invalid_git_status".into());
        }
        for k in ["title", "url", "baseRef", "headRef"] {
            string(pr, k, 8192)?;
        }
    }
    Ok(())
}

/// Mirrors native menu availability; composite actions additionally refuse upstream divergence.
pub fn actions(status: &Value) -> Value {
    let repo = status["isRepo"] == true;
    let branch = status["refName"].is_string();
    let remote = status["hasPrimaryRemote"] == true;
    let upstream = status["hasUpstream"] == true;
    let ahead = status["aheadCount"].as_u64().unwrap_or(0);
    let behind = status["behindCount"].as_u64().unwrap_or(0);
    let changed = status["hasWorkingTreeChanges"] == true;
    let delta = status["aheadOfDefaultCount"].as_u64().unwrap_or(ahead);
    let pr = status["pr"]["state"] == "open";
    let pushable = branch && behind == 0 && (upstream || remote);
    let create = pushable && remote && !pr && delta > 0;
    json!({"commit":repo && changed,"push":repo && remote && pushable && ahead > 0,
        "pull":repo && branch && upstream && behind > 0 && ahead == 0,
        "create_pr":repo && create,"commit_push":repo && changed && remote && pushable,
        "commit_push_pr":repo && changed && remote && pushable && !pr})
}

pub fn present(scope: &Value, status: &Value, scripts: &Value) -> Result<Value> {
    validate_status(status)?;
    let mut output =
        json!({"scope":scope,"status":status,"scripts":scripts,"actions":actions(status)});
    bounded(&output)?;
    // Request identity rejects late reads but is NOT repository content. A fresh read with a
    // new request ID must compare equal if the checkout, status and scripts are unchanged.
    let mut content = output.clone();
    content["scope"]
        .as_object_mut()
        .ok_or("invalid_git_scope")?
        .remove("requestId");
    let revision = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&content).map_err(|_| "invalid_git_result")?)
    );
    output["revision"] = json!(revision);
    Ok(output)
}

/// None means a late/closed/different checkout response; errors for the current scope stay visible.
pub fn scoped_result(expected: &Value, receipt: &Value) -> Option<Result<Value>> {
    for k in ["environment", "thread", "projectId", "cwd", "requestId"] {
        let wanted = expected[k].as_str().filter(|s| !s.is_empty())?;
        if receipt[k].as_str() != Some(wanted) {
            return None;
        }
    }
    if let Some(error) = receipt["error"].as_str().filter(|s| !s.is_empty()) {
        return Some(Err(error.into()));
    }
    for k in ["environment", "thread", "projectId", "cwd", "requestId"] {
        if receipt["result"]["scope"][k] != expected[k] {
            return Some(Err("git_scope_changed".into()));
        }
    }
    Some(Ok(receipt["result"].clone()))
}

/// Build an explicit-confirmation outbox plan from an authoritative read, not QML command text.
/// Parent MUST reread and compare checkout/status/scripts immediately before dispatch, enforce
/// native write scopes, and execute steps sequentially without automatic mutation retries.
pub fn build_action(current: &Value, intent: &Value) -> Result<Value> {
    for k in ["environment", "thread", "projectId", "cwd", "requestId"] {
        string(&current["scope"], k, 8192)?;
        if intent["scope"][k] != current["scope"][k] {
            return Err("git_scope_changed".into());
        }
    }
    if intent["revision"] != current["revision"] || !current["revision"].is_string() {
        return Err("git_stale_result".into());
    }
    if intent["confirmed"] != true {
        return Err("git_confirmation_required".into());
    }
    let id = string(intent, "actionId", 128)?;
    uuid::Uuid::parse_str(id).map_err(|_| "invalid_git_actionId")?;
    let operation = string(intent, "operation", 32)?;
    let sc = &current["scope"];
    let thread = string(sc, "thread", 2048)?;
    let cwd = path(sc, "cwd")?;
    let steps = if operation == "script" {
        let script_id = string(intent, "scriptId", 2048)?;
        let script = current["scripts"]
            .as_array()
            .and_then(|ss| ss.iter().find(|s| s["id"] == script_id))
            .ok_or("git_script_unavailable")?;
        let command = string(script, "command", 65535)?;
        // Native terminal.write has a 65,536-character bound. Bytes are a stricter local bound.
        let terminal = format!("term-ot3-script-{id}");
        let root = path(sc, "projectRoot")?;
        let mut env = json!({"T3CODE_PROJECT_ROOT":root});
        if !sc["worktreePath"].is_null() {
            env["T3CODE_WORKTREE_PATH"] = json!(path(sc, "worktreePath")?);
        }
        let mut open = json!({"threadId":thread,"terminalId":terminal,"cwd":cwd,"env":env,"cols":120,"rows":30});
        if !sc["worktreePath"].is_null() {
            open["worktreePath"] = sc["worktreePath"].clone();
        }
        json!([{"tag":"terminal.open","payload":open},{"tag":"terminal.write","payload":{"threadId":thread,"terminalId":terminal,"data":format!("{command}\r")}}])
    } else {
        validate_status(&current["status"])?;
        if actions(&current["status"])[operation] != true {
            return Err("git_action_unavailable".into());
        }
        if current["status"]["isDefaultRef"] == true
            && operation != "commit"
            && operation != "pull"
            && intent["defaultBranchConfirmed"] != true
        {
            return Err("git_default_branch_confirmation_required".into());
        }
        if operation == "pull" {
            json!([{"tag":"vcs.pull","payload":{"cwd":cwd}}])
        } else {
            let mut payload = json!({"actionId":id,"cwd":cwd,"action":operation,"threadId":thread,"featureBranch":false});
            if let Some(v) = intent.get("commitMessage") {
                if !matches!(operation, "commit" | "commit_push" | "commit_push_pr") {
                    return Err("invalid_git_commitMessage".into());
                }
                let message = v
                    .as_str()
                    .filter(|s| {
                        !s.trim().is_empty()
                            && s.trim() == *s
                            && s.encode_utf16().count() <= 10000
                            && !s.contains('\0')
                    })
                    .ok_or("invalid_git_commitMessage")?;
                payload["commitMessage"] = json!(message);
            }
            if let Some(v) = intent.get("filePaths") {
                if !matches!(operation, "commit" | "commit_push" | "commit_push_pr") {
                    return Err("invalid_git_files".into());
                }
                let files = v
                    .as_array()
                    .filter(|f| !f.is_empty() && f.len() <= MAX_FILES)
                    .ok_or("invalid_git_files")?;
                let known = current["status"]["workingTree"]["files"]
                    .as_array()
                    .ok_or("invalid_git_status")?;
                let mut unique = HashSet::new();
                for f in files {
                    let p = f.as_str().ok_or("invalid_git_files")?;
                    if !unique.insert(p) || !known.iter().any(|k| k["path"] == p) {
                        return Err("invalid_git_files".into());
                    }
                }
                payload["filePaths"] = v.clone();
            }
            json!([{"tag":"git.runStackedAction","payload":payload}])
        }
    };
    Ok(
        json!({"scope":sc,"revision":current["revision"],"actionId":id,"operation":operation,"steps":steps,"retryAutomatically":false}),
    )
}

/// Parent obtains `fresh` with execute() immediately before enqueue, under its per-checkout
/// outbox lock. Native RPCs have no status CAS: another external Git client can still race.
pub fn build_verified_action(current: &Value, fresh: &Value, intent: &Value) -> Result<Value> {
    if current["revision"] != fresh["revision"] || !fresh["revision"].is_string() {
        return Err("git_stale_result".into());
    }
    build_action(current, intent)
}
