//! Host side of MCP apps (T3 McpAppFrame): an app's JSON-RPC requests that need the
//! server go through mcpApps.*, bound to the tool call that produced the app.
use crate::{json, text, write::rpc, Environment, Result};
use serde_json::Value;

pub fn request(input: &Value) -> Result<(&'static str, Value)> {
    let thread = text(input, "threadId");
    let item = text(input, "itemId");
    if !crate::valid_id(thread) || item.is_empty() || item.len() > 512 {
        return Err("invalid_id".into());
    }
    let params = &input["params"];
    Ok(match text(input, "method") {
        "tools/call" => {
            let name = params["name"].as_str().filter(|n| !n.trim().is_empty() && n.len() <= 256).ok_or("invalid_app_request")?;
            let arguments = if params["arguments"].is_object() { params["arguments"].clone() } else { json!({}) };
            ("mcpApps.callTool", json!({"threadId":thread,"itemId":item,"name":name,"arguments":arguments}))
        }
        "resources/read" => {
            let uri = params["uri"].as_str().filter(|u| !u.trim().is_empty() && u.len() <= 2048).ok_or("invalid_app_request")?;
            ("mcpApps.readResource", json!({"threadId":thread,"itemId":item,"uri":uri}))
        }
        "ui/update-model-context" => {
            let conversation = text(input, "conversationThreadId");
            let mut p = json!({"threadId":thread,"itemId":item,"conversationThreadId":if conversation.is_empty(){thread}else{conversation}});
            if params["content"].is_array() {
                p["content"] = params["content"].clone();
            }
            if params["structuredContent"].is_object() {
                p["structuredContent"] = params["structuredContent"].clone();
            }
            ("mcpApps.updateModelContext", p)
        }
        _ => return Err("unsupported_app_request".into()),
    })
}
pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    if input.to_string().len() > 1024 * 1024 {
        return Err("invalid_app_request".into());
    }
    let (tag, payload) = request(input)?;
    let result = rpc(e, tag, payload)?;
    Ok(if tag == "mcpApps.updateModelContext" { json!({}) } else { result })
}

#[cfg(test)]
mod tests {
    use super::request;
    use serde_json::json;
    #[test]
    fn maps_app_requests_to_the_server() {
        let base = json!({"threadId":"t","itemId":"i","method":"tools/call","params":{"name":"refresh","arguments":{"city":"Santiago"}}});
        let (tag, p) = request(&base).unwrap();
        assert_eq!((tag, p["arguments"]["city"].as_str()), ("mcpApps.callTool", Some("Santiago")));
        let (tag, p) = request(&json!({"threadId":"t","itemId":"i","method":"ui/update-model-context","params":{"structuredContent":{"a":1}}})).unwrap();
        assert_eq!((tag, p["conversationThreadId"].as_str()), ("mcpApps.updateModelContext", Some("t")));
        assert!(request(&json!({"threadId":"t","itemId":"i","method":"ui/open-link","params":{}})).is_err());
        assert!(request(&json!({"threadId":"t","itemId":"i","method":"tools/call","params":{}})).is_err());
    }
}
