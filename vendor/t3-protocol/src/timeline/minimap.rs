//! ot3's conversation minimap (`ui/js/TimelineModel.js` and the marker rules of
//! `T3TimelineMinimap.qml`): one marker per user message, paired with the last
//! assistant reply before the next user message.
use crate::js;
use serde_json::{json, Map, Value};
use std::collections::HashSet;

/// `derive(rows)`: `{id, rowIndex, userText, assistantText}` per user row of the
/// timeline. `userText` is `""` and `assistantText` `null` when the row has no text.
pub fn items(rows: &[Value]) -> Vec<Value> {
    let mut items: Vec<Map<String, Value>> = Vec::new();
    let mut current = false;
    for (i, row) in rows.iter().enumerate() {
        if row["role"] == "user" {
            let mut item = Map::new();
            if let Some(id) = row.get("id") {
                item.insert("id".into(), id.clone());
            }
            item.insert("rowIndex".into(), json!(i));
            item.insert("userText".into(), if js::truthy(&row["text"]) { row["text"].clone() } else { json!("") });
            item.insert("assistantText".into(), Value::Null);
            items.push(item);
            current = true;
        } else if current && row["role"] == "assistant" {
            let text = if js::truthy(&row["text"]) { row["text"].clone() } else { Value::Null };
            items.last_mut().unwrap().insert("assistantText".into(), text);
        }
    }
    items.into_iter().map(Value::Object).collect()
}

/// `indexAt(y, height, count)`: the marker under `y` on a rail `height` tall, or -1.
pub fn index_at(y: f64, height: f64, count: i64) -> i64 {
    if count == 0 || height.is_nan() || height <= 0.0 {
        return -1;
    }
    let last = (count - 1) as f64;
    // Math.round on a value that is never negative here: f64::round agrees.
    let at = ((y / height).clamp(0.0, 1.0) * last).round();
    at.min(last).max(0.0) as i64
}

/// `updateViewport`'s markers: the items whose user row is on screen and the
/// current one (the first on screen, else the last at or before the first row
/// on screen, else the last item when the list is at its end with no row on screen).
pub fn position(rows: &[Value], items: &[Value], visible: &HashSet<&str>, at_end: bool) -> (i64, Vec<Value>) {
    let first = rows.iter().position(|r| r["id"].as_str().is_some_and(|id| visible.contains(id))).unwrap_or(rows.len());
    let (mut current, mut preceding, mut markers) = (-1i64, -1i64, Vec::new());
    for (n, item) in items.iter().enumerate() {
        if item["id"].as_str().is_some_and(|id| visible.contains(id)) {
            // A set, as in ot3: an id shows once.
            if !markers.contains(&item["id"]) {
                markers.push(item["id"].clone());
            }
            if current < 0 {
                current = n as i64;
            }
        }
        if item["rowIndex"].as_u64().is_some_and(|i| i as usize <= first) {
            preceding = n as i64;
        }
    }
    if current < 0 {
        current = preceding;
    }
    if at_end && first == rows.len() {
        current = items.len() as i64 - 1;
    }
    (current, markers)
}

/// The minimap of a timeline: its items with their one-line previews
/// (`compact`), whether ot3 would draw it (two items or more) and, given the
/// ids of the rows on screen, the current item and the visible markers.
pub fn view(rows: &[Value], visible_ids: &[Value], at_end: bool) -> Value {
    let mut items = items(rows);
    let visible = visible_ids.iter().filter_map(Value::as_str).collect::<HashSet<_>>();
    let (current, markers) = position(rows, &items, &visible, at_end);
    for item in &mut items {
        item["userPreview"] = json!(js::compact(&item["userText"]));
        item["assistantPreview"] = json!(js::compact(&item["assistantText"]));
    }
    json!({"items":items,"visible":items.len() >= 2,"currentIndex":current,"visibleIds":markers})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pairs_each_user_message_with_its_last_reply() {
        let rows = vec![
            json!({"id":"a0","role":"assistant","text":"before any user"}),
            json!({"id":"u1","role":"user","text":"  first\n\nquestion "}),
            json!({"id":"x","role":"activity"}),
            json!({"id":"a1","role":"assistant","text":"one"}),
            json!({"id":"a2","role":"assistant","text":"two"}),
            json!({"id":"u2","role":"user"}),
            json!({"id":"a3","role":"assistant","text":""}),
        ];
        let v = view(&rows, &[json!("a1")], false);
        assert_eq!(v["items"][0], json!({"id":"u1","rowIndex":1,"userText":"  first\n\nquestion ","assistantText":"two","userPreview":"first question","assistantPreview":"two"}));
        assert_eq!(v["items"][1], json!({"id":"u2","rowIndex":5,"userText":"","assistantText":null,"userPreview":"","assistantPreview":""}));
        assert_eq!((v["visible"].as_bool(), v["currentIndex"].as_i64(), v["visibleIds"].clone()), (Some(true), Some(0), json!([])));
        assert_eq!(view(&rows, &[json!("u2")], false)["currentIndex"], 1);
        assert_eq!(view(&rows, &[], true)["currentIndex"], 1);
        assert_eq!(view(&rows, &[], false)["currentIndex"], 1);
        assert_eq!(view(&rows[..2], &[], false)["visible"], false);
        assert_eq!((index_at(0.0, 100.0, 5), index_at(49.0, 100.0, 5), index_at(500.0, 100.0, 5), index_at(10.0, 0.0, 5), index_at(1.0, 1.0, 0)), (0, 2, 4, -1, -1));
        assert_eq!(index_at(-5.0, 100.0, -3), 0);
    }
}
