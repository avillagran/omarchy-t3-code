//! Native compact bounded snapshots, matching @t3tools/shared/orchestrationV2BoundedSnapshot.
//! Pinned T3 source is MIT. Restore before reducers see a snapshot.
use super::*;

/// Thread IDs accepted in a snapshot path: ASCII alphanumeric first, then
/// alphanumerics and `_:%.-`, at most 2048 bytes.
pub fn valid_thread_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 2048
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_:%.-".contains(&b))
}
/// `/api/orchestration/threads/<id>/bounded?compactTurnItems=1`. The native
/// opt-in drops duplicated local timeline items without dropping history.
pub fn path(thread_id: &str) -> Result<String> {
    if !valid_thread_id(thread_id) {
        return Err("invalid_id".into());
    }
    let mut u = Url::parse("http://thread.invalid/").map_err(|_| "invalid_endpoint")?;
    u.path_segments_mut()
        .map_err(|_| "invalid_endpoint")?
        .pop_if_empty()
        .extend(["api", "orchestration", "threads", thread_id, "bounded"]);
    u.query_pairs_mut().append_pair("compactTurnItems", "1").append_pair("dedupeBodies", "1").append_pair("compactIds", "1");
    Ok(format!("{}?{}", u.path(), u.query().unwrap_or("")))
}
/// Puts the omitted local items back in front of `turnItems`. A snapshot without
/// `turnItemsOmitLocalVisible` is returned unchanged; one with any other value
/// than `true`, or without a thread ID or rows, is `invalid_compact_snapshot`.
pub fn restore(mut snapshot: Value) -> Result<Value> {
    crate::wire::restore_snapshot(&mut snapshot)?;
    match snapshot.get("turnItemsOmitLocalVisible") {
        None => return Ok(snapshot),
        Some(Value::Bool(true)) => {}
        _ => return Err("invalid_compact_snapshot".into()),
    }
    let p = &snapshot["projection"];
    let thread = p["thread"]["id"].as_str().unwrap_or("");
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

impl Connection {
    /// The thread's bounded snapshot (protocol GET, up to 64 MiB), restored.
    pub fn bounded_thread(&self, thread_id: &str) -> Result<Value> {
        restore(self.request_json("GET", &path(thread_id)?, None, true)?)
    }
}
