//! Native PR reads shared with Omi; credentials and repositories come from T3.
//! Also the pull request writes from T3's pull request page: comment, review
//! verdicts and the host actions (merge, close, reopen, ready, draft...), and the
//! thread.pull-request.* link commands. The page only offers what the detail's
//! viewerPermissions allow; the server checks again.
use crate::util::{text, Result};
use serde_json::{json, Value};
pub fn request(v: &Value) -> Result<(String, Value)> {
    let action = v["action"].as_str().unwrap_or("");
    if action == "list" {
        let state = v["state"].as_str().unwrap_or("open");
        let involvement = v["involvement"].as_str().unwrap_or("all");
        if !["all", "open", "closed", "merged"].contains(&state)
            || !["all", "authored", "reviewing"].contains(&involvement)
        {
            return Err("invalid_filter".into());
        }
        let query = v["query"].as_str().unwrap_or("");
        if query.len() > 256 {
            return Err("invalid_filter".into());
        }
        let mut p = json!({"state":state,"involvement":involvement,"limit":50});
        if !query.trim().is_empty() {
            p["query"] = json!(query)
        }
        if v["cursors"].is_object() && v["cursors"].to_string().len() < 64000 {
            p["cursors"] = v["cursors"].clone()
        }
        return Ok(("pullRequests.list".into(), p));
    }
    let r = &v["reference"];
    let mut p = json!({});
    for key in ["projectId", "repository", "host"] {
        let s = r[key]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control))
            .ok_or("invalid_pull_request")?;
        p[key] = json!(s)
    }
    p["number"] = json!(r["number"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("invalid_pull_request")?);
    let tag = match action {
        "detail" => "pullRequests.detail",
        "checks" => "pullRequests.checks",
        "activity" => "pullRequests.activity",
        "diff" => "diff",
        _ => return Err("invalid_command".into()),
    };
    if action == "diff" {
        if let Some(cursor) = v["cursor"].as_str().filter(|s| s.len() < 4096) {
            p["cursor"] = json!(cursor)
        }
    }
    Ok((tag.into(), p))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_reads_are_scoped_and_never_mutate() {
        assert_eq!(
            request(
                &json!({"action":"list","state":"open","involvement":"authored","query":"rust"})
            )
            .unwrap()
            .1["limit"],
            50
        );
        let v = json!({"action":"detail","reference":{"projectId":"p","repository":"o/r","host":"github.com","number":7}});
        assert_eq!(request(&v).unwrap().0, "pullRequests.detail");
        let mut bad = v;
        bad["action"] = json!("merge");
        assert!(request(&bad).is_err());
    }
}

/// Native sidebar badge; one shared derivation for Omarchy and Omi.
pub fn sidebar_badge(thread: &Value) -> Value {
    let mut links = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let candidates = if let Some(a) = thread["pullRequests"].as_array() {
        a.clone()
    } else if thread["linkedPullRequest"].is_object() {
        vec![thread["linkedPullRequest"].clone()]
    } else {
        vec![]
    };
    let candidates = if candidates.is_empty() && thread["branchPullRequest"].is_object() {
        vec![thread["branchPullRequest"].clone()]
    } else {
        candidates
    };
    for link in candidates {
        if link["source"] == "stack-dismissed" {
            continue;
        }
        let Some(number) = link["number"].as_u64().filter(|n| *n > 0) else {
            continue;
        };
        let Some(url) = link["url"]
            .as_str()
            .and_then(|u| reqwest::Url::parse(u).ok())
            .filter(|u| {
                matches!(u.scheme(), "http" | "https")
                    && u.username().is_empty()
                    && u.password().is_none()
            })
        else {
            continue;
        };
        let host = link["host"]
            .as_str()
            .or_else(|| url.host_str())
            .unwrap_or("")
            .to_lowercase();
        let repository = link["repository"].as_str().unwrap_or("").to_owned();
        if host.is_empty()
            || repository.is_empty()
            || !seen.insert(format!("{host}/{}#{number}", repository.to_lowercase()))
        {
            continue;
        }
        let mut v = link.clone();
        v["host"] = json!(host);
        v["repository"] = json!(repository);
        v["number"] = json!(number);
        if !v["projectId"].is_string() {
            v["projectId"] = thread["projectId"].clone()
        }
        links.push(v);
    }
    if links.is_empty() {
        return Value::Null;
    }
    let n = links.len();
    let mut parents = vec![None; n];
    let mut children = vec![None; n];
    let mut linear = true;
    let native = links[0]["stack"]["id"].as_str().filter(|s| !s.is_empty());
    let native_stack = native.is_some()
        && links
            .iter()
            .all(|l| l["stack"]["id"].as_str() == native && l["host"] == links[0]["host"]);
    let mut stack = native_stack;
    if links.iter().all(|l| !l["stack"].is_object()) {
        for i in 0..n {
            let base = links[i]["snapshot"]["baseBranch"].as_str().unwrap_or("");
            if base.is_empty() {
                continue;
            }
            let candidates = (0..n)
                .filter(|j| {
                    *j != i
                        && links[*j]["host"] == links[i]["host"]
                        && links[*j]["repository"]
                            .as_str()
                            .unwrap_or("")
                            .eq_ignore_ascii_case(links[i]["repository"].as_str().unwrap_or(""))
                        && links[*j]["snapshot"]["headBranch"].as_str() == Some(base)
                })
                .collect::<Vec<_>>();
            if candidates.len() == 1 {
                let p = candidates[0];
                parents[i] = Some(p);
                if children[p].is_some() {
                    linear = false
                }
                children[p] = Some(i)
            }
        }
        let roots = (0..n).filter(|i| parents[*i].is_none()).collect::<Vec<_>>();
        if linear && roots.len() == 1 {
            let mut visited = std::collections::HashSet::new();
            let mut cursor = Some(roots[0]);
            while let Some(i) = cursor {
                if !visited.insert(i) {
                    break;
                }
                cursor = children[i]
            }
            stack = visited.len() == n
        }
    }
    let open = |l: &Value| l["snapshot"]["state"].as_str().unwrap_or("open") == "open";
    let mut eligible = (0..n).filter(|i| open(&links[*i])).collect::<Vec<_>>();
    if eligible.is_empty() {
        eligible = (0..n).collect()
    }
    let selected = if native_stack {
        let order = links[0]["stack"]["layers"].as_array();
        *eligible
            .iter()
            .max_by_key(|i| {
                order
                    .and_then(|a| a.iter().position(|l| l["number"] == links[**i]["number"]))
                    .unwrap_or(0)
            })
            .unwrap()
    } else if stack {
        *eligible
            .iter()
            .max_by_key(|i| {
                let mut depth = 0;
                let mut cursor = parents[**i];
                while let Some(p) = cursor {
                    depth += 1;
                    if depth > n {
                        break;
                    }
                    cursor = parents[p]
                }
                depth
            })
            .unwrap()
    } else {
        *eligible
            .iter()
            .max_by_key(|i| links[**i]["linkedAt"].as_str().unwrap_or(""))
            .unwrap()
    };
    let current = &links[selected];
    let state = if n == 1 {
        if current["snapshot"]["isDraft"] == true {
            "draft"
        } else {
            current["snapshot"]["state"].as_str().unwrap_or("unknown")
        }
    } else if links
        .iter()
        .all(|l| l["snapshot"]["state"] == "open" && l["snapshot"]["isDraft"] == true)
    {
        "draft"
    } else if links.iter().any(open) {
        "open"
    } else if links.iter().all(|l| l["snapshot"]["state"] == "merged") {
        "merged"
    } else {
        "closed"
    };
    let kind = if n > 1 && stack {
        "stack"
    } else if n > 1 {
        "multiple"
    } else {
        "single"
    };
    json!({"kind":kind,"text":if kind=="stack"{n.to_string()}else if n>1{format!("+{n}")}else{current["number"].to_string()},"state":state,"current":current,"links":links})
}

const ACTIONS: &[&str] = &["merge", "ready", "draft", "close", "reopen", "delete-branch", "update-branch", "enable-auto-merge", "disable-auto-merge"];

fn reference(input: &Value) -> Result<Value> {
    let mut probe = input.clone();
    probe["action"] = json!("detail");
    Ok(request(&probe)?.1)
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

#[cfg(test)]
mod action_tests {
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
