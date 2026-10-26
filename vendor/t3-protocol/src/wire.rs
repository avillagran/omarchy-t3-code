//! Nightly 3726 wire opt-ins. Decode before reducers or caches see records.
use serde_json::Value;
use crate::Result;

fn record(record: &mut Value, ids: &[Value]) -> Result<()> {
    let Some(fields) = record.as_object_mut() else { return Ok(()) };
    for (key, value) in fields {
        if key == "id" || key.ends_with("Id") {
            if value.is_number() { *value = lookup(value, ids)?; }
        } else if key.ends_with("Ids") {
            if let Some(list) = value.as_array_mut() {
                for entry in list { if entry.is_number() { *entry = lookup(entry, ids)?; } }
            }
        } else if key == "item" { record_ids(value, ids)?; }
    }
    Ok(())
}
fn record_ids(value: &mut Value, ids: &[Value]) -> Result<()> { record(value, ids) }
fn lookup(value: &Value, ids: &[Value]) -> Result<Value> {
    value.as_u64().and_then(|n| usize::try_from(n).ok()).and_then(|n| ids.get(n)).cloned().ok_or_else(|| "invalid_id_table".into())
}
/// ID references occur only on records and their `item`, never arbitrary nested JSON.
pub fn expand_ids(value: &mut Value) -> Result<()> {
    let Some(ids) = value.get("ids") else { return Ok(()) };
    let ids = ids.as_array().filter(|ids| ids.iter().all(|id| id.is_string() || id.is_number())).ok_or("invalid_id_table")?.clone();
    let container = if value.get("projection").is_some() { &mut value["projection"] }
        else { &mut *value };
    if let Some(fields) = container.as_object_mut() {
        for (key, v) in fields {
            if key == "ids" { continue; }
            if let Some(rows) = v.as_array_mut() { for row in rows { record(row, &ids)?; } }
            else if v.is_object() { record(v, &ids)?; }
        }
    }
    value.as_object_mut().ok_or("invalid_id_table")?.remove("ids");
    Ok(())
}
fn source<'a>(projection: &'a Value, key: &str, id: &Value) -> Option<&'a Value> {
    projection[key].as_array()?.iter().rev().find(|row| row["id"] == *id)
}
fn restore_field(item: &mut Value, previous: Option<&Value>, field: &str, marker: &str) -> Result<()> {
    if item[marker] != true { return Ok(()) }
    let prior = previous.filter(|p| p[marker] != true).and_then(|p| p.get(field)).ok_or("missing_deduped_body")?;
    item[field] = prior.clone();
    item.as_object_mut().ok_or("invalid_deduped_body")?.remove(marker);
    Ok(())
}
pub fn restore_item(item: &mut Value, projection: &Value) -> Result<()> {
    match item["type"].as_str().unwrap_or("") {
        "user_message" | "assistant_message" => restore_field(item, source(projection, "messages", &item["messageId"]), "text", "textOmitted"),
        "checkpoint" => restore_field(item, source(projection, "checkpoints", &item["checkpointId"]), "files", "filesOmitted"),
        "subagent" => restore_subagent(item, source(projection, "subagents", &item["subagentId"])),
        _ => Ok(())
    }
}
pub fn restore_subagent(item: &mut Value, prior: Option<&Value>) -> Result<()> {
    restore_field(item, prior, "prompt", "promptOmitted")?;
    restore_field(item, prior, "result", "resultOmitted")
}
pub fn restore_snapshot(value: &mut Value) -> Result<()> {
    expand_ids(value)?;
    let p = &mut value["projection"];
    if !p.is_object() { return Ok(()) }
    // Move the lists out to avoid copying the projection and its message bodies.
    let mut items = p.as_object_mut().unwrap().remove("turnItems").unwrap_or(Value::Null);
    let mut visible = p.as_object_mut().unwrap().remove("visibleTurnItems").unwrap_or(Value::Null);
    for item in items.as_array_mut().into_iter().flatten() { restore_item(item, p)?; }
    for row in visible.as_array_mut().into_iter().flatten() {
        if row["visibility"] == "local" || row["sourceThreadId"] == p["thread"]["id"] { restore_item(&mut row["item"], p)?; }
    }
    if !items.is_null() { p["turnItems"] = items; }
    if !visible.is_null() { p["visibleTurnItems"] = visible; }
    Ok(())
}
