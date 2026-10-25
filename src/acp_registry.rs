//! ACP agent registry (server.searchAcpRegistry / prepareAcpRegistryAgent): find an agent
//! and install it before adding it as an acpRegistry provider instance.
use crate::{json, protocol::api::server, text, Environment, Result};
use serde_json::Value;

pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    match text(input, "action") {
        "search" => {
            let query: String = text(input, "query").trim().chars().take(120).collect();
            let r = server::search_acp_registry(&e.connection(true)?, &query, None)?;
            let agents: Vec<Value> = r["agents"].as_array().into_iter().flatten().take(20).map(|a| json!({"id":a["id"],"name":a["name"],"version":a["version"],"description":a["description"],"license":a["license"],"website":a["website"]})).collect();
            Ok(json!({"agents":agents}))
        }
        "prepare" => {
            let id = input["agentId"].as_str().filter(|s| !s.trim().is_empty() && s.len() <= 256).ok_or("invalid_command")?;
            let r = server::prepare_acp_registry_agent(&e.connection(true)?, id)?;
            Ok(json!({"agentId":r["agentId"],"version":r["version"]}))
        }
        _ => Err("invalid_command".into()),
    }
}
