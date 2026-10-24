//! `vcs.*`: repository status. Tanda 1 only reads it.
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

pub const REFRESH_STATUS: Function = api_fn! {
    name: "vcs.refreshStatus", module: "vcs", tag: Some("vcs.refreshStatus"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(cwd, Path, required)],
    payload: |a| refresh_status_payload(a.req("cwd")?),
};
pub const FUNCTIONS: &[Function] = &[REFRESH_STATUS];

/// `vcs.refreshStatus`: `VcsStatus` of the repository at `cwd` on the server's host.
pub fn refresh_status(conn: &Connection, cwd: &str) -> Result<Value> {
    send(conn, &REFRESH_STATUS, refresh_status_payload(cwd)?)
}
fn refresh_status_payload(cwd: &str) -> Result<Value> {
    Ok(json!({"cwd":ids::path("cwd", cwd)?}))
}
