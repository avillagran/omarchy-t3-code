//! Subscription streams: the 13 stream tags ot3 uses. Each has a `*_spec` that
//! validates its payload without any network, a registry row (`stream: true`)
//! that `subscribe` with `fn` reads, and a `subscribe_*` that runs the existing
//! `Connection::subscribe_probed` loop. Reconnection, backoff and keeping the
//! cursor stay with the host; `stream::cursor_of` says where to resume.
use super::{api_fn, ids, Args, Function};
use crate::{Connection, Result};
use serde_json::{json, Map, Value};
use std::time::Duration;

/// Largest `afterSequence` T3 decodes (`NonNegativeInt` is a safe JS integer).
const SEQUENCE_MAX: u64 = (1 << 53) - 1;
/// T3 `CONFIGURED_LOCAL_SERVER_URLS_MAX_ITEMS` and `PREVIEW_URL_MAX_LENGTH`.
const CONFIGURED_URLS_MAX: usize = 32;
const URL_MAX: usize = 2048;
/// `GitRunStackedActionInput.commitMessage`.
const COMMIT_MESSAGE_MAX: usize = 10_000;
const ACTION_ID_MAX: usize = 256;
pub const STACKED_ACTIONS: &[&str] = &["commit", "push", "create_pr", "commit_push", "commit_push_pr"];

pub const SUBSCRIBE_SHELL: Function = Function {
    stream: true,
    ..api_fn! {
        name: "orchestration.subscribeShell", module: "orchestration", tag: Some("orchestration.subscribeShell"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [(afterSequence, Integer, optional), (requestCompletionMarker, Bool, optional)],
        payload: |a| ShellSub::from_args(a)?.payload(),
    }
};
pub const SUBSCRIBE_THREAD: Function = Function {
    stream: true,
    ..api_fn! {
        name: "orchestration.subscribeThread", module: "orchestration", tag: Some("orchestration.subscribeThread"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [
            (threadId, Id, required), (afterSequence, Integer, optional), (requestCompletionMarker, Bool, optional),
            (acceptBoundedSnapshot, Bool, optional), (acceptCompactTurnItems, Bool, optional),
        ],
        payload: |a| ThreadSub::from_args(a)?.payload(),
    }
};
pub const SUBSCRIBE_COMMAND_OUTPUT: Function = Function {
    stream: true,
    ..api_fn! {
        name: "orchestration.subscribeCommandOutput", module: "orchestration", tag: Some("orchestration.subscribeCommandOutput"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [(threadId, Id, required), (itemId, Id, required)],
        payload: |a| command_output_payload(a.req("threadId")?, a.req("itemId")?),
    }
};
/// Attaching may start (or, with `restartIfNotRunning`, restart) the terminal;
/// attaching again to the same id is what a reconnecting host does.
pub const TERMINAL_ATTACH: Function = Function {
    stream: true,
    ..api_fn! {
        name: "terminal.attach", module: "terminal", tag: Some("terminal.attach"),
        kind: IdempotentMutation, scope: Fixed("terminal:operate"),
        args: [
            (threadId, Id, required), (terminalId, Token, required), (cwd, Path, optional),
            (worktreePath, Path, optional), (cols, Integer, optional), (rows, Integer, optional),
            (env, Object, optional), (providerInstanceId, Slug, optional),
            (restartIfNotRunning, Bool, optional), (resume, Bool, optional),
        ],
        payload: |a| TerminalAttach::from_args(a)?.payload(),
    }
};
pub const SUBSCRIBE_DEVICE_STATE: Function = Function {
    stream: true,
    ..api_fn! {
        name: "device.subscribeDeviceState", module: "device", tag: Some("subscribeDeviceState"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [], payload: |_| Ok(json!({})),
    }
};
pub const SUBSCRIBE_DISCOVERED_LOCAL_SERVERS: Function = Function {
    stream: true,
    ..api_fn! {
        name: "preview.subscribeDiscoveredLocalServers", module: "preview", tag: Some("subscribeDiscoveredLocalServers"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [(configuredUrls, Array, optional)],
        payload: |a| local_servers_payload(strings(a, "configuredUrls")?.as_deref()),
    }
};
pub const SUBSCRIBE_PREVIEW_EVENTS: Function = Function {
    stream: true,
    ..api_fn! {
        name: "preview.subscribePreviewEvents", module: "preview", tag: Some("subscribePreviewEvents"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [], payload: |_| Ok(json!({})),
    }
};
pub const SUBSCRIBE_SERVER_CONFIG: Function = Function {
    stream: true,
    ..api_fn! {
        name: "server.subscribeServerConfig", module: "server", tag: Some("subscribeServerConfig"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [(environmentThemes, Bool, optional), (usageLimitSources, Bool, optional), (usageLimitsCommand, Bool, optional)],
        payload: |a| ConfigSub::from_args(a)?.payload(),
    }
};
pub const SUBSCRIBE_WORKTREE_SETUP: Function = Function {
    stream: true,
    ..api_fn! {
        name: "worktreeSetup.subscribeWorktreeSetup", module: "worktreeSetup", tag: Some("subscribeWorktreeSetup"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [(threadId, Id, required)],
        payload: |a| thread_payload(a.req("threadId")?),
    }
};
pub const STORAGE_CLEANUP_REPORT: Function = Function {
    stream: true,
    ..api_fn! {
        name: "server.getStorageCleanupReport", module: "diagnostics", tag: Some("server.getStorageCleanupReport"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [], payload: |_| Ok(json!({})),
    }
};
pub const PROVIDER_INSTALL: Function = Function {
    stream: true,
    ..api_fn! {
        name: "provider.install.subscribe", module: "provider", tag: Some("provider.install.subscribe"),
        kind: Read, scope: Fixed("orchestration:read"),
        args: [(instanceId, Slug, required)],
        payload: |a| instance_payload(a.req("instanceId")?),
    }
};
pub const PROVIDER_AUTH: Function = Function {
    stream: true,
    ..api_fn! {
        name: "provider.auth.subscribe", module: "provider", tag: Some("provider.auth.subscribe"),
        kind: Read, scope: Fixed("providers:manage"),
        args: [(instanceId, Slug, required)],
        payload: |a| instance_payload(a.req("instanceId")?),
    }
};
/// Commits, pushes and opens PRs: never repeat it after a cut. Reconcile with
/// `vcs.refreshStatus` and `vcs.listRefs`. With `threadId` and a PR action T3
/// also requires `orchestration:read`.
pub const RUN_STACKED_ACTION: Function = Function {
    stream: true,
    ..api_fn! {
        name: "git.runStackedAction", module: "vcs", tag: Some("git.runStackedAction"),
        kind: UnrepeatableMutation, scope: Dynamic(&["source-control:write", "orchestration:read"]),
        args: [
            (actionId, Token, required), (cwd, Path, required), (action, Enum(STACKED_ACTIONS), required),
            (commitMessage, Text, optional), (featureBranch, Bool, optional), (filePaths, Array, optional),
            (threadId, Id, optional), (projectId, Id, optional),
        ],
        payload: |a| StackedAction::from_args(a)?.payload(),
    }
};

/// Reads of a stream's first item with a deadline (`stream::first_item`), for
/// hosts that only want the current state: no queue and no `poll`.
pub const INSTALL_STATE: Function = api_fn! {
    name: "provider.installState", module: "provider", tag: None,
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(instanceId, Slug, required)],
    payload: |a| instance_payload(a.req("instanceId")?),
    timeout_ms: 10_000,
};
pub const AUTH_STATE: Function = api_fn! {
    name: "provider.authState", module: "provider", tag: None,
    kind: Read, scope: Fixed("providers:manage"),
    args: [(instanceId, Slug, required)],
    payload: |a| instance_payload(a.req("instanceId")?),
    timeout_ms: 10_000,
};
pub const LAST_STORAGE_CLEANUP_REPORT: Function = api_fn! {
    name: "diagnostics.storageCleanupReport", module: "diagnostics", tag: None,
    kind: Read, scope: Fixed("orchestration:read"),
    args: [], payload: |_| Ok(json!({})),
    timeout_ms: 10_000,
};
/// Waits up to its deadline for a list with servers: the first may precede the scan.
pub const DISCOVERED_SERVERS: Function = api_fn! {
    name: "preview.discoveredServers", module: "preview", tag: None,
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(configuredUrls, Array, optional)],
    payload: |a| local_servers_payload(strings(a, "configuredUrls")?.as_deref()),
    timeout_ms: 4_000,
};

pub const FUNCTIONS: &[Function] = &[
    SUBSCRIBE_SHELL,
    SUBSCRIBE_THREAD,
    SUBSCRIBE_COMMAND_OUTPUT,
    TERMINAL_ATTACH,
    SUBSCRIBE_DEVICE_STATE,
    SUBSCRIBE_DISCOVERED_LOCAL_SERVERS,
    SUBSCRIBE_PREVIEW_EVENTS,
    SUBSCRIBE_SERVER_CONFIG,
    SUBSCRIBE_WORKTREE_SETUP,
    STORAGE_CLEANUP_REPORT,
    PROVIDER_INSTALL,
    PROVIDER_AUTH,
    RUN_STACKED_ACTION,
    INSTALL_STATE,
    AUTH_STATE,
    LAST_STORAGE_CLEANUP_REPORT,
    DISCOVERED_SERVERS,
];

/// How a host reads a stream's items, as `describe` lists them under `streams`.
pub struct Shape {
    pub function: &'static Function,
    /// Member that tells items apart, or `None` when every item is the whole state.
    pub item_key: Option<&'static str>,
    /// Values of `item_key` (`[]` with no key). Newer servers may add more.
    pub items: &'static [&'static str],
    /// What the first item can mean (`stream::Start`).
    pub starts: &'static [&'static str],
    /// `afterSequence`: resubscribe with the poll's `cursor`. `sequence`: items
    /// carry one only to drop replays. `None`: no cursor.
    pub cursor: Option<&'static str>,
    /// Items after which the server ends the stream.
    pub ends: &'static [&'static str],
    /// The composed read of its current state, if there is one.
    pub first_item: Option<&'static str>,
}
const STATE: &[&str] = &["snapshot"];
pub const SHAPES: &[Shape] = &[
    Shape {
        function: &SUBSCRIBE_SHELL,
        item_key: Some("kind"),
        items: &["synchronized", "snapshot", "project.updated", "project.removed", "thread.updated", "thread.removed"],
        starts: &["snapshot", "resume"],
        cursor: Some("afterSequence"),
        ends: &[],
        first_item: None,
    },
    Shape {
        function: &SUBSCRIBE_THREAD,
        item_key: Some("kind"),
        items: &["synchronized", "snapshot", "event"],
        starts: &["snapshot", "resume"],
        cursor: Some("afterSequence"),
        ends: &[],
        first_item: None,
    },
    Shape {
        function: &SUBSCRIBE_COMMAND_OUTPUT,
        item_key: Some("kind"),
        items: &["replace", "append"],
        starts: STATE,
        cursor: None,
        ends: &[],
        first_item: None,
    },
    Shape {
        function: &TERMINAL_ATTACH,
        item_key: Some("type"),
        items: &["snapshot", "output", "exited", "closed", "error", "cleared", "restarted", "activity"],
        starts: STATE,
        cursor: Some("sequence"),
        ends: &[],
        first_item: None,
    },
    Shape { function: &SUBSCRIBE_DEVICE_STATE, item_key: None, items: &[], starts: STATE, cursor: None, ends: &[], first_item: None },
    Shape {
        function: &SUBSCRIBE_DISCOVERED_LOCAL_SERVERS,
        item_key: None,
        items: &[],
        starts: STATE,
        cursor: None,
        ends: &[],
        first_item: Some("preview.discoveredServers"),
    },
    Shape {
        function: &SUBSCRIBE_PREVIEW_EVENTS,
        item_key: Some("type"),
        items: &["opened", "navigated", "resized", "failed", "closed"],
        starts: &["events"],
        cursor: None,
        ends: &[],
        first_item: None,
    },
    Shape {
        function: &SUBSCRIBE_SERVER_CONFIG,
        item_key: Some("type"),
        items: &["snapshot", "keybindingsUpdated", "providerStatuses", "settingsUpdated", "environmentThemesUpdated", "usageLimitSourcesUpdated"],
        starts: STATE,
        cursor: None,
        ends: &[],
        first_item: None,
    },
    Shape { function: &SUBSCRIBE_WORKTREE_SETUP, item_key: None, items: &[], starts: STATE, cursor: Some("sequence"), ends: &[], first_item: None },
    Shape {
        function: &STORAGE_CLEANUP_REPORT,
        item_key: None,
        items: &[],
        starts: STATE,
        cursor: None,
        ends: &[],
        first_item: Some("diagnostics.storageCleanupReport"),
    },
    Shape { function: &PROVIDER_INSTALL, item_key: None, items: &[], starts: STATE, cursor: None, ends: &[], first_item: Some("provider.installState") },
    Shape { function: &PROVIDER_AUTH, item_key: None, items: &[], starts: STATE, cursor: None, ends: &[], first_item: Some("provider.authState") },
    Shape {
        function: &RUN_STACKED_ACTION,
        item_key: Some("kind"),
        items: &["action_started", "phase_started", "hook_started", "hook_output", "hook_finished", "action_finished", "action_failed"],
        starts: &["events"],
        cursor: None,
        ends: &["action_finished", "action_failed"],
        first_item: None,
    },
];
/// `describe`'s `streams`: one entry per stream function, in registry order.
pub fn describe() -> Value {
    json!(SHAPES
        .iter()
        .map(|s| json!({"fn":s.function.name,"tag":s.function.tag,"itemKey":s.item_key,"items":s.items,
            "starts":s.starts,"cursor":s.cursor,"ends":s.ends,"firstItem":s.first_item}))
        .collect::<Vec<_>>())
}

/// A validated subscription: its registry row and the exact wire payload.
#[derive(Clone)]
pub struct StreamSpec {
    pub function: &'static Function,
    pub payload: Value,
}
impl std::fmt::Debug for StreamSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamSpec").field("tag", &self.tag()).field("payload", &self.payload).finish()
    }
}
impl StreamSpec {
    fn new(function: &'static Function, payload: Value) -> Self {
        Self { function, payload }
    }
    pub fn tag(&self) -> &'static str {
        self.function.tag.unwrap_or_default()
    }
    /// Largest single item: 64 MiB only for a thread stream with `acceptBoundedSnapshot`.
    pub fn item_limit(&self) -> u64 {
        crate::stream::item_limit(self.tag(), &self.payload)
    }
    /// Runs the existing subscription loop (`Connection::subscribe_probed`) until
    /// `cancel`, an error or the server's end. Never reconnects.
    pub fn subscribe(
        &self,
        conn: &Connection,
        cancel: impl Fn() -> bool,
        probe: impl FnMut() -> bool,
        receive: impl FnMut(Option<Value>) -> Result<()>,
    ) -> Result<()> {
        conn.subscribe_probed(self.tag(), self.payload.clone(), cancel, probe, receive)
    }
}
/// The spec of a stream function by name and JSON args (what `subscribe` with `fn` does).
/// `unknown_function` for an unregistered name, `not_a_stream` for a unary one.
pub fn spec(name: &str, args: &Value) -> Result<StreamSpec> {
    let function = super::function(name).ok_or("unknown_function")?;
    if !function.stream {
        return Err("not_a_stream".into());
    }
    Ok(StreamSpec::new(function, function.payload(args)?))
}

/// `orchestration.subscribeShell` arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellSub {
    /// The host's cursor: the largest `sequence` (or `snapshot.snapshotSequence`) applied.
    pub after_sequence: Option<u64>,
    /// Ask for a `{"kind":"synchronized"}` item once the replay is complete.
    pub request_completion_marker: Option<bool>,
}
impl ShellSub {
    pub fn payload(&self) -> Result<Value> {
        let mut p = json!({});
        put_sequence(&mut p, self.after_sequence)?;
        put(&mut p, "requestCompletionMarker", self.request_completion_marker);
        Ok(p)
    }
    fn from_args(a: &Args) -> Result<Self> {
        Ok(Self { after_sequence: uint(a, "afterSequence")?, request_completion_marker: a.bool("requestCompletionMarker")? })
    }
}
/// `orchestration.subscribeThread` arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThreadSub {
    pub thread_id: String,
    /// The host's cursor: `max(snapshotSequence, sequence of every event)` applied.
    pub after_sequence: Option<u64>,
    pub request_completion_marker: Option<bool>,
    /// Lets the first `snapshot` be a bounded window of up to 64 MiB.
    pub accept_bounded_snapshot: Option<bool>,
    pub accept_compact_turn_items: Option<bool>,
}
impl ThreadSub {
    pub fn new(thread_id: &str) -> Self {
        Self { thread_id: thread_id.into(), ..Self::default() }
    }
    pub fn payload(&self) -> Result<Value> {
        let mut p = thread_payload(&self.thread_id)?;
        put_sequence(&mut p, self.after_sequence)?;
        put(&mut p, "requestCompletionMarker", self.request_completion_marker);
        put(&mut p, "acceptBoundedSnapshot", self.accept_bounded_snapshot);
        put(&mut p, "acceptCompactTurnItems", self.accept_compact_turn_items);
        Ok(p)
    }
    fn from_args(a: &Args) -> Result<Self> {
        Ok(Self {
            thread_id: a.req("threadId")?.into(),
            after_sequence: uint(a, "afterSequence")?,
            request_completion_marker: a.bool("requestCompletionMarker")?,
            accept_bounded_snapshot: a.bool("acceptBoundedSnapshot")?,
            accept_compact_turn_items: a.bool("acceptCompactTurnItems")?,
        })
    }
}
/// `terminal.attach` arguments (`TerminalAttachInput`). `terminal::attach` builds the
/// usual desktop payload from a shell context; this is the same input, typed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerminalAttach {
    pub thread_id: String,
    pub terminal_id: String,
    pub cwd: Option<String>,
    pub worktree_path: Option<String>,
    pub cols: Option<u64>,
    pub rows: Option<u64>,
    /// Variable names `^[A-Za-z_][A-Za-z0-9_]*$` (≤ 128), string values (≤ 8192), ≤ 128 entries.
    pub env: Option<Map<String, Value>>,
    pub provider_instance_id: Option<String>,
    pub restart_if_not_running: Option<bool>,
    /// Keep a stopped terminal stopped when resubscribing; unknown ids recover after a server restart.
    pub resume: Option<bool>,
}
impl TerminalAttach {
    pub fn new(thread_id: &str, terminal_id: &str) -> Self {
        Self { thread_id: thread_id.into(), terminal_id: terminal_id.into(), ..Self::default() }
    }
    pub fn payload(&self) -> Result<Value> {
        let mut p = thread_payload(&self.thread_id)?;
        p["terminalId"] = json!(ids::token("terminalId", &self.terminal_id, 128)?);
        if let Some(cwd) = &self.cwd {
            p["cwd"] = json!(ids::path("cwd", cwd)?);
        }
        if let Some(path) = &self.worktree_path {
            p["worktreePath"] = json!(ids::path("worktreePath", path)?);
        }
        for (name, value, max) in [("cols", self.cols, 1000), ("rows", self.rows, 500)] {
            if let Some(n) = value {
                if !(1..=max).contains(&n) {
                    return Err(ids::invalid(name));
                }
                p[name] = json!(n);
            }
        }
        if let Some(env) = &self.env {
            let name_ok = |k: &str| {
                let b = k.as_bytes();
                !b.is_empty() && b.len() <= 128 && !b[0].is_ascii_digit()
                    && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
            };
            let ok = env.len() <= 128
                && env.iter().all(|(k, v)| name_ok(k) && v.as_str().is_some_and(|s| s.chars().count() <= 8192));
            if !ok {
                return Err(ids::invalid("env"));
            }
            p["env"] = json!(env);
        }
        if let Some(id) = &self.provider_instance_id {
            p["providerInstanceId"] = json!(ids::slug("providerInstanceId", id)?);
        }
        put(&mut p, "restartIfNotRunning", self.restart_if_not_running);
        put(&mut p, "resume", self.resume);
        Ok(p)
    }
    fn from_args(a: &Args) -> Result<Self> {
        Ok(Self {
            thread_id: a.req("threadId")?.into(),
            terminal_id: a.req("terminalId")?.into(),
            cwd: a.str("cwd")?.map(Into::into),
            worktree_path: a.str("worktreePath")?.map(Into::into),
            cols: uint(a, "cols")?,
            rows: uint(a, "rows")?,
            env: a.object("env")?.and_then(Value::as_object).cloned(),
            provider_instance_id: a.str("providerInstanceId")?.map(Into::into),
            restart_if_not_running: a.bool("restartIfNotRunning")?,
            resume: a.bool("resume")?,
        })
    }
}
/// `subscribeServerConfig` flags. Each asks for an event type an older client
/// would not decode; leave them off unless the host handles it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigSub {
    pub environment_themes: Option<bool>,
    pub usage_limit_sources: Option<bool>,
    /// The host answers `/usage-limits` itself.
    pub usage_limits_command: Option<bool>,
}
impl ConfigSub {
    pub fn payload(&self) -> Result<Value> {
        let mut p = json!({});
        put(&mut p, "environmentThemes", self.environment_themes);
        put(&mut p, "usageLimitSources", self.usage_limit_sources);
        put(&mut p, "usageLimitsCommand", self.usage_limits_command);
        Ok(p)
    }
    fn from_args(a: &Args) -> Result<Self> {
        Ok(Self {
            environment_themes: a.bool("environmentThemes")?,
            usage_limit_sources: a.bool("usageLimitSources")?,
            usage_limits_command: a.bool("usageLimitsCommand")?,
        })
    }
}
/// `git.runStackedAction` arguments. The host mints `action_id` and keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StackedAction {
    pub action_id: String,
    pub cwd: String,
    /// One of `STACKED_ACTIONS`.
    pub action: String,
    pub commit_message: Option<String>,
    pub feature_branch: Option<bool>,
    /// At least one path when present.
    pub file_paths: Option<Vec<String>>,
    pub thread_id: Option<String>,
    pub project_id: Option<String>,
}
impl StackedAction {
    pub fn payload(&self) -> Result<Value> {
        if !STACKED_ACTIONS.contains(&self.action.as_str()) {
            return Err(ids::invalid("action"));
        }
        let mut p = json!({
            "actionId": ids::token("actionId", &self.action_id, ACTION_ID_MAX)?,
            "cwd": ids::path("cwd", &self.cwd)?,
            "action": self.action,
        });
        if let Some(message) = &self.commit_message {
            p["commitMessage"] = json!(ids::text("commitMessage", message, COMMIT_MESSAGE_MAX)?);
        }
        put(&mut p, "featureBranch", self.feature_branch);
        if let Some(paths) = &self.file_paths {
            let ok = !paths.is_empty() && paths.iter().all(|f| !f.is_empty() && f.len() <= 4096 && !f.contains('\0'));
            if !ok {
                return Err(ids::invalid("filePaths"));
            }
            p["filePaths"] = json!(paths);
        }
        for (name, id) in [("threadId", &self.thread_id), ("projectId", &self.project_id)] {
            if let Some(id) = id {
                p[name] = json!(ids::id(name, id)?);
            }
        }
        Ok(p)
    }
    fn from_args(a: &Args) -> Result<Self> {
        Ok(Self {
            action_id: a.req("actionId")?.into(),
            cwd: a.req("cwd")?.into(),
            action: a.req("action")?.into(),
            commit_message: a.str("commitMessage")?.map(Into::into),
            feature_branch: a.bool("featureBranch")?,
            file_paths: strings(a, "filePaths")?,
            thread_id: a.str("threadId")?.map(Into::into),
            project_id: a.str("projectId")?.map(Into::into),
        })
    }
}

pub fn shell_spec(sub: &ShellSub) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&SUBSCRIBE_SHELL, sub.payload()?))
}
pub fn thread_spec(sub: &ThreadSub) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&SUBSCRIBE_THREAD, sub.payload()?))
}
pub fn command_output_spec(thread_id: &str, item_id: &str) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&SUBSCRIBE_COMMAND_OUTPUT, command_output_payload(thread_id, item_id)?))
}
pub fn terminal_attach_spec(attach: &TerminalAttach) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&TERMINAL_ATTACH, attach.payload()?))
}
pub fn device_state_spec() -> Result<StreamSpec> {
    Ok(StreamSpec::new(&SUBSCRIBE_DEVICE_STATE, json!({})))
}
pub fn local_servers_spec(configured_urls: Option<&[String]>) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&SUBSCRIBE_DISCOVERED_LOCAL_SERVERS, local_servers_payload(configured_urls)?))
}
pub fn preview_events_spec() -> Result<StreamSpec> {
    Ok(StreamSpec::new(&SUBSCRIBE_PREVIEW_EVENTS, json!({})))
}
pub fn server_config_spec(sub: &ConfigSub) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&SUBSCRIBE_SERVER_CONFIG, sub.payload()?))
}
pub fn worktree_setup_spec(thread_id: &str) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&SUBSCRIBE_WORKTREE_SETUP, thread_payload(thread_id)?))
}
pub fn storage_cleanup_report_spec() -> Result<StreamSpec> {
    Ok(StreamSpec::new(&STORAGE_CLEANUP_REPORT, json!({})))
}
pub fn provider_install_spec(instance_id: &str) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&PROVIDER_INSTALL, instance_payload(instance_id)?))
}
pub fn provider_auth_spec(instance_id: &str) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&PROVIDER_AUTH, instance_payload(instance_id)?))
}
pub fn stacked_action_spec(action: &StackedAction) -> Result<StreamSpec> {
    Ok(StreamSpec::new(&RUN_STACKED_ACTION, action.payload()?))
}

/// Defines `subscribe_*`: validate (no socket on a bad argument), then run the
/// existing loop with the caller's `cancel`, `probe` and `receive`.
macro_rules! subscribe_fns {
    ($($(#[$doc:meta])* $name:ident($($arg:ident: $ty:ty),*) => $spec:ident;)*) => {$(
        $(#[$doc])*
        pub fn $name(
            conn: &Connection,
            $($arg: $ty,)*
            cancel: impl Fn() -> bool,
            probe: impl FnMut() -> bool,
            receive: impl FnMut(Option<Value>) -> Result<()>,
        ) -> Result<()> {
            $spec($($arg),*)?.subscribe(conn, cancel, probe, receive)
        }
    )*};
}
subscribe_fns! {
    /// Items by `kind`: `snapshot`, `synchronized`, `project.updated|removed`, `thread.updated|removed`.
    subscribe_shell(sub: &ShellSub) => shell_spec;
    /// Items by `kind`: `snapshot`, `synchronized`, `event` (also events this build cannot decode).
    subscribe_thread(sub: &ThreadSub) => thread_spec;
    /// `{kind: replace|append, text, truncated, running}`; ends after `running: false`.
    subscribe_command_output(thread_id: &str, item_id: &str) => command_output_spec;
    /// Items by `type`, starting with `snapshot`. `terminal::Output::apply` drops replays.
    subscribe_terminal_attach(attach: &TerminalAttach) => terminal_attach_spec;
    /// Every item is the whole `DeviceServiceState`.
    subscribe_device_state() => device_state_spec;
    /// Every item is the whole `{servers, scannedAt}` list; the first may precede the scan.
    subscribe_local_servers(configured_urls: Option<&[String]>) => local_servers_spec;
    /// `PreviewEvent` by `type`. No cursor: reread `preview.list` after a gap.
    subscribe_preview_events() => preview_events_spec;
    /// `{version: 1, type}` items, starting with `snapshot`.
    subscribe_server_config(sub: &ConfigSub) => server_config_spec;
    /// Every item is the whole `WorktreeSetupSnapshot` (or `null`), `sequence` growing.
    subscribe_worktree_setup(thread_id: &str) => worktree_setup_spec;
    /// The last `StorageCleanupReport` or `null`.
    subscribe_storage_cleanup_report() => storage_cleanup_report_spec;
    /// Every item is the whole `ProviderInstallState`.
    subscribe_provider_install(instance_id: &str) => provider_install_spec;
    /// Every item is the whole `ProviderAuthState`.
    subscribe_provider_auth(instance_id: &str) => provider_auth_spec;
    /// `GitActionProgressEvent` by `kind`, ending with `action_finished` or `action_failed`.
    subscribe_stacked_action(action: &StackedAction) => stacked_action_spec;
}

/// `provider.installState`: the current `ProviderInstallState` (10 s unless `timeout`).
pub fn install_state(conn: &Connection, instance_id: &str, timeout: Option<Duration>) -> Result<Value> {
    first_state(conn, &INSTALL_STATE, instance_payload(instance_id)?, timeout)
}
/// `provider.authState`: the current `ProviderAuthState` (10 s unless `timeout`).
pub fn auth_state(conn: &Connection, instance_id: &str, timeout: Option<Duration>) -> Result<Value> {
    first_state(conn, &AUTH_STATE, instance_payload(instance_id)?, timeout)
}
/// `diagnostics.storageCleanupReport`: the last `StorageCleanupReport`, or `null` before the first run.
pub fn storage_cleanup_report(conn: &Connection, timeout: Option<Duration>) -> Result<Value> {
    first_state(conn, &LAST_STORAGE_CLEANUP_REPORT, json!({}), timeout)
}
/// `preview.discoveredServers`: `{servers, scannedAt, …}`, the first list with servers
/// or the last one seen within 4 s (unless `timeout`).
pub fn discovered_servers(conn: &Connection, configured_urls: Option<&[String]>, timeout: Option<Duration>) -> Result<Value> {
    first_state(conn, &DISCOVERED_SERVERS, local_servers_payload(configured_urls)?, timeout)
}
fn first_state(conn: &Connection, f: &Function, payload: Value, timeout: Option<Duration>) -> Result<Value> {
    read_first(conn, f, payload, timeout.unwrap_or(f.timeout()))
}
/// A composed first-item read (`tag: None`): the stream it reads and when an item is enough.
/// Nothing within the deadline is `stream_timeout`.
pub(crate) fn read_first(conn: &Connection, f: &Function, payload: Value, timeout: Duration) -> Result<Value> {
    let (stream, accept): (&'static Function, fn(&Value) -> bool) = match f.name {
        "provider.installState" => (&PROVIDER_INSTALL, |_| true),
        "provider.authState" => (&PROVIDER_AUTH, |_| true),
        "diagnostics.storageCleanupReport" => (&STORAGE_CLEANUP_REPORT, |_| true),
        "preview.discoveredServers" => {
            (&SUBSCRIBE_DISCOVERED_LOCAL_SERVERS, |v| v["servers"].as_array().is_some_and(|s| !s.is_empty()))
        }
        _ => return Err("unknown_function".into()),
    };
    crate::stream::first_item(conn, &StreamSpec::new(stream, payload), timeout, accept)?
        .ok_or_else(|| "stream_timeout".into())
}

fn thread_payload(thread_id: &str) -> Result<Value> {
    Ok(json!({"threadId":ids::id("threadId", thread_id)?}))
}
fn command_output_payload(thread_id: &str, item_id: &str) -> Result<Value> {
    let mut p = thread_payload(thread_id)?;
    p["itemId"] = json!(ids::id("itemId", item_id)?);
    Ok(p)
}
fn instance_payload(instance_id: &str) -> Result<Value> {
    Ok(json!({"instanceId":ids::slug("instanceId", instance_id)?}))
}
fn local_servers_payload(configured_urls: Option<&[String]>) -> Result<Value> {
    let mut p = json!({});
    if let Some(urls) = configured_urls {
        if urls.len() > CONFIGURED_URLS_MAX {
            return Err(ids::invalid("configuredUrls"));
        }
        let urls = urls
            .iter()
            .map(|u| ids::text("configuredUrls", u, URL_MAX))
            .collect::<Result<Vec<_>>>()?;
        p["configuredUrls"] = json!(urls);
    }
    Ok(p)
}
fn put(p: &mut Value, key: &str, value: Option<bool>) {
    if let Some(v) = value {
        p[key] = json!(v);
    }
}
fn put_sequence(p: &mut Value, after: Option<u64>) -> Result<()> {
    if let Some(n) = after {
        if n > SEQUENCE_MAX {
            return Err(ids::invalid("afterSequence"));
        }
        p["afterSequence"] = json!(n);
    }
    Ok(())
}
/// An optional non-negative integer; another JSON type is `invalid_<name>`.
fn uint(a: &Args, name: &str) -> Result<Option<u64>> {
    a.get(name).map(|v| v.as_u64().ok_or_else(|| ids::invalid(name))).transpose()
}
/// An optional array of strings.
fn strings(a: &Args, name: &str) -> Result<Option<Vec<String>>> {
    a.get(name)
        .map(|v| {
            v.as_array()
                .ok_or_else(|| ids::invalid(name))?
                .iter()
                .map(|s| s.as_str().map(str::to_string).ok_or_else(|| ids::invalid(name)))
                .collect()
        })
        .transpose()
}
