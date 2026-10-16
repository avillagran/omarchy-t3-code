//! Native PR reads shared with Omi; credentials and repositories come from T3.
use serde_json::{json, Value};
pub fn request(v: &Value) -> Result<(String, Value), String> {
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
