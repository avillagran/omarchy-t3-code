//! `mcpApps.*`: the MCP app embedded in a completed tool call talks to its server.
//! `documents::mcp_app` builds the app reference from the turn item.
use super::reader::put;
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

const NAME_MAX: usize = 1024;
const URI_MAX: usize = 4096;

pub const CALL_TOOL: Function = api_fn! {
    name: "mcpApps.callTool", module: "mcpApps", tag: Some("mcpApps.callTool"),
    kind: UnrepeatableMutation, scope: Fixed("orchestration:operate"),
    args: [(threadId, Id, required), (itemId, Id, required), (name, Text, required), (arguments, Object, required)],
    payload: |a| call_tool_payload(a.req("threadId")?, a.req("itemId")?, a.req("name")?, a.req_object("arguments")?),
};
pub const READ_RESOURCE: Function = api_fn! {
    name: "mcpApps.readResource", module: "mcpApps", tag: Some("mcpApps.readResource"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(threadId, Id, required), (itemId, Id, required), (uri, Text, required)],
    payload: |a| read_resource_payload(a.req("threadId")?, a.req("itemId")?, a.req("uri")?),
};
pub const UPDATE_MODEL_CONTEXT: Function = api_fn! {
    name: "mcpApps.updateModelContext", module: "mcpApps", tag: Some("mcpApps.updateModelContext"),
    kind: IdempotentMutation, scope: Fixed("orchestration:operate"),
    args: [(threadId, Id, required), (itemId, Id, required), (conversationThreadId, Id, required), (content, Array, optional), (structuredContent, Object, optional)],
    payload: |a| update_model_context_payload(a.req("threadId")?, a.req("itemId")?, a.req("conversationThreadId")?, a.array("content")?.map(Vec::as_slice), a.object("structuredContent")?),
};
pub const FUNCTIONS: &[Function] = &[CALL_TOOL, READ_RESOURCE, UPDATE_MODEL_CONTEXT];

fn item_payload(thread_id: &str, item_id: &str) -> Result<Value> {
    Ok(json!({"threadId":ids::id("threadId", thread_id)?,"itemId":ids::id("itemId", item_id)?}))
}
/// `mcpApps.callTool`: `{content[], structuredContent?, isError?, _meta?}`. The tool's
/// effect is external: never repeat it.
pub fn call_tool(conn: &Connection, thread_id: &str, item_id: &str, name: &str, arguments: &Value) -> Result<Value> {
    send(conn, &CALL_TOOL, call_tool_payload(thread_id, item_id, name, arguments)?)
}
fn call_tool_payload(thread_id: &str, item_id: &str, name: &str, arguments: &Value) -> Result<Value> {
    if !arguments.is_object() {
        return Err(ids::invalid("arguments"));
    }
    let mut v = item_payload(thread_id, item_id)?;
    v["name"] = json!(ids::text("name", name, NAME_MAX)?);
    v["arguments"] = arguments.clone();
    Ok(v)
}
/// `mcpApps.readResource`: `{contents[]}`.
pub fn read_resource(conn: &Connection, thread_id: &str, item_id: &str, uri: &str) -> Result<Value> {
    send(conn, &READ_RESOURCE, read_resource_payload(thread_id, item_id, uri)?)
}
fn read_resource_payload(thread_id: &str, item_id: &str, uri: &str) -> Result<Value> {
    let mut v = item_payload(thread_id, item_id)?;
    v["uri"] = json!(ids::text("uri", uri, URI_MAX)?);
    Ok(v)
}
/// `mcpApps.updateModelContext`: `null`. Each update replaces the previous one;
/// no content clears it.
pub fn update_model_context(
    conn: &Connection,
    thread_id: &str,
    item_id: &str,
    conversation_thread_id: &str,
    content: Option<&[Value]>,
    structured_content: Option<&Value>,
) -> Result<Value> {
    send(conn, &UPDATE_MODEL_CONTEXT, update_model_context_payload(thread_id, item_id, conversation_thread_id, content, structured_content)?)
}
fn update_model_context_payload(
    thread_id: &str,
    item_id: &str,
    conversation_thread_id: &str,
    content: Option<&[Value]>,
    structured_content: Option<&Value>,
) -> Result<Value> {
    let mut v = item_payload(thread_id, item_id)?;
    v["conversationThreadId"] = json!(ids::id("conversationThreadId", conversation_thread_id)?);
    put(&mut v, "content", content.map(|c| json!(c)));
    if let Some(s) = structured_content {
        if !s.is_object() {
            return Err(ids::invalid("structuredContent"));
        }
        v["structuredContent"] = s.clone();
    }
    Ok(v)
}
