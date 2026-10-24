//! `preview.*`: the thread's preview browser. Tanda 1 only reads its state.
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

pub const LIST: Function = api_fn! {
    name: "preview.list", module: "preview", tag: Some("preview.list"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(threadId, Id, required)],
    payload: |a| list_payload(a.req("threadId")?),
};
pub const FUNCTIONS: &[Function] = &[LIST];

/// `preview.list`: `{sessions[], serverEpoch, revision}` for one thread.
pub fn list(conn: &Connection, thread_id: &str) -> Result<Value> {
    send(conn, &LIST, list_payload(thread_id)?)
}
fn list_payload(thread_id: &str) -> Result<Value> {
    Ok(json!({"threadId":ids::id("threadId", thread_id)?}))
}
