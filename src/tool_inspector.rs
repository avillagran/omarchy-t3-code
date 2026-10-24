//! On-demand native turn-item reads. No persistence, polling or mutation.
use crate::{Environment, Result};
use serde_json::Value;
pub use t3_protocol::tool_inspector::{compact_reference, parse, request, scope, METHOD, RAW_BYTES, SECTION_BYTES};

/// Uses the existing connection, including its native relay/proof/credential routing.
pub fn execute(e: &Environment, input: &Value) -> Result<Value> {
    let (method, payload) = request(input)?;
    if input["environment"] != e.id {
        return Err("tool_scope_mismatch".into());
    }
    let response = e.connection(false)?.rpc(method, payload)?;
    parse(input, &response)
}

