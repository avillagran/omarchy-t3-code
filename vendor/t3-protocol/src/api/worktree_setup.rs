//! `worktreeSetup.cancel`: stop a thread's worktree setup scripts.
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

pub const CANCEL: Function = api_fn! {
    name: "worktreeSetup.cancel", module: "worktreeSetup", tag: Some("worktreeSetup.cancel"),
    kind: IdempotentMutation, scope: Fixed("orchestration:operate"),
    args: [(threadId, Id, required)],
    payload: |a| cancel_payload(a.req("threadId")?),
};
pub const FUNCTIONS: &[Function] = &[CANCEL];

/// `worktreeSetup.cancel`: `{cancelled}`.
pub fn cancel(conn: &Connection, thread_id: &str) -> Result<Value> {
    send(conn, &CANCEL, cancel_payload(thread_id)?)
}
fn cancel_payload(thread_id: &str) -> Result<Value> {
    Ok(json!({"threadId":ids::id("threadId", thread_id)?}))
}
