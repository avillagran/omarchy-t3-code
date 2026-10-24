//! Why a run failed, in the tool's own words: the failed tool items of a run
//! (T3 shows them in the timeline as "Failed to …" with their input and
//! output). Bounded thread reads omit tool output, so each failed item is read
//! once through orchestration.getTurnItem, and only a short excerpt is kept.
use crate::*;
use serde_json::{json, Value};
pub use t3_protocol::failure_context::{candidates, detail, input_summary, present};

fn read_item(e: &Environment, item: &Value) -> Value {
    let full = write::rpc(e, "orchestration.getTurnItem", json!({"threadId":item["threadId"],"itemId":item["id"],"revision":item["updatedAt"]}))
        .ok()
        .map(|r| r["item"].clone())
        .filter(|f| f["id"] == item["id"]);
    present(full.as_ref().unwrap_or(item))
}

/// op "failure-context": {environment, thread, runId}.
pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    let thread = text(input, "thread");
    let run = text(input, "runId");
    if !valid_id(thread) || run.is_empty() || run.len() > 1024 || run.chars().any(char::is_control) {
        return Err("invalid_id".into());
    }
    let d = Client::shared()?.bounded_thread(e, thread)?;
    let items = d["projection"]["turnItems"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let tools: Vec<Value> = candidates(items, run).into_iter().map(|i| read_item(e, i)).collect();
    Ok(json!({"environment":e.id,"thread":thread,"runId":run,"tools":tools}))
}

/// op "tool-failure": one failed timeline item, by its inspector reference.
pub fn tool(e: &Environment, input: &Value) -> Result<Value> {
    if input["environment"] != e.id {
        return Err("tool_scope_mismatch".into());
    }
    let (method, payload) = tool_inspector::request(input)?;
    let r = write::rpc(e, method, payload)?;
    let item = &r["item"];
    if item["id"] != input["sourceItemId"] {
        return Err("tool_scope_mismatch".into());
    }
    let mut out = present(item);
    out["sourceItemId"] = input["sourceItemId"].clone();
    Ok(out)
}

