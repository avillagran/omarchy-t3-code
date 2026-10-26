//! Owner controls for a guest link; redemption/submission belong to the server.
use super::{api_fn, ids, Function};
use serde_json::json;
pub const CREATE: Function = api_fn! {
 name: "guestShare.create", module: "guestShare", tag: Some("guestShare.create"),
 kind: UnrepeatableMutation, scope: Fixed("access:write"),
 args: [(threadId, Id, required)], payload: |a| Ok(json!({"threadId":ids::id("threadId",a.req("threadId")?)?})),
};
pub const FUNCTIONS: &[Function] = &[CREATE];
pub fn create(conn: &crate::Connection, thread: &str) -> crate::Result<serde_json::Value> {
    CREATE.call(conn,&json!({"threadId":thread}),None)
}
