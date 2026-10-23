//! Stream capabilities a T3 server advertises in `server.getConfig`, read once per
//! environment and shared by the shell and chat streams. A completion marker is
//! only requested from a server that announces it.
use crate::{json, protocol, Result};
use serde_json::Value;
use std::{collections::HashMap, sync::Mutex};

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Capabilities {
    pub shell_completion_marker: bool,
    pub thread_completion_marker: bool,
}
impl Capabilities {
    pub fn from_config(config: &Value) -> Self {
        Self {
            shell_completion_marker: config["shellResumeCompletionMarker"] == true,
            thread_completion_marker: config["threadResumeCompletionMarker"] == true,
        }
    }
}
static CACHE: Mutex<Option<HashMap<String, Capabilities>>> = Mutex::new(None);

pub fn get(env: &str, c: &protocol::Connection) -> Result<Capabilities> {
    if let Some(found) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(env).copied()) {
        return Ok(found);
    }
    let found = match c.rpc("server.getConfig", json!({})) {
        Ok(config) => Capabilities::from_config(&config),
        // A refused config read only costs the catch-up markers.
        Err(error) if protocol::error_code(&error) == "rpc_rejected" => Capabilities::default(),
        Err(error) => return Err(error),
    };
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(env.to_string(), found);
    Ok(found)
}
/// Read again on the next connection, e.g. after the server ended a stream (it may have
/// been restarted on another version).
pub fn invalidate(env: &str) {
    if let Some(map) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        map.remove(env);
    }
}
