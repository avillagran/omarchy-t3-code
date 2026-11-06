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

fn future_record(value: &serde_json::Map<String, Value>) -> bool {
    value.get("version") == Some(&json!(1))
        && value.get("contextId").is_some_and(Value::is_string)
        && value.contains_key("payload")
        && value
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| {
                !matches!(
                    kind,
                    "image"
                        | "file"
                        | "terminal"
                        | "element"
                        | "preview-annotation"
                        | "review-comment"
                        | "mention"
                        | "skill"
                        | "thread"
                        | "environment"
                )
            })
}
fn array_index(key: &str) -> Option<u32> {
    let index = key.parse::<u32>().ok()?;
    (index != u32::MAX && index.to_string() == key).then_some(index)
}
fn payload_keys(map: &serde_json::Map<String, Value>) -> Vec<&String> {
    let mut indexed = map
        .keys()
        .filter_map(|key| array_index(key).map(|index| (index, key)))
        .collect::<Vec<_>>();
    indexed.sort_by_key(|(index, _)| *index);
    indexed
        .into_iter()
        .map(|(_, key)| key)
        .chain(map.keys().filter(|key| array_index(key).is_none()))
        .collect()
}

/// Object field order is ordinarily structural. Unknown native context payloads
/// are the exception: their JSON.stringify text is sent to the provider.
pub fn semantic_equal(left: &Value, right: &Value) -> bool {
    fn equal(left: &Value, right: &Value, payload: bool) -> bool {
        match (left, right) {
            (Value::Array(a), Value::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b, payload))
            }
            (Value::Object(a), Value::Object(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                if payload {
                    let ak = payload_keys(a);
                    let bk = payload_keys(b);
                    ak == bk && ak.into_iter().all(|key| equal(&a[key], &b[key], true))
                } else {
                    let future = future_record(a) || future_record(b);
                    a.iter().all(|(key, value)| {
                        b.get(key)
                            .is_some_and(|other| equal(value, other, future && key == "payload"))
                    })
                }
            }
            _ => left == right,
        }
    }
    equal(left, right, false)
}

fn canonical_value(value: &Value, payload: bool) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.iter().map(|v| canonical_value(v, payload)).collect())
        }
        Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            let non_index = keys
                .iter()
                .filter(|key| array_index(key).is_none())
                .copied()
                .collect::<Vec<_>>();
            let non_index_sorted = non_index.windows(2).all(|pair| pair[0] <= pair[1]);
            if !payload || non_index_sorted {
                keys.sort();
            } else {
                // Integer property ordering is not user intent. Keep its legacy
                // deterministic representation while retaining non-index order.
                let mut indexed = keys
                    .iter()
                    .filter(|key| array_index(key).is_some())
                    .copied()
                    .collect::<Vec<_>>();
                indexed.sort();
                indexed.extend(non_index);
                keys = indexed;
            }
            let future = !payload && future_record(map);
            Value::Object(
                keys.into_iter()
                    .map(|key| {
                        (
                            key.clone(),
                            canonical_value(&map[key], payload || (future && key == "payload")),
                        )
                    })
                    .collect(),
            )
        }
        _ => value.clone(),
    }
}
/// Preserve pre-ordered-context hashes for structural data and legacy sorted
/// payloads, without erasing meaningful non-index ordering in a new payload.
pub fn canonical_json(value: &Value) -> String {
    canonical_value(value, false).to_string()
}
pub fn stable_hash(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(canonical_json(value).as_bytes()))
}
pub fn context_order_proof(value: &Value) -> Option<String> {
    let current = canonical_json(value);
    let mut legacy = value.clone();
    legacy.sort_all_objects();
    let legacy_json = legacy.to_string();
    (current != legacy_json).then(|| {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(current.as_bytes()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structural_hashes_keep_the_old_format_but_future_payload_order_is_semantic() {
        let ordinary: Value = serde_json::from_str(r#"{"z":[{"b":2,"a":1}],"a":true}"#).unwrap();
        assert_eq!(
            canonical_json(&ordinary),
            r#"{"a":true,"z":[{"a":1,"b":2}]}"#
        );
        let record = |payload: &str| {
            let mut value =
                json!({"version":1,"kind":"future-kind","contextId":"future","label":"Future"});
            value["payload"] = serde_json::from_str(payload).unwrap();
            json!({"version":1,"records":[value]})
        };
        let first = record(r#"{"z":1,"a":{"innerZ":0,"innerA":1},"2":"two","1":"one"}"#);
        let integer_reorder = record(r#"{"1":"one","2":"two","z":1,"a":{"innerZ":0,"innerA":1}}"#);
        assert!(semantic_equal(&first, &integer_reorder));
        assert_eq!(stable_hash(&first), stable_hash(&integer_reorder));
        let changed = record(r#"{"1":"one","2":"two","a":{"innerZ":0,"innerA":1},"z":1}"#);
        assert!(!semantic_equal(&first, &changed));
        assert_ne!(stable_hash(&first), stable_hash(&changed));
        let nested = record(r#"{"1":"one","2":"two","z":1,"a":{"innerA":1,"innerZ":0}}"#);
        assert!(!semantic_equal(&first, &nested));
        assert!(context_order_proof(&first).is_some());
        let legacy = record(r#"{"1":"one","10":"ten","2":"two","a":1,"z":0}"#);
        let mut sorted = legacy.clone();
        sorted.sort_all_objects();
        assert_eq!(canonical_json(&legacy), sorted.to_string());
        assert!(context_order_proof(&legacy).is_none());
    }

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
