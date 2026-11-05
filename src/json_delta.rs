//! Structural deltas for the JSON streams the watch process sends to the panel.
//!
//! A stream sends its first value whole; later values travel as a patch against
//! the previous one. Unchanged subtrees are not sent, so the panel parses only
//! what changed and keeps the identity of the objects it already holds (row
//! models and caches skip work for them).
//!
//! Patch nodes:
//! - `{"$": value}` replaces the value.
//! - `{"o": {key: node}, "r": [key]}` patches an object: `o` changes or adds
//!   members, `r` removes them (either may be absent).
//! - `{"k": field, "a": [entry]}` rebuilds an array of objects identified by
//!   `field`. An entry is the identifier of an unchanged previous element, or
//!   `{"i": identifier, "p": node}` for a changed one, or `{"$": value}` for a
//!   new one.
//!
//! The panel applies patches in the same way (`ui/js/JsonDelta.js`). A patch
//! names the sequence it applies to; on a gap the panel asks for the whole value
//! again (`stream-resync`), and the stream sends it with its next frame.
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

const KEY_FIELDS: [&str; 2] = ["key", "id"];

fn identity(v: &Value, field: &str) -> Option<String> {
    raw_identity(v.get(field)?)
}

fn raw_identity(raw: &Value) -> Option<String> {
    match raw {
        Value::String(s) => Some(format!("s{s}")),
        Value::Number(n) => Some(format!("n{n}")),
        _ => None,
    }
}

/// The field that identifies every element of both arrays uniquely, if any.
fn key_field(prev: &[Value], next: &[Value]) -> Option<&'static str> {
    if prev.is_empty() && next.is_empty() {
        return None;
    }
    KEY_FIELDS.into_iter().find(|field| {
        [prev, next].iter().all(|items| {
            let mut seen = HashSet::with_capacity(items.len());
            items.iter().all(|v| identity(v, field).is_some_and(|id| seen.insert(id)))
        })
    })
}

fn replace(v: &Value) -> Value {
    json!({ "$": v })
}

/// The patch that turns `prev` into `next`, or `None` when they are equal.
pub fn diff(prev: &Value, next: &Value) -> Option<Value> {
    match (prev, next) {
        (Value::Object(a), Value::Object(b)) => {
            let mut set = Map::new();
            for (k, v) in b {
                match a.get(k) {
                    Some(p) => {
                        if let Some(d) = diff(p, v) {
                            set.insert(k.clone(), d);
                        }
                    }
                    None => {
                        set.insert(k.clone(), replace(v));
                    }
                }
            }
            let removed: Vec<Value> = a.keys().filter(|k| !b.contains_key(*k)).map(|k| json!(k)).collect();
            if set.is_empty() && removed.is_empty() {
                return None;
            }
            let mut node = Map::new();
            if !set.is_empty() {
                node.insert("o".into(), Value::Object(set));
            }
            if !removed.is_empty() {
                node.insert("r".into(), Value::Array(removed));
            }
            Some(Value::Object(node))
        }
        (Value::Array(a), Value::Array(b)) => {
            let Some(field) = key_field(a, b) else {
                return (a != b).then(|| replace(next));
            };
            let by_id: HashMap<String, &Value> = a.iter().filter_map(|v| Some((identity(v, field)?, v))).collect();
            let mut changed = a.len() != b.len();
            let mut entries = Vec::with_capacity(b.len());
            for (i, v) in b.iter().enumerate() {
                let id = identity(v, field).unwrap_or_default();
                let raw = v[field].clone();
                match by_id.get(&id) {
                    Some(p) => {
                        if !changed && identity(&a[i], field).as_deref() != Some(id.as_str()) {
                            changed = true;
                        }
                        match diff(p, v) {
                            Some(d) => {
                                changed = true;
                                entries.push(json!({ "i": raw, "p": d }));
                            }
                            None => entries.push(raw),
                        }
                    }
                    None => {
                        changed = true;
                        entries.push(replace(v));
                    }
                }
            }
            changed.then(|| json!({ "k": field, "a": entries }))
        }
        _ => (prev != next).then(|| replace(next)),
    }
}

/// Applies a patch produced by [`diff`]. Mirrors `JsonDelta.apply` in the panel.
pub fn apply(prev: &Value, patch: &Value) -> Result<Value, &'static str> {
    if let Some(v) = patch.get("$") {
        return Ok(v.clone());
    }
    if let Some(field) = patch.get("k").and_then(Value::as_str) {
        let items = prev.as_array().ok_or("delta_base_not_array")?;
        let by_id: HashMap<String, &Value> = items.iter().filter_map(|v| Some((identity(v, field)?, v))).collect();
        let lookup = |raw: &Value| -> Result<&Value, &'static str> {
            let id = raw_identity(raw).ok_or("delta_bad_identity")?;
            by_id.get(&id).copied().ok_or("delta_missing_element")
        };
        let mut out = Vec::new();
        for entry in patch["a"].as_array().ok_or("delta_bad_array")? {
            if let Some(v) = entry.get("$") {
                out.push(v.clone());
            } else if entry.is_object() {
                out.push(apply(lookup(&entry["i"])?, &entry["p"])?);
            } else {
                out.push(lookup(entry)?.clone());
            }
        }
        return Ok(Value::Array(out));
    }
    let mut out = prev.as_object().cloned().ok_or("delta_base_not_object")?;
    for (k, node) in patch.get("o").and_then(Value::as_object).into_iter().flatten() {
        let next = apply(out.get(k).unwrap_or(&Value::Null), node)?;
        out.insert(k.clone(), next);
    }
    for k in patch.get("r").and_then(Value::as_array).into_iter().flatten() {
        out.remove(k.as_str().unwrap_or_default());
    }
    Ok(Value::Object(out))
}

fn registry() -> &'static Mutex<HashMap<u64, Weak<AtomicBool>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<u64, Weak<AtomicBool>>>> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

/// Asks stream `id` to send its next value whole. Unknown streams are ignored.
pub fn request_resync(id: u64) {
    let mut streams = registry().lock().unwrap_or_else(|e| e.into_inner());
    streams.retain(|_, flag| flag.strong_count() > 0);
    if let Some(flag) = streams.get(&id).and_then(Weak::upgrade) {
        flag.store(true, Ordering::Relaxed);
    }
}

/// One delta-encoded stream. Every frame carries the stream id and a sequence;
/// a patch frame also names the sequence it applies to.
pub struct Stream {
    id: u64,
    seq: u64,
    last: Option<Value>,
    resync: Arc<AtomicBool>,
}

impl Default for Stream {
    fn default() -> Self {
        Self::new()
    }
}

impl Stream {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let resync = Arc::new(AtomicBool::new(false));
        let mut streams = registry().lock().unwrap_or_else(|e| e.into_inner());
        streams.retain(|_, flag| flag.strong_count() > 0);
        streams.insert(id, Arc::downgrade(&resync));
        Self { id, seq: 0, last: None, resync }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    /// The next frame sends the whole value.
    pub fn reset(&mut self) {
        self.last = None;
    }
    /// Envelope fields for `value`: `{stream, seq, data}` or `{stream, seq, base, patch}`.
    pub fn frame(&mut self, value: Value) -> Value {
        let resync = self.resync.swap(false, Ordering::Relaxed);
        let base = self.seq;
        self.seq += 1;
        let frame = match self.last.as_ref().filter(|_| !resync) {
            Some(last) => {
                let patch = diff(last, &value).unwrap_or_else(|| json!({}));
                json!({ "stream": self.id, "seq": self.seq, "base": base, "patch": patch })
            }
            None => json!({ "stream": self.id, "seq": self.seq, "data": value }),
        };
        self.last = Some(value);
        frame
    }
}

/// Rebuilds whole values from stream frames, as the panel does. Lines that are
/// not stream frames pass through unchanged.
#[derive(Default)]
pub struct Reader {
    bases: HashMap<u64, (u64, Value)>,
}

impl Reader {
    pub fn resolve(&mut self, mut frame: Value) -> Result<Value, &'static str> {
        let Some(id) = frame["stream"].as_u64() else { return Ok(frame) };
        let seq = frame["seq"].as_u64().ok_or("delta_missing_seq")?;
        let data = match frame.get("patch") {
            Some(patch) => {
                let (base_seq, base) = self.bases.get(&id).ok_or("delta_missing_base")?;
                if frame["base"].as_u64() != Some(*base_seq) {
                    return Err("delta_sequence_gap");
                }
                apply(base, patch)?
            }
            None => frame["data"].clone(),
        };
        self.bases.insert(id, (seq, data.clone()));
        frame["data"] = data;
        Ok(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(a: Value, b: Value) -> Option<Value> {
        let patch = diff(&a, &b);
        let applied = match &patch {
            Some(p) => apply(&a, p).unwrap(),
            None => a.clone(),
        };
        assert_eq!(applied, b);
        patch
    }

    #[test]
    fn equal_values_have_no_patch() {
        assert!(round_trip(json!({"a": [1, 2], "b": {"c": null}}), json!({"a": [1, 2], "b": {"c": null}})).is_none());
    }

    #[test]
    fn object_members_are_patched_added_and_removed() {
        let patch = round_trip(json!({"a": 1, "b": {"c": 1, "d": 2}, "gone": true}), json!({"a": 1, "b": {"c": 3, "d": 2}, "new": "x"})).unwrap();
        assert_eq!(patch, json!({"o": {"b": {"o": {"c": {"$": 3}}}, "new": {"$": "x"}}, "r": ["gone"]}));
    }

    #[test]
    fn keyed_arrays_send_only_changed_rows() {
        let a = json!([{"key": "e/1", "id": "1", "t": "a"}, {"key": "e/2", "id": "2", "t": "b"}, {"key": "f/1", "id": "1", "t": "c"}]);
        let b = json!([{"key": "f/1", "id": "1", "t": "c"}, {"key": "e/2", "id": "2", "t": "B"}, {"key": "g/9", "id": "9"}]);
        let patch = round_trip(a, b).unwrap();
        // `id` repeats across hosts, so `key` identifies the rows.
        assert_eq!(patch, json!({"k": "key", "a": ["f/1", {"i": "e/2", "p": {"o": {"t": {"$": "B"}}}}, {"$": {"key": "g/9", "id": "9"}}]}));
    }

    #[test]
    fn reordering_alone_is_a_change() {
        let patch = round_trip(json!([{"id": 1}, {"id": 2}]), json!([{"id": 2}, {"id": 1}])).unwrap();
        assert_eq!(patch, json!({"k": "id", "a": [2, 1]}));
    }

    #[test]
    fn arrays_without_unique_identity_are_replaced() {
        assert_eq!(round_trip(json!([{"id": 1}, {"id": 1}]), json!([{"id": 1}])).unwrap(), json!({"$": [{"id": 1}]}));
        assert_eq!(round_trip(json!([1, 2]), json!([1, 3])).unwrap(), json!({"$": [1, 3]}));
        round_trip(json!([]), json!([{"id": "x"}]));
        round_trip(json!([{"id": "x"}]), json!([]));
    }

    #[test]
    fn type_changes_replace() {
        round_trip(json!({"a": [1]}), json!({"a": {"b": 1}}));
        round_trip(json!({"a": null}), json!({"a": [{"id": 1}]}));
    }

    #[test]
    fn apply_rejects_a_patch_for_another_base() {
        let patch = diff(&json!([{"id": 1}]), &json!([{"id": 1, "x": 2}])).unwrap();
        assert!(apply(&json!([{"id": 3}]), &patch).is_err());
        assert!(apply(&json!({"id": 1}), &patch).is_err());
    }

    #[test]
    fn stream_sends_whole_value_then_patches_and_resyncs_on_request() {
        let mut s = Stream::new();
        let first = s.frame(json!({"threads": [{"key": "a", "n": 1}]}));
        assert_eq!(first, json!({"stream": s.id(), "seq": 1, "data": {"threads": [{"key": "a", "n": 1}]}}));
        let second = s.frame(json!({"threads": [{"key": "a", "n": 2}]}));
        assert_eq!(second["base"], 1);
        assert_eq!(second["seq"], 2);
        assert_eq!(second["patch"], json!({"o": {"threads": {"k": "key", "a": [{"i": "a", "p": {"o": {"n": {"$": 2}}}}]}}}));
        let same = s.frame(json!({"threads": [{"key": "a", "n": 2}]}));
        assert_eq!(same["patch"], json!({}));
        request_resync(s.id());
        let whole = s.frame(json!({"threads": []}));
        assert_eq!(whole["seq"], 4);
        assert!(whole.get("patch").is_none());
        assert_eq!(whole["data"], json!({"threads": []}));
        s.reset();
        assert!(s.frame(json!({})).get("data").is_some());
    }

    #[test]
    fn reader_rebuilds_values_and_detects_gaps() {
        let mut s = Stream::new();
        let mut r = Reader::default();
        assert_eq!(r.resolve(json!({"type": "x", "data": 1})).unwrap()["data"], 1);
        let a = s.frame(json!({"rows": [{"id": 1, "v": "a"}]}));
        assert_eq!(r.resolve(a).unwrap()["data"], json!({"rows": [{"id": 1, "v": "a"}]}));
        let b = s.frame(json!({"rows": [{"id": 1, "v": "b"}, {"id": 2}]}));
        assert_eq!(r.resolve(b).unwrap()["data"], json!({"rows": [{"id": 1, "v": "b"}, {"id": 2}]}));
        let _lost = s.frame(json!({"rows": []}));
        let c = s.frame(json!({"rows": [{"id": 3}]}));
        assert_eq!(r.resolve(c).unwrap_err(), "delta_sequence_gap");
    }

    #[test]
    fn large_sidebar_patch_is_small() {
        let rows = |n: u64| -> Value { json!({"threads": (0..300).map(|i| json!({"key": format!("e/{i}"), "id": format!("{i}"), "title": "x".repeat(200), "updatedAt": if i == 7 { n } else { 0 }})).collect::<Vec<_>>()}) };
        let (a, b) = (rows(1), rows(2));
        let patch = round_trip(a.clone(), b).unwrap();
        assert!(patch.to_string().len() * 20 < a.to_string().len(), "{}", patch.to_string().len());
    }
}
