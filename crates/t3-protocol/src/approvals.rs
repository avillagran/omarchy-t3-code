//! Native pending approvals and decisions. Shared by desktop and Omi hosts.
use serde_json::{json, Value};

fn valid_decision(s: &str) -> bool {
    matches!(
        s,
        "accept" | "acceptForSession" | "acceptAlways" | "decline" | "cancel"
    )
}

pub fn pending(p: &Value) -> Value {
    let mut rows = Vec::new();
    for r in p["runtimeRequests"].as_array().into_iter().flatten() {
        if r["status"] != "pending"
            || !matches!(
                r["kind"].as_str(),
                Some("command" | "file-read" | "file-change" | "permission" | "mcp-elicitation")
            )
        {
            continue;
        }
        let item = p["turnItems"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find(|i| i["type"] == "approval_request" && i["requestId"] == r["id"]);
        let mut row = json!({"requestId":r["id"],"requestKind":r["kind"],"createdAt":r["createdAt"],"responseCapability":if r["responseCapability"]["type"]=="live" {"live"} else {"not_resumable"}});
        if let Some(i) = item {
            for (source, target) in [
                ("prompt", "detail"),
                ("appName", "appName"),
                ("options", "options"),
            ] {
                if let Some(v) = i.get(source) {
                    row[target] = v.clone();
                }
            }
        }
        // Absence means native defaults; an explicitly empty provider list stays empty.
        if row.get("options").is_none() {
            row["options"] = json!([
                {"decision":"cancel","label":"Cancel"},
                {"decision":"decline","label":"Decline"},
                {"decision":"acceptForSession","label":"Always allow this session"},
                {"decision":"accept","label":"Approve"}
            ]);
        }
        rows.push(row);
    }
    rows.sort_by(|a, b| {
        a["createdAt"]
            .as_str()
            .unwrap_or("")
            .cmp(b["createdAt"].as_str().unwrap_or(""))
    });
    json!(rows)
}

/// Build a decision only for an actionable request and a provider-advertised choice.
pub fn command(input: &Value, projection: &Value) -> Result<Value, String> {
    let thread = input["thread"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("invalid_id")?;
    let request = input["runtimeRequestId"]
        .as_str()
        .filter(|s| !s.is_empty() && s.trim() == *s && !s.chars().any(char::is_control))
        .ok_or("invalid_runtime_request")?;
    let decision = input["decision"]
        .as_str()
        .filter(|s| valid_decision(s))
        .ok_or("invalid_approval_decision")?;
    let rows = pending(projection);
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["requestId"] == request)
        .ok_or("approval_no_longer_pending")?;
    if row["responseCapability"] != "live" {
        return Err("approval_not_resumable".into());
    }
    if !row["options"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|o| o["decision"] == decision)
    {
        return Err("invalid_approval_decision".into());
    }
    Ok(
        json!({"type":"runtime-request.respond","threadId":thread,"requestId":request,"decision":decision}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_join_defaults_provider_choices_and_guards() {
        let id = format!("acp:{}", "opaque%253A".repeat(60));
        let mut p = json!({"runtimeRequests":[{"id":id,"status":"pending","kind":"command","createdAt":"2","responseCapability":{"type":"live"}},{"id":"missing-item","status":"pending","kind":"permission","createdAt":"1","responseCapability":{"type":"live"}},{"id":"auth","status":"pending","kind":"auth_refresh"},{"id":"question","status":"pending","kind":"user_input"},{"id":"done","status":"resolved","kind":"command"}],"turnItems":[{"type":"approval_request","requestId":id,"prompt":"old"},{"type":"approval_request","requestId":id,"prompt":"rm -rf ./build","appName":"Tool","options":[{"decision":"decline","label":"No"},{"decision":"acceptAlways","label":"Allow permanently","warning":"Persistent access"}]}]});
        let rows = pending(&p);
        assert_eq!(rows.as_array().unwrap().len(), 2);
        assert_eq!(rows[0]["options"][3]["decision"], "accept");
        assert_eq!(rows[1]["detail"], "rm -rf ./build");
        assert_eq!(rows[1]["options"][1]["warning"], "Persistent access");
        let mut i = json!({"thread":"t","runtimeRequestId":id,"decision":"acceptAlways"});
        assert_eq!(
            command(&i, &p).unwrap(),
            json!({"type":"runtime-request.respond","threadId":"t","requestId":id,"decision":"acceptAlways"})
        );
        i["decision"] = json!("accept");
        assert!(command(&i, &p).is_err());
        i["decision"] = json!("acceptAlways");
        p["runtimeRequests"][0]["responseCapability"]["type"] = json!("message");
        assert_eq!(command(&i, &p).unwrap_err(), "approval_not_resumable");
        p["runtimeRequests"][0]["status"] = json!("resolved");
        assert_eq!(command(&i, &p).unwrap_err(), "approval_no_longer_pending");
        p["turnItems"][1]["options"] = json!([]);
        p["runtimeRequests"][0]["status"] = json!("pending");
        p["runtimeRequests"][0]["responseCapability"]["type"] = json!("live");
        assert!(command(&i, &p).is_err());
    }
}
