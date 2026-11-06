//! Native structured composer records retained alongside their inline references.
use crate::{text, Result};
use serde_json::{json, Value};
use std::collections::HashSet;

// Queue editing can retain current, frozen, original queued and backup contexts.
// Each keeps its own native 16m-UTF16/200-record limit; ordinary fields stay small.
pub const MAX_DRAFT_BYTES: usize = 200 * 1024 * 1024;

struct JsonUnits(usize);
impl std::io::Write for JsonUnits {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let text = std::str::from_utf8(bytes).map_err(std::io::Error::other)?;
        self.0 = self.0.saturating_add(text.encode_utf16().count());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn serialized_units(value: &impl serde::Serialize) -> Result<usize> {
    let mut count = JsonUnits(0);
    serde_json::to_writer(&mut count, value).map_err(|_| "invalid_message_context")?;
    Ok(count.0)
}
struct JsonBytes(usize);
impl std::io::Write for JsonBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub fn serialized_bytes(value: &Value) -> Result<usize> {
    let mut count = JsonBytes(0);
    serde_json::to_writer(&mut count, value).map_err(|_| "invalid_message_context")?;
    Ok(count.0)
}

// Only native context slots receive the larger allowance. Ordinary text, file
// metadata, answers and legacy contexts retain the original aggregate budget.
pub fn ordinary_draft(value: &Value) -> Value {
    let Some(map) = value.as_object() else {
        return value.clone();
    };
    Value::Object(
        map.iter()
            .filter_map(|(key, value)| {
                if key == "context" && value["version"] == 1 {
                    None
                } else {
                    Some((
                        key.clone(),
                        if matches!(key.as_str(), "attempt" | "queueEdit" | "backup") {
                            ordinary_draft(value)
                        } else {
                            value.clone()
                        },
                    ))
                }
            })
            .collect(),
    )
}

fn bounded(value: &Value, limit: usize) -> bool {
    value
        .as_str()
        .is_some_and(|s| s.encode_utf16().count() <= limit)
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

/// Keep forward-compatible record fields. Known terminal records are produced by
/// the native renderer; malformed entries are dropped before duplicate checking.
pub fn clean(value: &Value) -> Result<Value> {
    if value["version"] != 1 {
        return Err("invalid_message_context".into());
    }
    let records = value["records"]
        .as_array()
        .filter(|r| r.len() <= 200)
        .ok_or("invalid_message_context")?;
    if serialized_units(records)? > 16_000_000 {
        return Err("invalid_message_context".into());
    }
    let mut out = Vec::new();
    let mut ids = HashSet::new();
    for record in records {
        let kind = text(record, "kind");
        let id = text(record, "contextId");
        if record["version"] != 1
            || !identifier(id)
            || !bounded(&record["label"], 200)
            || kind.is_empty()
            || kind.len() > 40
            || !kind.as_bytes()[0].is_ascii_lowercase()
            || !kind
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            continue;
        }
        if kind == "terminal" {
            let start = record["lineStart"].as_u64();
            let end = record["lineEnd"].as_u64();
            if start.is_none()
                || end.is_none()
                || end < start
                || !bounded(&record["text"], 64_000)
                || !bounded(&record["terminalId"], 255)
                || text(record, "terminalId").trim().is_empty()
                || !bounded(&record["terminalLabel"], 255)
                || text(record, "terminalLabel").trim().is_empty()
            {
                continue;
            }
        }
        if !ids.insert(id) {
            return Err("invalid_message_context".into());
        }
        out.push(record.clone());
    }
    let mut result = json!({"version":1,"records":out});
    if let Some(guest) = value.get("guest") {
        result["guest"] = guest.clone();
    }
    Ok(result)
}

/// Retain only records still referenced by this message, in their original order.
pub fn for_message(value: &Value, message: &str) -> Result<Value> {
    let mut clean = clean(value)?;
    clean["records"].as_array_mut().unwrap().retain(|r| {
        message.contains(&format!(
            "t3-context://v1/{}/{})",
            text(r, "kind"),
            text(r, "contextId")
        ))
    });
    Ok(clean)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_preserve_exact_text_and_drop_malformed_entries_before_duplicate_check() {
        let terminal = json!({"version":1,"contextId":"terminal_test","kind":"terminal","label":"Terminal 1 lines 2-3","terminalId":"one","terminalLabel":"Terminal 1","lineStart":2,"lineEnd":3,"text":"hello\n世界"});
        let mut invalid = terminal.clone();
        invalid["lineEnd"] = json!(1);
        let input = json!({"version":1,"records":[invalid,terminal.clone()]});
        assert_eq!(clean(&input).unwrap()["records"], json!([terminal.clone()]));
        assert!(
            clean(&json!({"version":1,"records":[terminal.clone(),terminal.clone()]})).is_err()
        );
        assert_eq!(
            for_message(&input, "[log](t3-context://v1/terminal/terminal_test)").unwrap()
                ["records"][0],
            terminal
        );
        assert!(for_message(&input, "unrelated").unwrap()["records"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}
