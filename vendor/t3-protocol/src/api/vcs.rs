//! `vcs.*` and `git.preparePullRequestThread`: repository status, refs, pull and
//! init. Every write needs `source-control:write`. `git.runStackedAction` is a
//! stream (`api::streams`).
use super::reader::{int, one_of, put};
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

/// Bound for refs; ot3 uses the same for branch names.
const REF_MAX: usize = 1024;
/// T3 `GIT_LIST_BRANCHES_MAX_LIMIT`.
const LIST_REFS_LIMIT_MAX: u64 = 200;
pub const REF_KINDS: &[&str] = &["all", "local", "remote"];
/// T3 `VcsDriverKind`.
pub const DRIVER_KINDS: &[&str] = &["git", "jj", "unknown"];
pub const PREPARE_MODES: &[&str] = &["local", "worktree"];

pub const REFRESH_STATUS: Function = api_fn! {
    name: "vcs.refreshStatus", module: "vcs", tag: Some("vcs.refreshStatus"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(cwd, Path, required)],
    payload: |a| cwd_payload(a.req("cwd")?),
};
pub const LIST_REFS: Function = api_fn! {
    name: "vcs.listRefs", module: "vcs", tag: Some("vcs.listRefs"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [
        (cwd, Path, required), (query, Text, optional), (cursor, Integer, optional),
        (includeMatchingRemoteRefs, Bool, optional), (refKind, Enum(REF_KINDS), optional),
        (refresh, Bool, optional), (limit, Integer, optional),
    ],
    payload: |a| {
        let query = RefQuery {
            query: a.str("query")?.map(Into::into),
            cursor: a.u64("cursor")?,
            include_matching_remote_refs: a.bool("includeMatchingRemoteRefs")?,
            ref_kind: a.str("refKind")?.map(Into::into),
            refresh: a.bool("refresh")?,
            limit: a.u64("limit")?,
        };
        list_refs_payload(a.req("cwd")?, &query)
    },
};
pub const CREATE_REF: Function = api_fn! {
    name: "vcs.createRef", module: "vcs", tag: Some("vcs.createRef"),
    kind: UnrepeatableMutation, scope: Fixed("source-control:write"),
    args: [(cwd, Path, required), (refName, Text, required), (switchRef, Bool, optional)],
    payload: |a| create_ref_payload(a.req("cwd")?, a.req("refName")?, a.bool("switchRef")?),
};
pub const SWITCH_REF: Function = api_fn! {
    name: "vcs.switchRef", module: "vcs", tag: Some("vcs.switchRef"),
    kind: IdempotentMutation, scope: Fixed("source-control:write"),
    args: [(cwd, Path, required), (refName, Text, required)],
    payload: |a| switch_ref_payload(a.req("cwd")?, a.req("refName")?),
};
pub const PULL: Function = api_fn! {
    name: "vcs.pull", module: "vcs", tag: Some("vcs.pull"),
    kind: IdempotentMutation, scope: Fixed("source-control:write"),
    args: [(cwd, Path, required)],
    payload: |a| cwd_payload(a.req("cwd")?),
};
pub const INIT: Function = api_fn! {
    name: "vcs.init", module: "vcs", tag: Some("vcs.init"),
    kind: IdempotentMutation, scope: Fixed("source-control:write"),
    args: [(cwd, Path, required), (kind, Enum(DRIVER_KINDS), optional)],
    payload: |a| init_payload(a.req("cwd")?, a.str("kind")?),
};
/// With `mode: "worktree"` and a `threadId`, T3 also requires `orchestration:operate`.
pub const PREPARE_PULL_REQUEST_THREAD: Function = api_fn! {
    name: "git.preparePullRequestThread", module: "vcs", tag: Some("git.preparePullRequestThread"),
    kind: UnrepeatableMutation, scope: Dynamic(&["source-control:write", "orchestration:operate"]),
    args: [(cwd, Path, required), (reference, Text, required), (mode, Enum(PREPARE_MODES), required), (threadId, Id, optional)],
    payload: |a| prepare_pull_request_thread_payload(a.req("cwd")?, a.req("reference")?, a.req("mode")?, a.str("threadId")?),
};
pub const FUNCTIONS: &[Function] = &[REFRESH_STATUS, LIST_REFS, CREATE_REF, SWITCH_REF, PULL, INIT, PREPARE_PULL_REQUEST_THREAD];

fn cwd_payload(cwd: &str) -> Result<Value> {
    Ok(json!({"cwd":ids::path("cwd", cwd)?}))
}
/// `vcs.refreshStatus`: `VcsStatus` of the repository at `cwd` on the server's host.
pub fn refresh_status(conn: &Connection, cwd: &str) -> Result<Value> {
    send(conn, &REFRESH_STATUS, cwd_payload(cwd)?)
}

/// `VcsListRefsInput` filters; all optional.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefQuery {
    pub query: Option<String>,
    pub cursor: Option<u64>,
    pub include_matching_remote_refs: Option<bool>,
    /// `all`, `local` or `remote`.
    pub ref_kind: Option<String>,
    pub refresh: Option<bool>,
    /// 1..=200.
    pub limit: Option<u64>,
}
/// `vcs.listRefs`: `{refs[], isRepo, hasPrimaryRemote, nextCursor, totalCount, isAtRepositoryRoot?}`.
/// `branch_picker::dedupe` post-processes `refs`.
pub fn list_refs(conn: &Connection, cwd: &str, query: &RefQuery) -> Result<Value> {
    send(conn, &LIST_REFS, list_refs_payload(cwd, query)?)
}
fn list_refs_payload(cwd: &str, q: &RefQuery) -> Result<Value> {
    let mut v = cwd_payload(cwd)?;
    put(&mut v, "query", q.query.as_deref().map(|s| ids::text("query", s, 256)).transpose()?);
    put(&mut v, "cursor", q.cursor);
    put(&mut v, "includeMatchingRemoteRefs", q.include_matching_remote_refs);
    put(&mut v, "refKind", q.ref_kind.as_deref().map(|k| one_of("refKind", k, REF_KINDS)).transpose()?);
    put(&mut v, "refresh", q.refresh);
    put(&mut v, "limit", q.limit.map(|l| int("limit", l, 1, LIST_REFS_LIMIT_MAX)).transpose()?);
    Ok(v)
}
/// `vcs.createRef`: `{refName}`. Fails when the ref exists, so after
/// `rpc_outcome_unknown` read `list_refs` before trying again.
pub fn create_ref(conn: &Connection, cwd: &str, ref_name: &str, switch_ref: Option<bool>) -> Result<Value> {
    send(conn, &CREATE_REF, create_ref_payload(cwd, ref_name, switch_ref)?)
}
fn create_ref_payload(cwd: &str, ref_name: &str, switch_ref: Option<bool>) -> Result<Value> {
    let mut v = json!({"cwd":ids::path("cwd", cwd)?,"refName":ids::text("refName", ref_name, REF_MAX)?});
    put(&mut v, "switchRef", switch_ref);
    Ok(v)
}
/// `vcs.switchRef`: `{refName}` (`null` when detached).
pub fn switch_ref(conn: &Connection, cwd: &str, ref_name: &str) -> Result<Value> {
    send(conn, &SWITCH_REF, switch_ref_payload(cwd, ref_name)?)
}
fn switch_ref_payload(cwd: &str, ref_name: &str) -> Result<Value> {
    Ok(json!({"cwd":ids::path("cwd", cwd)?,"refName":ids::text("refName", ref_name, REF_MAX)?}))
}
/// `vcs.pull`: `{status: "pulled"|"skipped_up_to_date", refName, upstreamRef}`.
pub fn pull(conn: &Connection, cwd: &str) -> Result<Value> {
    send(conn, &PULL, cwd_payload(cwd)?)
}
/// `vcs.init`: `null`. `kind` is `git`, `jj` or `unknown` (the server's default).
pub fn init(conn: &Connection, cwd: &str, kind: Option<&str>) -> Result<Value> {
    send(conn, &INIT, init_payload(cwd, kind)?)
}
fn init_payload(cwd: &str, kind: Option<&str>) -> Result<Value> {
    let mut v = cwd_payload(cwd)?;
    put(&mut v, "kind", kind.map(|k| one_of("kind", k, DRIVER_KINDS)).transpose()?);
    Ok(v)
}
/// `git.preparePullRequestThread`: `{pullRequest, branch, worktreePath, isOnPullRequestHead}`.
/// `reference` is a PR number or URL; `mode` is `local` or `worktree`. May create
/// a worktree: reconcile with `list_refs` before repeating.
pub fn prepare_pull_request_thread(conn: &Connection, cwd: &str, reference: &str, mode: &str, thread_id: Option<&str>) -> Result<Value> {
    send(conn, &PREPARE_PULL_REQUEST_THREAD, prepare_pull_request_thread_payload(cwd, reference, mode, thread_id)?)
}
fn prepare_pull_request_thread_payload(cwd: &str, reference: &str, mode: &str, thread_id: Option<&str>) -> Result<Value> {
    let mut v = json!({"cwd":ids::path("cwd", cwd)?,"reference":ids::text("reference", reference, 2048)?,"mode":one_of("mode", mode, PREPARE_MODES)?});
    put(&mut v, "threadId", thread_id.map(|t| ids::id("threadId", t)).transpose()?);
    Ok(v)
}
/// The scopes T3 requires for a `git.preparePullRequestThread` call.
pub fn prepare_pull_request_thread_scopes(mode: &str, thread_id: Option<&str>) -> &'static [&'static str] {
    if mode == "worktree" && thread_id.is_some() {
        &["source-control:write", "orchestration:operate"]
    } else {
        &["source-control:write"]
    }
}
