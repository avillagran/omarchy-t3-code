//! Native user-input requests, shared by QML and Omi hosts.
use serde_json::{json, Value};

/// Match the native client's join: pending entity + latest matching turn item.
pub fn pending(p: &Value) -> Value {
    let mut inputs = Vec::new();
    for r in p["runtimeRequests"].as_array().into_iter().flatten() {
        if r["kind"] != "user_input" || r["status"] != "pending" {
            continue;
        }
        let Some(item) = p["turnItems"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find(|i| i["type"] == "user_input_request" && i["requestId"] == r["id"])
        else {
            continue;
        };
        if !item["questions"].is_array() {
            continue;
        }
        let capability = r["responseCapability"]["type"]
            .as_str()
            .unwrap_or("not_resumable");
        inputs.push(json!({"requestId":r["id"],"createdAt":r["createdAt"],"questions":item["questions"],"responseCapability":capability,"dismissible":item["responseMode"]=="message" || capability=="message"}));
    }
    inputs.sort_by(|a, b| {
        a["createdAt"]
            .as_str()
            .unwrap_or("")
            .cmp(b["createdAt"].as_str().unwrap_or(""))
    });
    json!(inputs)
}
/// Mutation payload only; callers own transport, permissions and durable command IDs.
pub fn command(input: &Value) -> Result<Value, String> {
    let thread = input["thread"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or("invalid_id")?;
    let request = input["runtimeRequestId"]
        .as_str()
        .filter(|v| !v.is_empty() && v.trim() == *v && !v.chars().any(char::is_control))
        .ok_or("invalid_runtime_request")?;
    if input["action"] == "dismiss" {
        return Ok(
            json!({"type":"thread.user-input.dismiss","threadId":thread,"requestId":request}),
        );
    }
    if input["action"] != "respond" {
        return Err("invalid_command".into());
    }
    let answers = input["answers"]
        .as_object()
        .filter(|v| !v.is_empty() && v.len() <= 100)
        .ok_or("invalid_answers")?;
    for (k, v) in answers {
        if k.is_empty()
            || !(v.is_string()
                || v.as_array()
                    .is_some_and(|a| a.len() <= 100 && a.iter().all(Value::is_string)))
        {
            return Err("invalid_answers".into());
        }
    }
    if input["answers"].to_string().len() > 480000 {
        return Err("invalid_answers".into());
    }
    Ok(
        json!({"type":"runtime-request.respond","threadId":thread,"requestId":request,"answers":answers}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn join_latest_pending_and_capabilities() {
        let p = json!({"runtimeRequests":[{"id":"done","kind":"user_input","status":"resolved"},{"id":"live","kind":"user_input","status":"pending","responseCapability":{"type":"live"}},{"id":"async","kind":"user_input","status":"pending","responseCapability":{"type":"message"}},{"id":"missing","kind":"user_input","status":"pending"}],"turnItems":[{"type":"user_input_request","requestId":"live","questions":[{"id":"old"}]},{"type":"user_input_request","requestId":"live","questions":[{"id":"new","initialAnswer":"  "}]},{"type":"user_input_request","requestId":"async","questions":[]}]});
        let rows = pending(&p);
        assert_eq!(rows.as_array().unwrap().len(), 2);
        assert_eq!(rows[0]["questions"][0]["id"], "new");
        assert_eq!(rows[0]["dismissible"], false);
        assert_eq!(rows[1]["dismissible"], true);
    }
    #[test]
    fn native_answer_shapes_and_dismiss() {
        let mut i = json!({"thread":"t","runtimeRequestId":"r","action":"respond","answers":{"single":" value ","multi":["a","b"],"editor":""}});
        let c = command(&i).unwrap();
        assert_eq!(c["type"], "runtime-request.respond");
        assert_eq!(c["answers"], i["answers"]);
        assert!(c.get("text").is_none());
        i["answers"]["bad"] = json!({"x":true});
        assert!(command(&i).is_err());
        i["action"] = json!("dismiss");
        assert_eq!(command(&i).unwrap()["type"], "thread.user-input.dismiss");
    }
    #[test]
    fn opaque_provider_ids_are_not_shortened_or_rejected_by_length() {
        let id = format!(
            "runtime-request:provider:acpRegistry:{}:elicitation:0",
            "provider-turn%253Aencoded%3A".repeat(32)
        );
        assert!(id.len() > 256);
        let mut input = json!({"thread":"thread","runtimeRequestId":id,"action":"respond","answers":{"q0":"Chosen response"}});
        assert_eq!(
            command(&input).unwrap()["requestId"],
            input["runtimeRequestId"]
        );
        input["action"] = json!("dismiss");
        assert_eq!(
            command(&input).unwrap()["requestId"],
            input["runtimeRequestId"]
        );
        input["runtimeRequestId"] = json!("bad\nrequest");
        assert!(command(&input).is_err());
    }
}
