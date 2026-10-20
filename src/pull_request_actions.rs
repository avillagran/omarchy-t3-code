//! Pull request writes from T3's pull request page: comment, review verdicts and the
//! host actions (merge, close, reopen, ready, draft...). The page only offers what the
//! detail's viewerPermissions allow; the server checks again.
use crate::{json, protocol, text, write::rpc, Environment, Result};
use serde_json::Value;

const ACTIONS: &[&str] = &["merge", "ready", "draft", "close", "reopen", "delete-branch", "update-branch", "enable-auto-merge", "disable-auto-merge"];

fn reference(input: &Value) -> Result<Value> {
    let mut probe = input.clone();
    probe["action"] = json!("detail");
    Ok(protocol::pull_requests::request(&probe)?.1)
}
fn body(input: &Value, allow_empty: bool) -> Result<String> {
    let body = input["body"].as_str().unwrap_or("");
    if body.len() > 65_536 || (!allow_empty && body.trim().is_empty()) {
        return Err("invalid_comment".into());
    }
    Ok(body.to_string())
}
pub fn payload(input: &Value) -> Result<(&'static str, Value)> {
    let mut p = reference(input)?;
    Ok(match text(input, "kind") {
        "comment" => {
            p["body"] = json!(body(input, false)?);
            ("pullRequests.comment", p)
        }
        "review" => {
            let verdict = text(input, "verdict");
            if !matches!(verdict, "comment" | "approve" | "request-changes") {
                return Err("invalid_review".into());
            }
            // A plain comment review needs words; approving may be silent.
            p["body"] = json!(body(input, verdict != "comment")?);
            p["verdict"] = json!(verdict);
            p["comments"] = json!([]);
            ("pullRequests.submitReview", p)
        }
        "action" => {
            let action = text(input, "action");
            if !ACTIONS.contains(&action) {
                return Err("invalid_command".into());
            }
            p["action"] = json!(action);
            if let Some(method) = input["mergeMethod"].as_str() {
                if !matches!(method, "merge" | "squash" | "rebase") {
                    return Err("invalid_command".into());
                }
                p["mergeMethod"] = json!(method);
            }
            ("pullRequests.runAction", p)
        }
        "reviewer-candidates" => ("pullRequests.reviewerCandidates", p),
        "label-candidates" => ("pullRequests.labelCandidates", p),
        "reviewers" => {
            let rows = input["reviewers"].as_array().filter(|r| !r.is_empty() && r.len() <= 25).ok_or("invalid_command")?;
            let mut list = vec![];
            for r in rows {
                let id = r["id"].as_str().filter(|s| !s.trim().is_empty() && s.len() <= 256).ok_or("invalid_command")?;
                let kind = r["kind"].as_str().filter(|k| matches!(*k, "user" | "team")).ok_or("invalid_command")?;
                list.push(json!({"id":id,"kind":kind}));
            }
            p["reviewers"] = json!(list);
            p["requested"] = json!(input["requested"] == true);
            ("pullRequests.requestReviewers", p)
        }
        "labels" => {
            let labels: Vec<&str> = input["labels"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|l| !l.trim().is_empty() && l.len() <= 256).collect();
            if labels.is_empty() || labels.len() > 25 {
                return Err("invalid_command".into());
            }
            p["labels"] = json!(labels);
            p["applied"] = json!(input["applied"] == true);
            ("pullRequests.setLabels", p)
        }
        _ => return Err("invalid_command".into()),
    })
}
pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    let (tag, payload) = payload(input)?;
    let result = rpc(e, tag, payload)?;
    Ok(json!({"kind":input["kind"],"action":input["action"],"result":result}))
}

#[cfg(test)]
mod tests {
    use super::payload;
    use serde_json::json;
    #[test]
    fn writes_are_validated_and_scoped() {
        let r = json!({"projectId":"p","repository":"o/r","host":"github.com","number":7});
        let (tag, p) = payload(&json!({"kind":"action","action":"merge","mergeMethod":"squash","reference":r})).unwrap();
        assert_eq!((tag, &p["action"], &p["mergeMethod"], &p["number"]), ("pullRequests.runAction", &json!("merge"), &json!("squash"), &json!(7)));
        assert!(payload(&json!({"kind":"action","action":"revert","reference":r})).is_err());
        assert!(payload(&json!({"kind":"comment","body":"  ","reference":r})).is_err());
        let (tag, p) = payload(&json!({"kind":"review","verdict":"approve","body":"","reference":r})).unwrap();
        assert_eq!((tag, &p["verdict"], &p["comments"]), ("pullRequests.submitReview", &json!("approve"), &json!([])));
        assert!(payload(&json!({"kind":"review","verdict":"comment","body":"","reference":r})).is_err());
        assert!(payload(&json!({"kind":"comment","body":"hi","reference":{"projectId":"p","repository":"o/r","host":"github.com","number":0}})).is_err());
    }
}

/// Link, unlink or watch a pull request on a thread (thread.pull-request.* commands).
pub fn thread_link(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    if !crate::valid_id(thread) {
        return Err("invalid_id".into());
    }
    let command = thread_command(input)?;
    rpc(e, "orchestration.dispatchCommand", command)?;
    Ok(json!({"link":input["link"]}))
}
pub fn thread_command(input: &Value) -> Result<Value> {
    let r = &input["reference"];
    let host = r["host"].as_str().filter(|s| !s.is_empty() && s.len() <= 512).ok_or("invalid_pull_request")?;
    let repository = r["repository"].as_str().filter(|s| !s.is_empty() && s.len() <= 512).ok_or("invalid_pull_request")?;
    let number = r["number"].as_u64().filter(|n| *n > 0).ok_or("invalid_pull_request")?;
    let id = format!("omarchy-pr-{}", uuid::Uuid::new_v4());
    let mut c = json!({"commandId":id,"threadId":input["thread"],"host":host,"repository":repository,"number":number});
    let url = || {
        r["url"]
            .as_str()
            .filter(|u| reqwest::Url::parse(u).is_ok_and(|u| matches!(u.scheme(), "http" | "https")))
            .ok_or("invalid_pull_request")
    };
    match text(input, "link") {
        "link" => {
            c["type"] = json!("thread.pull-request.link");
            c["url"] = json!(url()?);
            c["source"] = json!("manual");
        }
        "unlink" => c["type"] = json!("thread.pull-request.unlink"),
        "watch" | "unwatch" => {
            c["type"] = json!("thread.pull-request.watch");
            c["watching"] = json!(text(input, "link") == "watch");
            if text(input, "link") == "watch" {
                c["link"] = json!({"url":url()?,"source":"manual"});
            }
        }
        _ => return Err("invalid_command".into()),
    }
    Ok(c)
}
#[cfg(test)]
mod link_tests {
    use super::thread_command;
    use serde_json::json;
    #[test]
    fn thread_links_follow_the_native_commands() {
        let r = json!({"host":"github.com","repository":"o/r","number":5,"url":"https://github.com/o/r/pull/5"});
        let c = thread_command(&json!({"thread":"t","link":"link","reference":r})).unwrap();
        assert_eq!((c["type"].as_str(), c["source"].as_str(), c["number"].as_u64()), (Some("thread.pull-request.link"), Some("manual"), Some(5)));
        let c = thread_command(&json!({"thread":"t","link":"watch","reference":r})).unwrap();
        assert_eq!((c["watching"].as_bool(), c["link"]["url"].as_str()), (Some(true), Some("https://github.com/o/r/pull/5")));
        let c = thread_command(&json!({"thread":"t","link":"unlink","reference":r})).unwrap();
        assert!(c.get("url").is_none());
        assert!(thread_command(&json!({"thread":"t","link":"link","reference":{"host":"github.com","repository":"o/r","number":5,"url":"javascript:x"}})).is_err());
    }
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
    let mut payload = json!({"cwd":cwd,"reference":url,"mode":mode});
    if mode == "worktree" {
        payload["threadId"] = json!(thread);
    }
    let prepared = rpc(e, "git.preparePullRequestThread", payload)?;
    let title = format!("#{} {}", prepared["pullRequest"]["number"], text(&prepared["pullRequest"], "title")).chars().take(200).collect::<String>();
    rpc(e, "orchestration.dispatchCommand", json!({"type":"thread.create","commandId":format!("omarchy-{}",uuid::Uuid::new_v4()),"threadId":thread,"projectId":project,"title":title.trim(),"modelSelection":input["modelSelection"],"runtimeMode":"approval-required","interactionMode":"default","branch":prepared["branch"],"worktreePath":prepared["worktreePath"],"createdBy":"user","creationSource":"web"}))?;
    Ok(json!({"threadId":thread,"branch":prepared["branch"],"worktreePath":prepared["worktreePath"],"onHead":prepared["isOnPullRequestHead"]!=false}))
}
