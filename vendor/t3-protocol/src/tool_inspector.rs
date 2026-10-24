//! On-demand native turn-item reads. No persistence, polling or mutation.
use crate::util::{text, Result};
use serde_json::{json, Value};
use std::io::{self, Write};

pub const METHOD: &str = "orchestration.getTurnItem";
pub const SECTION_BYTES: usize = 64 * 1024;
pub const RAW_BYTES: usize = 128 * 1024;

fn identifier(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control))
}
fn nullable_identifier(value: Option<&Value>) -> bool {
    value.is_some_and(|v| v.is_null() || identifier(v))
}

/// Add this to EACH compact activity entry before grouping. Never copy item bodies.
/// Projected rows must retain their source identities, especially inherited rows.
pub fn compact_reference(row: &Value) -> Result<Value> {
    let projected = row.get("item").is_some();
    let item = if projected { &row["item"] } else { row };
    let source_thread = if projected {
        &row["sourceThreadId"]
    } else {
        &item["threadId"]
    };
    let source_item = if projected {
        &row["sourceItemId"]
    } else {
        &item["id"]
    };
    if !identifier(source_thread)
        || !identifier(source_item)
        || !identifier(&item["type"])
        || !nullable_identifier(item.get("runId"))
        || !nullable_identifier(item.get("providerTurnId"))
    {
        return Err("invalid_tool_reference".into());
    }
    let revision = if matches!(
        text(item, "status"),
        "idle" | "pending" | "running" | "waiting"
    ) {
        "live"
    } else {
        text(item, "updatedAt")
    };
    if revision.is_empty() || revision.len() > 128 {
        return Err("invalid_tool_reference".into());
    }
    Ok(
        json!({"sourceThreadId":source_thread,"sourceItemId":source_item,
        "type":item["type"],"runId":item["runId"],"providerTurnId":item["providerTurnId"],
        "revision":revision}),
    )
}

/// Local scope includes viewer thread and environment; neither is sent to native RPC.
pub fn scope(input: &Value) -> Result<Value> {
    for key in [
        "environment",
        "threadId",
        "sourceThreadId",
        "sourceItemId",
        "type",
    ] {
        if !identifier(&input[key]) {
            return Err("invalid_tool_reference".into());
        }
    }
    if !nullable_identifier(input.get("runId"))
        || !nullable_identifier(input.get("providerTurnId"))
        || input["revision"]
            .as_str()
            .is_none_or(|s| s.is_empty() || s.len() > 128 || s.chars().any(char::is_control))
    {
        return Err("invalid_tool_reference".into());
    }
    Ok(
        json!({"environment":input["environment"],"threadId":input["threadId"],
        "sourceThreadId":input["sourceThreadId"],"sourceItemId":input["sourceItemId"],
        "type":input["type"],"runId":input["runId"],"providerTurnId":input["providerTurnId"],
        "revision":input["revision"]}),
    )
}

pub fn request(input: &Value) -> Result<(&'static str, Value)> {
    let s = scope(input)?;
    Ok((
        METHOD,
        json!({"threadId":s["sourceThreadId"],"itemId":s["sourceItemId"],"revision":s["revision"]}),
    ))
}

// Stop JSON serialization at the budget instead of allocating an unbounded pretty string.
struct LimitedText {
    bytes: Vec<u8>,
    limit: usize,
    truncated: bool,
}
impl Write for LimitedText {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let n = bytes.len().min(self.limit.saturating_sub(self.bytes.len()));
        self.bytes.extend_from_slice(&bytes[..n]);
        if n < bytes.len() {
            self.truncated = true;
            return Err(io::Error::other("inspector_text_limit"));
        }
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn bounded(value: &Value, limit: usize, raw: bool) -> (String, bool) {
    if !raw {
        if let Some(text) = value.as_str() {
            let mut end = text.len().min(limit);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            return (text[..end].into(), end < text.len());
        }
    }
    let mut out = LimitedText {
        bytes: Vec::with_capacity(limit.min(4096)),
        limit,
        truncated: false,
    };
    if serde_json::to_writer_pretty(&mut out, value).is_err() {
        out.truncated = true;
    }
    // The last write may end in a multibyte code point.
    while std::str::from_utf8(&out.bytes).is_err() {
        out.bytes.pop();
    }
    (
        String::from_utf8(out.bytes).unwrap_or_default(),
        out.truncated,
    )
}

// Same text block vocabulary as native itemDetail.ts; structured fallbacks stay JSON.
fn block_text(value: &Value, depth: usize) -> Option<String> {
    if depth > 4 {
        return None;
    }
    if let Some(s) = value.as_str() {
        return Some(s.into());
    }
    if let Some(list) = value.as_array() {
        let parts = list
            .iter()
            .map(|v| block_text(v, depth + 1))
            .collect::<Option<Vec<_>>>()?;
        return Some(
            parts
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    let object = value.as_object()?;
    match text(value, "type") {
        "text" => return value["text"].as_str().map(str::to_owned),
        "resource_link" => return value["uri"].as_str().map(str::to_owned),
        "resource" => {
            return value["resource"]["text"]
                .as_str()
                .or(value["resource"]["uri"].as_str())
                .map(str::to_owned)
        }
        _ => {}
    }
    let keys = object
        .keys()
        .filter(|k| !matches!(k.as_str(), "isError" | "is_error"))
        .collect::<Vec<_>>();
    if (keys.len() == 1 && object.contains_key("content"))
        || (keys.len() == 2
            && object.contains_key("content")
            && object.contains_key("structuredContent"))
    {
        let result = block_text(&value["content"], depth + 1)?;
        if !result.trim().is_empty() {
            return Some(result);
        }
    }
    None
}
fn output_value(item: &Value) -> Value {
    if item["outputOmitted"] == true {
        return Value::Null;
    }
    let output = &item["output"];
    if item["type"] == "command_execution" {
        if let Some(raw) = output
            .as_str()
            .filter(|s| s.trim_start().starts_with("{\"stdout\""))
        {
            if let Ok(v) = serde_json::from_str::<Value>(raw) {
                if v["stdout"].is_string()
                    && v["stderr"].is_string()
                    && v["interrupted"].is_boolean()
                {
                    return json!([text(&v, "stdout"), text(&v, "stderr")]
                        .into_iter()
                        .filter(|s| !s.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join("\n"));
                }
            }
        }
        return output.clone();
    }
    block_text(output, 0)
        .map(Value::String)
        .unwrap_or_else(|| output.clone())
}
fn section(sections: &mut Vec<Value>, key: &str, label: &str, value: &Value) {
    if value.is_null() {
        return;
    }
    let (body, truncated) = bounded(value, SECTION_BYTES, false);
    if body.trim().is_empty() || body == "{}" || body == "[]" {
        return;
    }
    sections.push(json!({"key":key,"label":label,"text":body,"truncated":truncated}));
}

/// Decode the EXACT native `{ item: TurnItem | null }`, not a guessed output envelope.
/// Identity/type mismatches fail closed; revisions may advance while a request is in flight.
pub fn parse(input: &Value, response: &Value) -> Result<Value> {
    let s = scope(input)?;
    let item = response.get("item").ok_or("malformed_tool_item")?;
    if item.is_null() {
        return Ok(
            json!({"scope":s,"available":false,"sections":[],"raw":"","rawTruncated":false}),
        );
    }
    if !item.is_object()
        || item["id"] != s["sourceItemId"]
        || item["threadId"] != s["sourceThreadId"]
        || item["type"] != s["type"]
        || item.get("runId") != Some(&s["runId"])
        || item.get("providerTurnId") != Some(&s["providerTurnId"])
    {
        return Err("tool_scope_mismatch".into());
    }
    if !item["status"].is_string() || !item["updatedAt"].is_string() {
        return Err("malformed_tool_item".into());
    }
    let mut sections = Vec::new();
    match text(item, "type") {
        "command_execution" | "dynamic_tool" => {
            if item["type"] == "command_execution" && !item["input"].is_string() {
                return Err("malformed_tool_item".into());
            }
            if !item.as_object().is_some_and(|o| o.contains_key("input")) {
                return Err("malformed_tool_item".into());
            }
            section(&mut sections, "input", "Input", &item["input"]);
            section(&mut sections, "output", "Output", &output_value(item));
        }
        "reasoning" => section(&mut sections, "reasoning", "Reasoning", &item["text"]),
        "file_search" => {
            section(&mut sections, "input", "Input", &item["pattern"]);
            section(&mut sections, "results", "Results", &item["results"]);
        }
        "web_search" => {
            section(&mut sections, "input", "Input", &item["patterns"]);
            section(&mut sections, "results", "Results", &item["results"]);
        }
        "file_change" => {
            section(&mut sections, "file", "File", &item["fileName"]);
            section(&mut sections, "changes", "Changes", &item["changes"]);
            if item["status"] == "failed" {
                section(&mut sections, "diff", "Diff", &item["diffStr"]);
            }
        }
        "proposed_plan" => section(&mut sections, "plan", "Plan", &item["markdown"]),
        "todo_list" => section(&mut sections, "steps", "Steps", &item["steps"]),
        "approval_request" => section(&mut sections, "prompt", "Prompt", &item["prompt"]),
        "user_input_request" => {
            section(&mut sections, "questions", "Questions", &item["questions"])
        }
        "notification" => section(&mut sections, "detail", "Details", &item["detail"]),
        "system_notice" | "run_interrupt_request" | "run_interrupt_result" => {
            section(&mut sections, "message", "Message", &item["message"])
        }
        "error" => section(&mut sections, "error", "Error", &item["failure"]),
        "subagent" => {
            section(&mut sections, "prompt", "Prompt", &item["prompt"]);
            section(&mut sections, "progress", "Progress", &item["progress"]);
            section(&mut sections, "result", "Result", &item["result"]);
        }
        _ => {} // Other native types are inspectable in bounded raw mode.
    }
    let metadata = json!({"type":item["type"],"title":item["title"],"toolName":item["toolName"],
        "status":item["status"],"runId":item["runId"],"providerTurnId":item["providerTurnId"],
        "providerThreadId":item["providerThreadId"],"startedAt":item["startedAt"],
        "completedAt":item["completedAt"],"updatedAt":item["updatedAt"],"exitCode":item["exitCode"],
        "outputIndicatesFailure":item["outputIndicatesFailure"],"toolSurface":item["toolSurface"],
        "toolSource":item["toolSource"],"toolNonExecutionKind":item["toolNonExecutionKind"]});
    let (metadata_text, metadata_truncated) = bounded(&metadata, SECTION_BYTES, true);
    let (raw, raw_truncated) = bounded(item, RAW_BYTES, true);
    let (title, _) = bounded(
        &json!(item["toolName"]
            .as_str()
            .or(item["title"].as_str())
            .unwrap_or(text(item, "type"))),
        256,
        false,
    );
    let tool = matches!(text(item, "type"), "command_execution" | "dynamic_tool");
    Ok(
        json!({"scope":s,"available":true,"title":title,"status":bounded(&item["status"],128,false).0,
        "failed":item["status"]=="failed" || item["outputIndicatesFailure"]==true || item["exitCode"].as_i64().is_some_and(|n| n!=0),
        "exitCode":item["exitCode"].as_i64(),"sections":sections,"metadata":metadata_text,
        "metadataTruncated":metadata_truncated,"raw":raw,"rawTruncated":raw_truncated,
        "outputOmitted":item["outputOmitted"]==true,
        "emptyOutput":tool && item["outputOmitted"]!=true && !sections.iter().any(|v|v["key"]=="output")}),
    )
}
