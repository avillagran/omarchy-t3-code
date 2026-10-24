//! Native Git header and project-script contracts. Reads only; mutation builders never send.
use serde_json::Value;
pub use t3_protocol::git_controls::{actions, build_action, build_verified_action, present, resolve_scripts, scope, scoped_result, validate_status, MAX_FILES, MAX_SCRIPTS};

type Result<T> = std::result::Result<T, String>;

/// One explicit user refresh: native shell + native settings + native VCS RPC. No polling,
/// command execution, local Git subprocess, filesystem path probing, or mutation credentials.
pub fn execute(e: &crate::Environment, input: &Value) -> Result<Value> {
    if input["environment"] != e.id {
        return Err("git_scope_changed".into());
    }
    let c = e.connection(false)?;
    let shell = c.request_json("GET", "/api/orchestration/shell", None, true)?;
    let scoped = scope(&shell, input)?;
    let project = shell["projects"]
        .as_array()
        .and_then(|ps| ps.iter().find(|p| p["id"] == scoped["projectId"]))
        .ok_or("git_project_unavailable")?;
    let settings = t3_protocol::api::server::settings(&c)?;
    let scripts = resolve_scripts(&settings, project)?;
    let status = t3_protocol::api::vcs::refresh_status(&c, crate::text(&scoped, "cwd"))?;
    present(&scoped, &status, &scripts)
}

