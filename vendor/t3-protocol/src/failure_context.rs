//! Why a run failed, in the tool's own words: the failed tool items of a run
//! (T3 shows them in the timeline as "Failed to …" with their input and
//! output). Bounded thread reads omit tool output, so each failed item is read
//! once through orchestration.getTurnItem (by the host), and only a short excerpt is kept.
use crate::util::*;
use serde_json::{json, Value};

const DETAIL_CHARS: usize = 1500;
const INPUT_CHARS: usize = 300;
const MAX_TOOLS: usize = 3;
const SKIP: [&str; 9] = ["error", "reasoning", "assistant_message", "user_message", "checkpoint", "todo_list", "system_notice", "proposed_plan", "compaction"];

fn clip(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        return s.to_string();
    }
    s.chars().take(n).collect::<String>() + "…"
}

/// Text of one output value: plain strings, ACP/MCP content blocks or a command's streams.
fn output_text(v: &Value, depth: usize) -> String {
    if depth > 4 {
        return String::new();
    }
    match v {
        Value::String(s) => {
            // Command output may be a JSON {stdout,stderr} string.
            if let Some(o) = serde_json::from_str::<Value>(s.trim_start()).ok().filter(|o| o["stdout"].is_string() || o["stderr"].is_string()) {
                return [text(&o, "stderr"), text(&o, "stdout")].into_iter().filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n");
            }
            s.clone()
        }
        Value::Array(list) => list.iter().map(|x| output_text(x, depth + 1)).filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n"),
        Value::Object(_) => {
            for k in ["text", "message", "error", "stderr"] {
                if let Some(s) = v[k].as_str().filter(|s| !s.trim().is_empty()) {
                    return s.to_string();
                }
            }
            for k in ["content", "output", "result"] {
                let t = output_text(&v[k], depth + 1);
                if !t.trim().is_empty() {
                    return t;
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

/// What the tool reported: its output, else a failed edit's message.
pub fn detail(item: &Value) -> String {
    let mut t = output_text(&item["output"], 0);
    if t.trim().is_empty() {
        t = text(item, "diffStr").to_string();
    }
    clip(&t, DETAIL_CHARS)
}

/// The one argument that says what the tool was doing (T3 shows the url, path or command).
pub fn input_summary(item: &Value) -> String {
    let i = &item["input"];
    let raw = if let Some(s) = i.as_str() {
        s.to_string()
    } else {
        ["url", "path", "file_path", "command", "cmd", "pattern", "query"].iter().find_map(|k| i[*k].as_str().map(|s| format!("{k} {s}"))).unwrap_or_default()
    };
    // Some ACP harnesses send their own row title as the "input".
    if raw == text(item, "title") {
        return String::new();
    }
    clip(&raw, INPUT_CHARS)
}

fn failed(item: &Value) -> bool {
    item["status"] == "failed" || item["outputIndicatesFailure"] == true
}

/// The run's failed or cancelled tool items, the last few, newest last.
pub fn candidates<'a>(items: &'a [Value], run: &str) -> Vec<&'a Value> {
    let tools: Vec<&Value> = items
        .iter()
        .map(|r| r.get("item").unwrap_or(r))
        .filter(|i| i["runId"] == run && !SKIP.contains(&text(i, "type")))
        .collect();
    let mut hit: Vec<&Value> = tools.iter().copied().filter(|i| failed(i) || matches!(text(i, "status"), "cancelled" | "interrupted" | "stopped" | "declined")).collect();
    hit.sort_by_key(|i| i["ordinal"].as_i64().unwrap_or(0));
    let skip = hit.len().saturating_sub(MAX_TOOLS);
    hit.into_iter().skip(skip).collect()
}

/// The bounded view of one failed tool item.
pub fn present(item: &Value) -> Value {
    let name = text(item, "toolName");
    let title = text(item, "title");
    let label = if !title.is_empty() && title != "Tool" { title } else if !name.is_empty() { name } else { text(item, "type") };
    json!({"itemId":item["id"],"type":item["type"],"status":item["status"],"failed":failed(item),
        "label":clip(label,180),"toolName":clip(name,180),"input":input_summary(item),"detail":detail(item)})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_tools_of_the_run_with_their_words() {
        let items = vec![
            json!({"item":{"id":"a","runId":"r","type":"dynamic_tool","status":"completed","ordinal":1}}),
            json!({"item":{"id":"b","runId":"r","type":"dynamic_tool","status":"failed","ordinal":2,"toolName":"t3-code.preview_open","title":null,"input":{"url":"https://demo"},
                "output":"Preview automation open failed: AppArmor blocks the sandbox."}}),
            json!({"item":{"id":"c","runId":"other","type":"command_execution","status":"failed","ordinal":3}}),
            json!({"item":{"id":"d","runId":"r","type":"error","status":"failed","ordinal":4}}),
        ];
        let hit = candidates(&items, "r");
        assert_eq!(hit.len(), 1);
        let p = present(hit[0]);
        assert_eq!(p["label"], "t3-code.preview_open");
        assert_eq!(p["input"], "url https://demo");
        assert!(p["detail"].as_str().unwrap().contains("AppArmor"));
    }
    #[test]
    fn content_blocks_and_command_streams_become_text() {
        let acp = json!({"output":[{"content":{"text":"Read failed: File not found","type":"text"},"type":"content"}]});
        assert_eq!(detail(&acp), "Read failed: File not found");
        let cmd = json!({"output":"{\"stdout\":\"\",\"stderr\":\"boom\",\"interrupted\":false}"});
        assert_eq!(detail(&cmd), "boom");
        let edit = json!({"diffStr":"write_file failed"});
        assert_eq!(detail(&edit), "write_file failed");
        assert_eq!(input_summary(&json!({"title":"Ran command","input":"Ran command"})), "");
    }
    #[test]
    fn cancelled_tools_count_as_causes() {
        let items = vec![json!({"id":"x","runId":"r","type":"dynamic_tool","status":"cancelled","ordinal":1,"title":"browser_exec"})];
        assert_eq!(candidates(&items, "r").len(), 1);
    }
}
