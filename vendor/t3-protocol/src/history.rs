//! Opaque native history cursors and portable conversion of projected page rows.
use serde_json::{json, Value};
pub fn path(thread: &str, cursor: &str) -> Result<String, String> {
    if thread.is_empty()
        || cursor.is_empty()
        || cursor.len() > 16384
        || cursor.chars().any(char::is_control)
    {
        return Err("invalid_history_cursor".into());
    }
    let mut url = reqwest::Url::parse("http://localhost/").unwrap();
    url.path_segments_mut()
        .unwrap()
        .extend(["api", "orchestration", "threads", thread, "history"]);
    url.query_pairs_mut().append_pair("cursor", cursor).append_pair("compactIds", "1");
    Ok(format!("{}?{}", url.path(), url.query().unwrap()))
}
/// Only presentation data is reconstructed. Historical rows never replace live
/// control state (requests, queue, runs, provider sessions or permissions).
pub fn envelope(page: &Value, thread: &str) -> Result<Value, String> {
    let rows = page["items"].as_array().ok_or("malformed_history")?;
    if !page["hasMoreHistory"].is_boolean()
        || !(page["nextCursor"].is_string() || page["nextCursor"].is_null())
    {
        return Err("malformed_history".into());
    }
    let mut items = Vec::new();
    let mut messages = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for row in rows {
        let item = &row["item"];
        let source = row["sourceThreadId"].as_str().ok_or("malformed_history")?;
        let id = row["sourceItemId"].as_str().ok_or("malformed_history")?;
        if !ids.insert((source, id)) {
            continue;
        }
        let local = source == thread;
        let mut native = item.clone();
        let time = item
            .get("createdAt")
            .filter(|v| !v.is_null())
            .or_else(|| item.get("startedAt").filter(|v| !v.is_null()))
            .unwrap_or(&item["updatedAt"])
            .clone();
        native["createdAt"] = time.clone();
        if !local {
            native["id"] = json!(format!("{source}/{id}"));
        }
        match item["type"].as_str() {
            Some("user_message" | "assistant_message") => {
                let message_id = item["messageId"].as_str().ok_or("malformed_history")?;
                messages.push(json!({"id":if local {message_id.to_string()} else {format!("{source}/{message_id}")},"role":if item["type"]=="user_message" {"user"} else {"assistant"},"text":item["text"],"attachments":item["attachments"].as_array().cloned().unwrap_or_default(),"createdAt":time,"streaming":false}));
            }
            _ => {}
        }
        items.push(native);
    }
    Ok(
        json!({"projection":{"thread":{"id":thread},"messages":messages,"turnItems":items},"historyCursor":page["nextCursor"],"hasMoreHistory":page["hasMoreHistory"]}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_page_preserves_refs_attachments_and_source_identity() {
        assert_eq!(
            path("chat", "opaque+/=?&").unwrap(),
            "/api/orchestration/threads/chat/history?cursor=opaque%2B%2F%3D%3F%26&compactIds=1"
        );
        let row = json!({"sourceThreadId":"parent","sourceItemId":"one","item":{"id":"one","type":"user_message","messageId":"m","text":"hello","startedAt":"2026-01-01T00:00:00Z","attachments":[{"id":"img"}]}});
        let e = envelope(
            &json!({"items":[row,row],"nextCursor":"next","hasMoreHistory":true}),
            "chat",
        )
        .unwrap();
        assert_eq!(e["projection"]["messages"].as_array().unwrap().len(), 1);
        assert_eq!(e["projection"]["messages"][0]["id"], "parent/m");
        assert_eq!(
            e["projection"]["messages"][0]["attachments"][0]["id"],
            "img"
        );
        assert_eq!(e["historyCursor"], "next");
        assert!(e["projection"].get("runtimeRequests").is_none());
    }
}

impl crate::Connection {
    /// Authenticated bounded history read, with compact ids expanded by transport.
    pub fn history_page(&self,thread:&str,cursor:&str)->crate::Result<Value>{
        crate::api::ids::id("threadId",thread)?;
        self.request_json_with_limit("GET",&path(thread,cursor)?,None,true,8*1024*1024)
    }
}
