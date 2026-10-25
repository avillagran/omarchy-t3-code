//! `scheduledTasks.*`: recurring and webhook-triggered agent runs. Reads need
//! `orchestration:read` (without `orchestration:operate` the server strips each
//! task's `webhook`); every write needs `orchestration:operate`.
use super::orchestration::{WorkspaceStrategy, ACTORS, CREATION_SOURCES, INTERACTION_MODES};
use super::reader::{int, object, one_of, put, Reader};
use super::{api_fn, ids, send, Function};
use crate::{models, Connection, Result};
use serde_json::{json, Value};
use std::fmt;

/// T3 `MIN_SCHEDULED_TASK_INTERVAL_MS` and `MAX_WEBHOOK_FILTERS`.
const MIN_INTERVAL_MS: u64 = 60_000;
const MAX_WEBHOOK_FILTERS: usize = 16;
const TITLE_MAX: usize = 255;
const PROMPT_MAX: usize = 120_000;

pub const LIST: Function = api_fn! {
    name: "scheduledTasks.list", module: "scheduledTasks", tag: Some("scheduledTasks.list"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [], payload: |_| Ok(json!({})),
};
/// Deduplicated through `commandId`: the server derives the task id
/// `scheduled-task:<commandId>` and upserts it. The library requires `commandId`
/// or `id`, so a resend never creates a second task.
pub const UPSERT: Function = api_fn! {
    name: "scheduledTasks.upsert", module: "scheduledTasks", tag: Some("scheduledTasks.upsert"),
    kind: MutationWithCommandId, scope: Fixed("orchestration:operate"),
    args: [
        (title, Text, required), (prompt, Text, required), (enabled, Bool, required), (schedule, Object, required),
        (projectId, Id, required), (workspaceStrategy, Object, required), (modelSelection, Object, required),
        (runtimeMode, Enum(models::RUNTIME_MODES), required), (interactionMode, Enum(INTERACTION_MODES), required),
        (commandId, Id, optional), (id, Id, optional), (requireExisting, Bool, optional), (threadId, Id, optional),
        (createdBy, Enum(ACTORS), optional), (creationSource, Enum(CREATION_SOURCES), optional),
    ],
    payload: |a| TaskUpsert::from_json(a.all())?.payload(),
};
pub const SET_ENABLED: Function = api_fn! {
    name: "scheduledTasks.setEnabled", module: "scheduledTasks", tag: Some("scheduledTasks.setEnabled"),
    kind: IdempotentMutation, scope: Fixed("orchestration:operate"),
    args: [(id, Id, required), (enabled, Bool, required)],
    payload: |a| set_enabled_payload(a.req("id")?, a.bool("enabled")?.unwrap_or_default()),
};
pub const DELETE: Function = api_fn! {
    name: "scheduledTasks.delete", module: "scheduledTasks", tag: Some("scheduledTasks.delete"),
    kind: IdempotentMutation, scope: Fixed("orchestration:operate"),
    args: [(id, Id, required)],
    payload: |a| id_payload(a.req("id")?),
};
pub const RUN_NOW: Function = api_fn! {
    name: "scheduledTasks.runNow", module: "scheduledTasks", tag: Some("scheduledTasks.runNow"),
    kind: UnrepeatableMutation, scope: Fixed("orchestration:operate"),
    args: [(id, Id, required)],
    payload: |a| id_payload(a.req("id")?),
};
pub const FUNCTIONS: &[Function] = &[LIST, UPSERT, SET_ENABLED, DELETE, RUN_NOW];

/// `scheduledTasks.list`: `{tasks[]}`.
pub fn list(conn: &Connection) -> Result<Value> {
    send(conn, &LIST, json!({}))
}

/// `ScheduledTaskUpsertInput`. `Debug` hides the schedule (a webhook may carry
/// a signing secret).
#[derive(Clone, PartialEq)]
pub struct TaskUpsert {
    pub command_id: Option<String>,
    pub id: Option<String>,
    pub require_existing: Option<bool>,
    pub title: String,
    pub prompt: String,
    pub enabled: bool,
    /// `{type:"interval",everyMs ≥ 60000}`, `{type:"fixed_time",timeOfDay:"HH:MM",weekdays?:[0..6]}`
    /// or `{type:"webhook",signature?,maxDeliveryAgeMinutes?,dedupe?,filters?}`.
    pub schedule: Value,
    pub project_id: String,
    /// `None`: no thread; `Some(None)`: `null`.
    pub thread_id: Option<Option<String>>,
    pub workspace_strategy: WorkspaceStrategy,
    /// `ModelSelection` object.
    pub model_selection: Value,
    pub runtime_mode: String,
    pub interaction_mode: String,
    pub created_by: Option<String>,
    pub creation_source: Option<String>,
}
impl fmt::Debug for TaskUpsert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaskUpsert")
            .field("command_id", &self.command_id)
            .field("id", &self.id)
            .field("title", &self.title)
            .field("schedule", &self.schedule["type"])
            .field("project_id", &self.project_id)
            .finish_non_exhaustive()
    }
}
impl TaskUpsert {
    pub fn payload(&self) -> Result<Value> {
        if self.command_id.is_none() && self.id.is_none() {
            return Err(ids::invalid("commandId"));
        }
        let selection = object("modelSelection", &self.model_selection)?;
        if !selection["model"].is_string() || !(selection["instanceId"].is_string() || selection["provider"].is_string()) {
            return Err(ids::invalid("modelSelection"));
        }
        let mut v = json!({
            "title":ids::text("title", &self.title, TITLE_MAX)?,
            "prompt":ids::text("prompt", &self.prompt, PROMPT_MAX)?,
            "enabled":self.enabled,
            "schedule":schedule(&self.schedule)?,
            "projectId":ids::id("projectId", &self.project_id)?,
            "workspaceStrategy":self.workspace_strategy.to_json()?,
            "modelSelection":selection,
            "runtimeMode":one_of("runtimeMode", &self.runtime_mode, models::RUNTIME_MODES)?,
            "interactionMode":one_of("interactionMode", &self.interaction_mode, INTERACTION_MODES)?,
        });
        put(&mut v, "commandId", self.command_id.as_deref().map(|c| ids::id("commandId", c)).transpose()?);
        put(&mut v, "id", self.id.as_deref().map(|i| ids::id("id", i)).transpose()?);
        put(&mut v, "requireExisting", self.require_existing);
        if let Some(thread) = &self.thread_id {
            v["threadId"] = json!(thread.as_deref().map(|t| ids::id("threadId", t)).transpose()?);
        }
        put(&mut v, "createdBy", self.created_by.as_deref().map(|c| one_of("createdBy", c, ACTORS)).transpose()?);
        put(&mut v, "creationSource", self.creation_source.as_deref().map(|c| one_of("creationSource", c, CREATION_SOURCES)).transpose()?);
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let task = Self {
            command_id: r.opt_str("commandId")?,
            id: r.opt_str("id")?,
            require_existing: r.opt_bool("requireExisting")?,
            title: r.req_str("title")?,
            prompt: r.req_str("prompt")?,
            enabled: r.req_bool("enabled")?,
            schedule: r.req_object("schedule")?,
            project_id: r.req_str("projectId")?,
            thread_id: r.nullable_str("threadId")?,
            workspace_strategy: WorkspaceStrategy::from_json(&r.req_value("workspaceStrategy")?)?,
            model_selection: r.req_value("modelSelection")?,
            runtime_mode: r.req_str("runtimeMode")?,
            interaction_mode: r.req_str("interactionMode")?,
            created_by: r.opt_str("createdBy")?,
            creation_source: r.opt_str("creationSource")?,
        };
        r.done()?;
        Ok(task)
    }
}
/// `ScheduledTaskUpsertSchedule`.
fn schedule(v: &Value) -> Result<Value> {
    let invalid = || ids::invalid("schedule");
    let mut r = Reader::of("schedule", v)?;
    let out = match r.req_str("type").map_err(|_| invalid())?.as_str() {
        "interval" => {
            let every = r.req_u64("everyMs").ok().filter(|ms| *ms >= MIN_INTERVAL_MS).ok_or_else(invalid)?;
            json!({"type":"interval","everyMs":every})
        }
        "fixed_time" => {
            let time = r.req_str("timeOfDay").map_err(|_| invalid())?;
            let time = time.trim();
            let (h, m) = time.split_once(':').ok_or_else(invalid)?;
            let ok = (1..=2).contains(&h.len())
                && m.len() == 2
                && h.parse::<u8>().is_ok_and(|h| h < 24)
                && m.parse::<u8>().is_ok_and(|m| m < 60)
                && h.bytes().chain(m.bytes()).all(|c| c.is_ascii_digit());
            if !ok {
                return Err(invalid());
            }
            let mut out = json!({"type":"fixed_time","timeOfDay":time});
            if let Some(days) = r.opt_array("weekdays").map_err(|_| invalid())? {
                let days = days.iter().map(|d| d.as_u64().filter(|d| *d <= 6).ok_or_else(invalid)).collect::<Result<Vec<_>>>()?;
                out["weekdays"] = json!(days);
            }
            out
        }
        "webhook" => {
            let mut out = json!({"type":"webhook"});
            if let Some(signature) = r.nullable_value("signature") {
                out["signature"] = if signature.is_null() { Value::Null } else { webhook_signature(&signature)? };
            }
            if let Some(age) = r.nullable_value("maxDeliveryAgeMinutes") {
                out["maxDeliveryAgeMinutes"] = match age.as_u64() {
                    _ if age.is_null() => Value::Null,
                    Some(m) => json!(int("schedule", m, 1, 1440)?),
                    None => return Err(invalid()),
                };
            }
            if let Some(dedupe) = r.opt_value("dedupe") {
                let mut d = Reader::of("schedule", &dedupe)?;
                let mut o = json!({});
                for k in ["replayKey", "coalesceKey"] {
                    if let Some(t) = d.opt_str(k).map_err(|_| invalid())? {
                        if !(t.contains("{{") && t.contains("}}")) || t.chars().count() > 1024 {
                            return Err(invalid());
                        }
                        o[k] = json!(t);
                    }
                }
                put(&mut o, "coalesceWindowSeconds", d.opt_u64("coalesceWindowSeconds").map_err(|_| invalid())?.map(|s| int("schedule", s, 10, 3600)).transpose()?);
                d.done().map_err(|_| invalid())?;
                out["dedupe"] = o;
            }
            if let Some(filters) = r.opt_array("filters").map_err(|_| invalid())? {
                if filters.len() > MAX_WEBHOOK_FILTERS {
                    return Err(invalid());
                }
                out["filters"] = json!(filters.iter().map(webhook_filter).collect::<Result<Vec<_>>>()?);
            }
            out
        }
        _ => return Err(invalid()),
    };
    r.done().map_err(|_| invalid())?;
    Ok(out)
}
fn webhook_signature(v: &Value) -> Result<Value> {
    let invalid = || ids::invalid("schedule");
    let mut r = Reader::of("schedule", v)?;
    let header = r.req_str("header").map_err(|_| invalid())?;
    let encoding = r.req_str("encoding").map_err(|_| invalid())?;
    let prefix = r.raw("prefix").and_then(Value::as_str).ok_or_else(invalid)?.to_string();
    let mut out = json!({"header":ids::text("schedule", &header, 256)?,"encoding":one_of("schedule", &encoding, &["hex", "base64"])?,"prefix":prefix});
    for k in ["secret", "secretRef"] {
        if let Some(s) = r.opt_str(k).map_err(|_| invalid())? {
            out[k] = json!(ids::text("schedule", &s, 4096)?);
        }
    }
    r.done().map_err(|_| invalid())?;
    Ok(out)
}
fn webhook_filter(v: &Value) -> Result<Value> {
    let invalid = || ids::invalid("schedule");
    let mut r = Reader::of("schedule", v)?;
    let path = r.req_str("path").map_err(|_| invalid())?;
    let path = path.trim();
    let root = path.split('.').next().unwrap_or("");
    if !matches!(root, "body" | "headers" | "query") {
        return Err(invalid());
    }
    let op = r.req_str("op").map_err(|_| invalid())?;
    let mut out = json!({"path":path,"op":op});
    match op.as_str() {
        "equals" | "notEquals" | "startsWith" => out["value"] = json!(r.raw("value").and_then(Value::as_str).ok_or_else(invalid)?),
        "in" => {
            let values = r.raw("value").and_then(Value::as_array).filter(|v| !v.is_empty()).ok_or_else(invalid)?;
            let ok = values.iter().all(|v| v.as_str().is_some_and(|s| !s.is_empty() && !s.contains(',') && s.trim() == s));
            if !ok {
                return Err(invalid());
            }
            out["value"] = json!(values);
        }
        "exists" => {}
        _ => return Err(invalid()),
    }
    r.done().map_err(|_| invalid())?;
    Ok(out)
}
/// `scheduledTasks.upsert`: `{task}`. Resend the same payload after `rpc_outcome_unknown`.
pub fn upsert(conn: &Connection, task: &TaskUpsert) -> Result<Value> {
    send(conn, &UPSERT, task.payload()?)
}
fn id_payload(id: &str) -> Result<Value> {
    Ok(json!({"id":ids::id("id", id)?}))
}
/// `scheduledTasks.setEnabled`: `{task}`.
pub fn set_enabled(conn: &Connection, id: &str, enabled: bool) -> Result<Value> {
    send(conn, &SET_ENABLED, set_enabled_payload(id, enabled)?)
}
fn set_enabled_payload(id: &str, enabled: bool) -> Result<Value> {
    let mut v = id_payload(id)?;
    v["enabled"] = json!(enabled);
    Ok(v)
}
/// `scheduledTasks.delete`: `{id}`.
pub fn delete(conn: &Connection, id: &str) -> Result<Value> {
    send(conn, &DELETE, id_payload(id)?)
}
/// `scheduledTasks.runNow`: `{task}`. Starts a real thread: never repeat blindly;
/// reconcile with `list`.
pub fn run_now(conn: &Connection, id: &str) -> Result<Value> {
    send(conn, &RUN_NOW, id_payload(id)?)
}
