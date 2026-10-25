//! `terminal.*` unary RPCs: open, write and resize a thread's terminal.
//! `terminal.attach` is a stream. `crate::terminal` keeps the host helpers
//! (`context`, `attach`, `Output`) and builds `session`/`write`/`resize` with
//! the validators here, under its older error codes.
use super::reader::{int, put, Reader};
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Map, Value};

/// T3 `terminal.ts` bounds.
const TERMINAL_ID_MAX: usize = 128;
pub const COLS_MAX: u64 = 1000;
pub const ROWS_MAX: u64 = 500;
const DATA_MAX: usize = 65_536;
const ENV_MAX: usize = 128;
const ENV_KEY_MAX: usize = 128;
const ENV_VALUE_MAX: usize = 8192;

pub const OPEN: Function = api_fn! {
    name: "terminal.open", module: "terminal", tag: Some("terminal.open"),
    kind: IdempotentMutation, scope: Fixed("terminal:operate"),
    args: [
        (threadId, Id, required), (terminalId, Token, required), (cwd, Path, required), (worktreePath, Path, optional),
        (cols, Integer, optional), (rows, Integer, optional), (env, Object, optional), (providerInstanceId, Slug, optional),
    ],
    payload: |a| TerminalOpen::from_json(a.all())?.payload(),
};
pub const WRITE: Function = api_fn! {
    name: "terminal.write", module: "terminal", tag: Some("terminal.write"),
    kind: UnrepeatableMutation, scope: Fixed("terminal:operate"),
    args: [(threadId, Id, required), (terminalId, Token, required), (data, Text, required)],
    payload: |a| write_payload(a.req("threadId")?, a.req("terminalId")?, a.req("data")?),
};
pub const RESIZE: Function = api_fn! {
    name: "terminal.resize", module: "terminal", tag: Some("terminal.resize"),
    kind: IdempotentMutation, scope: Fixed("terminal:operate"),
    args: [(threadId, Id, required), (terminalId, Token, required), (cols, Integer, required), (rows, Integer, required)],
    payload: |a| resize_payload(a.req("threadId")?, a.req("terminalId")?, a.req_u64("cols")?, a.req_u64("rows")?),
};
pub const FUNCTIONS: &[Function] = &[OPEN, WRITE, RESIZE];

pub(crate) fn session_payload(thread_id: &str, terminal_id: &str) -> Result<Value> {
    Ok(json!({"threadId":ids::id("threadId", thread_id)?,"terminalId":ids::token("terminalId", terminal_id, TERMINAL_ID_MAX)?}))
}
pub(crate) fn size(cols: u64, rows: u64) -> Result<(u64, u64)> {
    Ok((int("cols", cols, 1, COLS_MAX)?, int("rows", rows, 1, ROWS_MAX)?))
}

/// `TerminalOpenInput`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerminalOpen {
    pub thread_id: String,
    /// Chosen by the client; T3's default is `term-1`.
    pub terminal_id: String,
    pub cwd: String,
    /// `Some(None)` sends `null`.
    pub worktree_path: Option<Option<String>>,
    pub cols: Option<u64>,
    pub rows: Option<u64>,
    /// Up to 128 `NAME=value` pairs (`^[A-Za-z_][A-Za-z0-9_]*$`, values ≤ 8192).
    pub env: Option<Map<String, Value>>,
    pub provider_instance_id: Option<String>,
}
impl TerminalOpen {
    pub fn payload(&self) -> Result<Value> {
        let mut v = session_payload(&self.thread_id, &self.terminal_id)?;
        v["cwd"] = json!(ids::path("cwd", &self.cwd)?.trim());
        if let Some(path) = &self.worktree_path {
            v["worktreePath"] = json!(path.as_deref().map(|p| ids::path("worktreePath", p).map(str::trim)).transpose()?);
        }
        put(&mut v, "cols", self.cols.map(|c| int("cols", c, 1, COLS_MAX)).transpose()?);
        put(&mut v, "rows", self.rows.map(|r| int("rows", r, 1, ROWS_MAX)).transpose()?);
        if let Some(env) = &self.env {
            let key_ok = |k: &str| {
                let b = k.as_bytes();
                !b.is_empty() && b.len() <= ENV_KEY_MAX && (b[0].is_ascii_alphabetic() || b[0] == b'_') && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
            };
            let ok = env.len() <= ENV_MAX && env.iter().all(|(k, v)| key_ok(k) && v.as_str().is_some_and(|s| s.chars().count() <= ENV_VALUE_MAX));
            if !ok {
                return Err(ids::invalid("env"));
            }
            v["env"] = json!(env);
        }
        put(&mut v, "providerInstanceId", self.provider_instance_id.as_deref().map(|p| ids::slug("providerInstanceId", p)).transpose()?);
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let open = Self {
            thread_id: r.req_str("threadId")?,
            terminal_id: r.req_str("terminalId")?,
            cwd: r.req_str("cwd")?,
            worktree_path: r.nullable_str("worktreePath")?,
            cols: r.opt_u64("cols")?,
            rows: r.opt_u64("rows")?,
            env: r.opt_object("env")?.and_then(|e| e.as_object().cloned()),
            provider_instance_id: r.opt_str("providerInstanceId")?,
        };
        r.done()?;
        Ok(open)
    }
}
/// `terminal.open`: `TerminalSessionSnapshot`. Opening a running terminal returns it as is.
pub fn open(conn: &Connection, open: &TerminalOpen) -> Result<Value> {
    send(conn, &OPEN, open.payload()?)
}
/// `terminal.write`: `null`. `data` is 1..=65 536 bytes, sent untrimmed. A repeat types it twice.
pub fn write(conn: &Connection, thread_id: &str, terminal_id: &str, data: &str) -> Result<Value> {
    send(conn, &WRITE, write_payload(thread_id, terminal_id, data)?)
}
pub(crate) fn write_payload(thread_id: &str, terminal_id: &str, data: &str) -> Result<Value> {
    if data.is_empty() || data.len() > DATA_MAX {
        return Err(ids::invalid("data"));
    }
    let mut v = session_payload(thread_id, terminal_id)?;
    v["data"] = json!(data);
    Ok(v)
}
/// `terminal.resize`: `null`. `cols` 1..=1000, `rows` 1..=500.
pub fn resize(conn: &Connection, thread_id: &str, terminal_id: &str, cols: u64, rows: u64) -> Result<Value> {
    send(conn, &RESIZE, resize_payload(thread_id, terminal_id, cols, rows)?)
}
pub(crate) fn resize_payload(thread_id: &str, terminal_id: &str, cols: u64, rows: u64) -> Result<Value> {
    let (cols, rows) = size(cols, rows)?;
    let mut v = session_payload(thread_id, terminal_id)?;
    v["cols"] = json!(cols);
    v["rows"] = json!(rows);
    Ok(v)
}
