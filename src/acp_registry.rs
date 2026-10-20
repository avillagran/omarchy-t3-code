//! ACP agent registry (server.searchAcpRegistry / prepareAcpRegistryAgent): find an agent
//! and install it before adding it as an acpRegistry provider instance.
use crate::{json, text, write::rpc, Environment, Result};
use serde_json::Value;

pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    match text(input, "action") {
        "search" => {
            let query: String = text(input, "query").trim().chars().take(120).collect();
            let r = rpc(e, "server.searchAcpRegistry", json!({"query":query}))?;
            let agents: Vec<Value> = r["agents"].as_array().into_iter().flatten().take(20).map(|a| json!({"id":a["id"],"name":a["name"],"version":a["version"],"description":a["description"],"license":a["license"],"website":a["website"]})).collect();
            Ok(json!({"agents":agents}))
        }
        "prepare" => {
            let id = input["agentId"].as_str().filter(|s| !s.trim().is_empty() && s.len() <= 256).ok_or("invalid_command")?;
            let r = rpc(e, "server.prepareAcpRegistryAgent", json!({"agentId":id}))?;
            Ok(json!({"agentId":r["agentId"],"version":r["version"]}))
        }
        _ => Err("invalid_command".into()),
    }
}
