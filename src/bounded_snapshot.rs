//! Native compact bounded snapshots, matching @t3tools/shared/orchestrationV2BoundedSnapshot.
//! Pinned T3 source is MIT; see licenses/T3-Code-MIT.txt. Restore before reducers see it.
use crate::{text, Result};
use serde_json::Value;
pub fn restore(mut snapshot: Value) -> Result<Value> {
    match snapshot.get("turnItemsOmitLocalVisible") {
        None => return Ok(snapshot),
        Some(Value::Bool(true)) => {}
        _ => return Err("invalid_compact_snapshot".into()),
    }
    let p = &snapshot["projection"];
    let thread = text(&p["thread"], "id");
    if thread.is_empty() {
        return Err("invalid_compact_snapshot".into());
    }
    let rows = p["visibleTurnItems"]
        .as_array()
        .ok_or("invalid_compact_snapshot")?;
    let mut local = Vec::new();
    for row in rows {
        if row["visibility"] == "local" || row["sourceThreadId"] == thread {
            if !row["item"].is_object() {
                return Err("invalid_compact_snapshot".into());
            }
            local.push(row["item"].clone());
        }
    }
    let remaining = snapshot["projection"]["turnItems"]
        .as_array_mut()
        .ok_or("invalid_compact_snapshot")?;
    local.append(remaining);
    *remaining = local;
    // Mark consumed so a restored projection can safely pass through another adapter.
    snapshot
        .as_object_mut()
        .ok_or("invalid_compact_snapshot")?
        .remove("turnItemsOmitLocalVisible");
    Ok(snapshot)
}
