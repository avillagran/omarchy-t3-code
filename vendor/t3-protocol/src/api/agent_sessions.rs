//! `agentSessions.*`: find and import sessions of other coding agents on the server's host.
use super::reader::put;
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

pub const SCAN: Function = api_fn! {
    name: "agentSessions.scan", module: "agentSessions", tag: Some("agentSessions.scan"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [], payload: |_| Ok(json!({})),
};
pub const IMPORT: Function = api_fn! {
    name: "agentSessions.import", module: "agentSessions", tag: Some("agentSessions.import"),
    kind: IdempotentMutation, scope: Fixed("orchestration:operate"),
    args: [(projectId, Id, required), (expectedWorkspaceRoot, Path, optional)],
    payload: |a| import_payload(a.req("projectId")?, a.str("expectedWorkspaceRoot")?),
};
pub const FUNCTIONS: &[Function] = &[SCAN, IMPORT];

/// `agentSessions.scan`: `{candidates[], scannedAt, truncated?}`.
pub fn scan(conn: &Connection) -> Result<Value> {
    send(conn, &SCAN, json!({}))
}
/// `agentSessions.import`: `{importedCount, skippedCount}`. Already imported
/// sessions are skipped, so a repeat does not duplicate them.
pub fn import(conn: &Connection, project_id: &str, expected_workspace_root: Option<&str>) -> Result<Value> {
    send(conn, &IMPORT, import_payload(project_id, expected_workspace_root)?)
}
fn import_payload(project_id: &str, expected_workspace_root: Option<&str>) -> Result<Value> {
    let mut v = json!({"projectId":ids::id("projectId", project_id)?});
    put(&mut v, "expectedWorkspaceRoot", expected_workspace_root.map(|p| ids::path("expectedWorkspaceRoot", p).map(str::trim)).transpose()?);
    Ok(v)
}
