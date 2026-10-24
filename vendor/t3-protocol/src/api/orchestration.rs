//! `orchestration.*`: commands and thread launch. `Command` types the
//! `dispatchCommand` variants ot3 emits; the payload builders that already
//! live in `queue`, `questions`, `approvals`, `models` and `sidebar` stay the
//! source of truth for theirs, and `Command` calls them.
use super::{api_fn, ids, send, ArgType, Function};
use crate::{approvals, models, questions, queue, sidebar, Connection, Result};
use serde_json::{json, Map, Value};

pub const INTERACTION_MODES: &[&str] = &["default", "plan"];
pub const CREATION_SOURCES: &[&str] = &["web", "mobile", "mcp", "provider", "server"];
pub const ACTORS: &[&str] = &["user", "agent", "system"];
pub const PULL_REQUEST_LINK_SOURCES: &[&str] = &["manual", "created", "agent", "stack", "stack-dismissed"];
pub const DELIVERY_INTENTS: &[&str] = &["auto", "steer", "restart"];
/// Messages and queued edits: ot3's bounds.
const TEXT_MAX: usize = 120_000;
const ATTACHMENTS_MAX: usize = 100;
const TITLE_MAX: usize = 255;
const BRANCH_MAX: usize = 1024;

/// Command types `Command` types. Every other contract type travels as `Command::Raw`.
pub const COMMAND_TYPES: &[&str] = &[
    "thread.create", "message.dispatch", "run.interrupt", "subagent.stop", "thread.metadata.update",
    "thread.runtime-mode.set", "thread.interaction-mode.set", "thread.fork", "thread.merge_back",
    "thread.archive", "thread.unarchive", "thread.delete", "thread.settle", "thread.unsettle",
    "thread.snooze", "thread.unsnooze", "thread.auto-settle", "thread.auto-settle.set", "thread.pin",
    "thread.unpin", "thread.pin.reorder", "thread.mark-unread", "thread.visit", "queued-run.cancel",
    "queued-run.reorder", "queued-run.edit", "queued-message.promote-to-steer", "queue.resume",
    "runtime-request.respond", "thread.user-input.dismiss", "checkpoint.rollback",
    "thread.pull-request.link", "thread.pull-request.unlink", "thread.pull-request.watch",
];
/// Contract command types ot3 does not emit (server, agents, other clients).
/// They are accepted only as `Command::Raw`, with just `type` and `commandId` checked.
pub const RAW_COMMAND_TYPES: &[&str] = &[
    "thread.stop", "thread.active.reorder", "thread.pull-request-link.sync", "thread.pull-request.sync",
    "thread.title.regeneration.complete", "thread.model-selection.set", "provider-session.detach",
    "prepared-run.release", "notification.delivery.accept", "prepared-run.progress", "prepared-run.fail",
    "prepared-run.retry", "queued-run.start", "delegated_task.request", "delegated_task.wake-policy",
    "delegated_task.completion-delivery.acknowledge", "delegated_task.completion-delivery.dispose",
    "thread.created.record", "provider.switch",
];

pub const DISPATCH_COMMAND: Function = api_fn! {
    name: "orchestration.dispatchCommand", module: "orchestration", tag: Some("orchestration.dispatchCommand"),
    kind: MutationWithCommandId, scope: Fixed("orchestration:operate"),
    args: [(type, ArgType::Command, required), (commandId, Id, required)],
    payload: |a| Command::from_json(a.all())?.payload(),
    open_args: true, variants: COMMAND_TYPES,
};
pub const LAUNCH_THREAD: Function = api_fn! {
    name: "orchestration.launchThread", module: "orchestration", tag: Some("orchestration.launchThread"),
    kind: MutationWithCommandId, scope: Fixed("orchestration:operate"),
    args: [
        (commandId, Id, required), (projectId, Id, required), (title, Text, required),
        (modelSelection, Object, required), (runtimeMode, Enum(models::RUNTIME_MODES), required),
        (interactionMode, Enum(INTERACTION_MODES), required), (workspaceStrategy, Object, required),
        (creationSource, Enum(CREATION_SOURCES), optional), (threadId, Id, optional),
        (reuseExistingThread, Bool, optional), (generateTitle, Bool, optional), (initialMessage, Object, optional),
    ],
    payload: |a| ThreadLaunch::from_json(a.all())?.payload(),
};
pub const FUNCTIONS: &[Function] = &[DISPATCH_COMMAND, LAUNCH_THREAD];

/// `orchestration.dispatchCommand`: `{sequence}`. The server deduplicates by
/// `commandId`; after `rpc_outcome_unknown`, resend the same command.
pub fn dispatch(conn: &Connection, command: &Command) -> Result<Value> {
    send(conn, &DISPATCH_COMMAND, command.payload()?)
}
/// `orchestration.launchThread`: `{threadId, projection, resumed}`. Deduplicated by `commandId`.
pub fn launch_thread(conn: &Connection, launch: &ThreadLaunch) -> Result<Value> {
    send(conn, &LAUNCH_THREAD, launch.payload()?)
}

/// Where a launched or deferred thread runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceStrategy {
    Root { branch: Option<String> },
    ExistingWorktree { worktree_path: String, branch: Option<String> },
    Worktree { base_ref: String, branch: Option<String>, start_from_origin: Option<bool> },
}
impl WorkspaceStrategy {
    pub fn to_json(&self) -> Result<Value> {
        let (mut v, branch) = match self {
            Self::Root { branch } => (json!({"type":"root"}), branch),
            Self::ExistingWorktree { worktree_path, branch } => {
                (json!({"type":"existing_worktree","worktreePath":ids::path("worktreePath", worktree_path)?}), branch)
            }
            Self::Worktree { base_ref, branch, start_from_origin } => {
                let mut v = json!({"type":"worktree","baseRef":ids::text("baseRef", base_ref, BRANCH_MAX)?});
                if let Some(origin) = start_from_origin {
                    v["startFromOrigin"] = json!(origin);
                }
                (v, branch)
            }
        };
        if let Some(b) = branch {
            v["branch"] = json!(ids::text("branch", b, BRANCH_MAX)?);
        }
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let invalid = || ids::invalid("workspaceStrategy");
        let mut r = Reader::new(v).map_err(|_| invalid())?;
        let strategy = match r.req_str("type").map_err(|_| invalid())?.as_str() {
            "root" => Self::Root { branch: r.opt_str("branch")? },
            "existing_worktree" => Self::ExistingWorktree { worktree_path: r.req_str("worktreePath")?, branch: r.opt_str("branch")? },
            "worktree" => Self::Worktree {
                base_ref: r.req_str("baseRef")?,
                branch: r.opt_str("branch")?,
                start_from_origin: r.opt_bool("startFromOrigin")?,
            },
            _ => return Err(invalid()),
        };
        r.done()?;
        Ok(strategy)
    }
}

/// The message a launch can deliver atomically with the thread.
#[derive(Clone, Debug, PartialEq)]
pub struct InitialMessage {
    pub message_id: Option<String>,
    pub text: String,
    pub context: Option<Value>,
    pub attachments: Vec<Value>,
}
/// `OrchestrationV2ThreadLaunchInput`.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadLaunch {
    pub command_id: String,
    pub project_id: String,
    pub title: String,
    /// `ModelSelection` object, passed as is.
    pub model_selection: Value,
    pub runtime_mode: String,
    pub interaction_mode: String,
    pub workspace_strategy: WorkspaceStrategy,
    pub creation_source: Option<String>,
    pub thread_id: Option<String>,
    pub reuse_existing_thread: Option<bool>,
    pub generate_title: Option<bool>,
    pub initial_message: Option<InitialMessage>,
}
impl ThreadLaunch {
    pub fn payload(&self) -> Result<Value> {
        let mut v = json!({
            "commandId":ids::id("commandId", &self.command_id)?,
            "projectId":ids::id("projectId", &self.project_id)?,
            "title":ids::text("title", &self.title, TITLE_MAX)?,
            "modelSelection":object("modelSelection", &self.model_selection)?,
            "runtimeMode":runtime_mode(&self.runtime_mode)?,
            "interactionMode":one_of("interactionMode", &self.interaction_mode, INTERACTION_MODES)?,
            "workspaceStrategy":self.workspace_strategy.to_json()?,
        });
        if let Some(s) = &self.creation_source {
            v["creationSource"] = json!(one_of("creationSource", s, CREATION_SOURCES)?);
        }
        if let Some(t) = &self.thread_id {
            v["threadId"] = json!(ids::id("threadId", t)?);
        }
        put(&mut v, "reuseExistingThread", self.reuse_existing_thread);
        put(&mut v, "generateTitle", self.generate_title);
        if let Some(m) = &self.initial_message {
            let mut message = json!({"text":message_text("initialMessage", &m.text, &m.attachments)?,"attachments":attachments("initialMessage", &m.attachments)?});
            if let Some(id) = &m.message_id {
                message["messageId"] = json!(ids::id("messageId", id)?);
            }
            if let Some(c) = &m.context {
                message["context"] = object("context", c)?;
            }
            v["initialMessage"] = message;
        }
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let launch = Self {
            command_id: r.req_str("commandId")?,
            project_id: r.req_str("projectId")?,
            title: r.req_str("title")?,
            model_selection: r.req_value("modelSelection")?,
            runtime_mode: r.req_str("runtimeMode")?,
            interaction_mode: r.req_str("interactionMode")?,
            workspace_strategy: WorkspaceStrategy::from_json(&r.req_value("workspaceStrategy")?)?,
            creation_source: r.opt_str("creationSource")?,
            thread_id: r.opt_str("threadId")?,
            reuse_existing_thread: r.opt_bool("reuseExistingThread")?,
            generate_title: r.opt_bool("generateTitle")?,
            initial_message: r
                .opt_value("initialMessage")
                .map(|m| -> Result<InitialMessage> {
                    let mut m = Reader::new(&m).map_err(|_| ids::invalid("initialMessage"))?;
                    let message = InitialMessage {
                        message_id: m.opt_str("messageId")?,
                        text: m.req_str("text")?,
                        context: m.opt_value("context"),
                        attachments: m.req_array("attachments")?,
                    };
                    m.done()?;
                    Ok(message)
                })
                .transpose()?,
        };
        r.done()?;
        Ok(launch)
    }
}

/// `message.dispatch` dispatch mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchMode {
    DeferStart { workspace_strategy: Option<WorkspaceStrategy> },
    SteerActive { target_run_id: String },
    RestartActive { target_run_id: String },
    QueueAfterActive,
    StartImmediately,
}
/// `message.dispatch`. `new` fills the user-sent defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageDispatch {
    pub command_id: String,
    pub thread_id: String,
    pub message_id: String,
    pub text: String,
    pub attachments: Vec<Value>,
    pub dispatch_mode: DispatchMode,
    pub created_by: String,
    pub creation_source: String,
    pub context: Option<Value>,
    pub title_seed: Option<String>,
    pub model_selection: Option<Value>,
    /// `{threadId, planId}`.
    pub source_plan_ref: Option<Value>,
    pub restart_continuation_of_run_id: Option<String>,
    pub usage_limit_continuation_of_run_id: Option<String>,
    pub manual_continuation_of_run_id: Option<String>,
    pub usage_limit_recovery_request_id: Option<String>,
    pub delivery_intent: Option<String>,
}
impl MessageDispatch {
    /// A user message that starts immediately, with no attachments.
    pub fn new(command_id: &str, thread_id: &str, message_id: &str, text: &str, creation_source: &str) -> Self {
        Self {
            command_id: command_id.into(),
            thread_id: thread_id.into(),
            message_id: message_id.into(),
            text: text.into(),
            attachments: vec![],
            dispatch_mode: DispatchMode::StartImmediately,
            created_by: "user".into(),
            creation_source: creation_source.into(),
            context: None,
            title_seed: None,
            model_selection: None,
            source_plan_ref: None,
            restart_continuation_of_run_id: None,
            usage_limit_continuation_of_run_id: None,
            manual_continuation_of_run_id: None,
            usage_limit_recovery_request_id: None,
            delivery_intent: None,
        }
    }
}
/// `thread.create` (metadata only; `launch_thread` can also deliver a message).
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadCreate {
    pub command_id: String,
    pub thread_id: String,
    pub project_id: String,
    pub title: String,
    pub model_selection: Value,
    pub runtime_mode: String,
    pub interaction_mode: String,
    pub branch: Option<String>,
    pub worktree_path: Option<String>,
    pub created_by: String,
    pub creation_source: String,
    pub imported_native_thread: Option<Value>,
}
/// `thread.metadata.update` fields. `Some(None)` clears a nullable field.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetadataUpdate {
    pub title: Option<String>,
    pub regenerate_title: Option<bool>,
    pub branch: Option<Option<String>>,
    pub worktree_path: Option<Option<String>>,
    pub expected_worktree_path: Option<Option<String>>,
    pub expected_branch: Option<Option<String>>,
    pub expected_empty: Option<bool>,
    /// `Value::Null` clears it.
    pub limit_recovery: Option<Value>,
    /// `Value::Null` unlinks.
    pub linked_pull_request: Option<Value>,
}
/// Where a fork or merge-back starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourcePoint {
    LatestStable,
    Run { run_id: String },
    Checkpoint { checkpoint_id: String },
}
/// `ThreadPullRequestKey`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PullRequestKey {
    pub host: String,
    pub repository: String,
    pub number: u64,
}
/// The answer of a `runtime-request.respond`.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeResponse {
    /// An approval decision (`accept`, `acceptForSession`, `acceptAlways`, `decline`, `cancel`).
    Decision(String),
    /// User-input answers by question id.
    Answers(Map<String, Value>),
}

/// The `orchestration.dispatchCommand` variants ot3 emits. Every variant
/// carries the host's `commandId`; the library validates it and never mints one.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    ThreadCreate(ThreadCreate),
    MessageDispatch(MessageDispatch),
    RunInterrupt { command_id: String, thread_id: String, run_id: String, reason: Option<String>, hold_queue: Option<bool> },
    SubagentStop { command_id: String, thread_id: String, run_id: String, subagent_id: String },
    ThreadMetadataUpdate { command_id: String, thread_id: String, update: MetadataUpdate },
    RuntimeModeSet { command_id: String, thread_id: String, runtime_mode: String },
    InteractionModeSet { command_id: String, thread_id: String, interaction_mode: String },
    ThreadFork {
        command_id: String,
        source_thread_id: String,
        target_thread_id: String,
        source_point: SourcePoint,
        title: Option<String>,
        created_at: Option<String>,
        created_by: String,
        creation_source: String,
    },
    ThreadMergeBack {
        command_id: String,
        source_thread_id: String,
        target_thread_id: String,
        source_point: SourcePoint,
        created_at: Option<String>,
        created_by: String,
        creation_source: String,
    },
    ThreadArchive { command_id: String, thread_id: String },
    ThreadUnarchive { command_id: String, thread_id: String },
    ThreadDelete { command_id: String, thread_id: String },
    ThreadSettle { command_id: String, thread_id: String, settled_at: Option<String> },
    ThreadUnsettle { command_id: String, thread_id: String },
    ThreadSnooze { command_id: String, thread_id: String, snoozed_until: String },
    ThreadUnsnooze { command_id: String, thread_id: String },
    ThreadAutoSettle { command_id: String, thread_id: String, snapshot_at: String, settled_at: Option<String> },
    ThreadAutoSettleSet { command_id: String, thread_id: String, enabled: bool },
    ThreadPin { command_id: String, thread_id: String, order_key: Option<String> },
    ThreadUnpin { command_id: String, thread_id: String },
    ThreadPinReorder { command_id: String, thread_id: String, order_key: String },
    ThreadMarkUnread { command_id: String, thread_id: String },
    ThreadVisit { command_id: String, thread_id: String, visited_at: String },
    QueuedRunCancel { command_id: String, thread_id: String, run_id: String },
    QueuedRunReorder { command_id: String, thread_id: String, run_id: String, before_run_id: Option<String> },
    QueuedRunEdit { command_id: String, thread_id: String, run_id: String, text: String, attachments: Vec<Value>, context: Option<Value> },
    QueuedMessagePromoteToSteer { command_id: String, thread_id: String, queued_run_id: String, target_run_id: String },
    QueueResume { command_id: String, thread_id: String },
    RuntimeRequestRespond { command_id: String, thread_id: String, request_id: String, response: RuntimeResponse },
    ThreadUserInputDismiss { command_id: String, thread_id: String, request_id: String },
    CheckpointRollback { command_id: String, thread_id: String, scope_id: String, checkpoint_id: String, restore_files: Option<bool> },
    ThreadPullRequestLink {
        command_id: String,
        thread_id: String,
        key: PullRequestKey,
        url: String,
        source: String,
        repository_web_url: Option<String>,
    },
    ThreadPullRequestUnlink { command_id: String, thread_id: String, key: PullRequestKey },
    ThreadPullRequestWatch { command_id: String, thread_id: String, key: PullRequestKey, watching: bool, link: Option<Value> },
    /// A contract command `Command` does not type. Only `type` and `commandId` are checked.
    Raw(Value),
}

impl Command {
    /// Wrap the output of an existing builder, adding the host's `commandId`.
    fn wrap(command_id: &str, mut built: Value) -> Result<Self> {
        built["commandId"] = json!(command_id);
        Self::from_json(&built)
    }
    /// `queue::command` (cancel, reorder, steer, resume, edit).
    pub fn queue(command_id: &str, input: &Value) -> Result<Self> {
        Self::wrap(command_id, queue::command(input)?)
    }
    /// `questions::command` (respond with answers, or dismiss).
    pub fn answer(command_id: &str, input: &Value) -> Result<Self> {
        Self::wrap(command_id, questions::command(input)?)
    }
    /// `approvals::command`: only a still-pending live request and an offered decision.
    pub fn approve(command_id: &str, input: &Value, projection: &Value) -> Result<Self> {
        Self::wrap(command_id, approvals::command(input, projection)?)
    }
    /// `models::runtime_mode_command`.
    pub fn runtime_mode(command_id: &str, thread_id: &str, mode: &str) -> Result<Self> {
        Self::from_json(&models::runtime_mode_command(thread_id, command_id, mode)?)
    }
    /// `sidebar::command` (settle, unsettle, snooze, unsnooze).
    pub fn sidebar(command_id: &str, action: &str, thread_id: &str, until: Option<&str>) -> Result<Self> {
        Self::wrap(command_id, sidebar::command(action, thread_id, until)?)
    }
    /// `sidebar::thread_action` (rename, regenerate title, pin, unpin, mark unread,
    /// auto-settle, archive, confirmed delete).
    pub fn thread_action(command_id: &str, input: &Value) -> Result<Self> {
        Self::wrap(command_id, sidebar::thread_action(input)?)
    }

    pub fn command_id(&self) -> Option<&str> {
        match self {
            Self::ThreadCreate(c) => Some(&c.command_id),
            Self::MessageDispatch(m) => Some(&m.command_id),
            Self::Raw(v) => v["commandId"].as_str(),
            Self::RunInterrupt { command_id, .. }
            | Self::SubagentStop { command_id, .. }
            | Self::ThreadMetadataUpdate { command_id, .. }
            | Self::RuntimeModeSet { command_id, .. }
            | Self::InteractionModeSet { command_id, .. }
            | Self::ThreadFork { command_id, .. }
            | Self::ThreadMergeBack { command_id, .. }
            | Self::ThreadArchive { command_id, .. }
            | Self::ThreadUnarchive { command_id, .. }
            | Self::ThreadDelete { command_id, .. }
            | Self::ThreadSettle { command_id, .. }
            | Self::ThreadUnsettle { command_id, .. }
            | Self::ThreadSnooze { command_id, .. }
            | Self::ThreadUnsnooze { command_id, .. }
            | Self::ThreadAutoSettle { command_id, .. }
            | Self::ThreadAutoSettleSet { command_id, .. }
            | Self::ThreadPin { command_id, .. }
            | Self::ThreadUnpin { command_id, .. }
            | Self::ThreadPinReorder { command_id, .. }
            | Self::ThreadMarkUnread { command_id, .. }
            | Self::ThreadVisit { command_id, .. }
            | Self::QueuedRunCancel { command_id, .. }
            | Self::QueuedRunReorder { command_id, .. }
            | Self::QueuedRunEdit { command_id, .. }
            | Self::QueuedMessagePromoteToSteer { command_id, .. }
            | Self::QueueResume { command_id, .. }
            | Self::RuntimeRequestRespond { command_id, .. }
            | Self::ThreadUserInputDismiss { command_id, .. }
            | Self::CheckpointRollback { command_id, .. }
            | Self::ThreadPullRequestLink { command_id, .. }
            | Self::ThreadPullRequestUnlink { command_id, .. }
            | Self::ThreadPullRequestWatch { command_id, .. } => Some(command_id),
        }
    }

    /// The validated wire payload. Validation happens here, before any socket opens.
    pub fn payload(&self) -> Result<Value> {
        let cid = |c: &str| ids::id("commandId", c).map(str::to_string);
        let tid = |t: &str| ids::id("threadId", t).map(str::to_string);
        let simple = |kind: &str, c: &str, t: &str| -> Result<Value> { Ok(json!({"type":kind,"commandId":cid(c)?,"threadId":tid(t)?})) };
        // Builders that do not take a commandId: validate ours, then add it.
        let with_id = |c: &str, built: Result<Value>| -> Result<Value> {
            let c = cid(c)?;
            let mut v = built?;
            v["commandId"] = json!(c);
            Ok(v)
        };
        Ok(match self {
            Self::ThreadCreate(c) => {
                let mut v = json!({"type":"thread.create","commandId":cid(&c.command_id)?,"threadId":tid(&c.thread_id)?,
                    "projectId":ids::id("projectId", &c.project_id)?,"title":ids::text("title", &c.title, TITLE_MAX)?,
                    "modelSelection":object("modelSelection", &c.model_selection)?,"runtimeMode":runtime_mode(&c.runtime_mode)?,
                    "interactionMode":one_of("interactionMode", &c.interaction_mode, INTERACTION_MODES)?,
                    "branch":c.branch.as_deref().map(|b| ids::text("branch", b, BRANCH_MAX)).transpose()?,
                    "worktreePath":c.worktree_path.as_deref().map(|p| ids::path("worktreePath", p)).transpose()?,
                    "createdBy":one_of("createdBy", &c.created_by, ACTORS)?,
                    "creationSource":one_of("creationSource", &c.creation_source, CREATION_SOURCES)?});
                if let Some(i) = &c.imported_native_thread {
                    v["importedNativeThread"] = object("importedNativeThread", i)?;
                }
                v
            }
            Self::MessageDispatch(m) => {
                let mode = match &m.dispatch_mode {
                    DispatchMode::DeferStart { workspace_strategy } => {
                        let mut d = json!({"type":"defer_start"});
                        if let Some(w) = workspace_strategy {
                            d["workspaceStrategy"] = w.to_json()?;
                        }
                        d
                    }
                    DispatchMode::SteerActive { target_run_id } => {
                        json!({"type":"steer_active","targetRunId":ids::id("targetRunId", target_run_id)?})
                    }
                    DispatchMode::RestartActive { target_run_id } => {
                        json!({"type":"restart_active","targetRunId":ids::id("targetRunId", target_run_id)?})
                    }
                    DispatchMode::QueueAfterActive => json!({"type":"queue_after_active"}),
                    DispatchMode::StartImmediately => json!({"type":"start_immediately"}),
                };
                let mut v = json!({"type":"message.dispatch","commandId":cid(&m.command_id)?,"threadId":tid(&m.thread_id)?,
                    "messageId":ids::id("messageId", &m.message_id)?,"text":message_text("text", &m.text, &m.attachments)?,
                    "attachments":attachments("attachments", &m.attachments)?,"createdBy":one_of("createdBy", &m.created_by, ACTORS)?,
                    "creationSource":one_of("creationSource", &m.creation_source, CREATION_SOURCES)?,"dispatchMode":mode});
                if let Some(c) = &m.context {
                    v["context"] = object("context", c)?;
                }
                if let Some(s) = &m.title_seed {
                    v["titleSeed"] = json!(ids::text("titleSeed", s, TITLE_MAX)?);
                }
                if let Some(s) = &m.model_selection {
                    v["modelSelection"] = object("modelSelection", s)?;
                }
                if let Some(p) = &m.source_plan_ref {
                    let plan = object("sourcePlanRef", p)?;
                    let ok = plan.as_object().is_some_and(|o| o.len() == 2)
                        && plan["threadId"].as_str().is_some_and(|t| ids::id("threadId", t).is_ok())
                        && plan["planId"].as_str().is_some_and(|t| ids::id("planId", t).is_ok());
                    if !ok {
                        return Err(ids::invalid("sourcePlanRef"));
                    }
                    v["sourcePlanRef"] = plan;
                }
                for (key, value) in [
                    ("restartContinuationOfRunId", &m.restart_continuation_of_run_id),
                    ("usageLimitContinuationOfRunId", &m.usage_limit_continuation_of_run_id),
                    ("manualContinuationOfRunId", &m.manual_continuation_of_run_id),
                    ("usageLimitRecoveryRequestId", &m.usage_limit_recovery_request_id),
                ] {
                    if let Some(id) = value {
                        v[key] = json!(ids::id(key, id)?);
                    }
                }
                if let Some(d) = &m.delivery_intent {
                    v["deliveryIntent"] = json!(one_of("deliveryIntent", d, DELIVERY_INTENTS)?);
                }
                v
            }
            Self::RunInterrupt { command_id, thread_id, run_id, reason, hold_queue } => {
                let mut v = simple("run.interrupt", command_id, thread_id)?;
                v["runId"] = json!(ids::id("runId", run_id)?);
                if let Some(r) = reason {
                    v["reason"] = json!(free_text("reason", r)?);
                }
                put(&mut v, "holdQueue", *hold_queue);
                v
            }
            Self::SubagentStop { command_id, thread_id, run_id, subagent_id } => {
                let mut v = simple("subagent.stop", command_id, thread_id)?;
                v["runId"] = json!(ids::id("runId", run_id)?);
                v["subagentId"] = json!(ids::id("subagentId", subagent_id)?);
                v
            }
            Self::ThreadMetadataUpdate { command_id, thread_id, update: u } => {
                let mut v = simple("thread.metadata.update", command_id, thread_id)?;
                if let Some(t) = &u.title {
                    v["title"] = json!(ids::text("title", t, TITLE_MAX)?);
                }
                put(&mut v, "regenerateTitle", u.regenerate_title);
                for (key, value, is_path) in [
                    ("branch", &u.branch, false),
                    ("worktreePath", &u.worktree_path, true),
                    ("expectedWorktreePath", &u.expected_worktree_path, true),
                    ("expectedBranch", &u.expected_branch, false),
                ] {
                    if let Some(value) = value {
                        v[key] = match value {
                            None => Value::Null,
                            Some(s) if is_path => json!(ids::path(key, s)?),
                            Some(s) => json!(ids::text(key, s, BRANCH_MAX)?),
                        };
                    }
                }
                put(&mut v, "expectedEmpty", u.expected_empty);
                for (key, value) in [("limitRecovery", &u.limit_recovery), ("linkedPullRequest", &u.linked_pull_request)] {
                    if let Some(value) = value {
                        v[key] = if value.is_null() { Value::Null } else { object(key, value)? };
                    }
                }
                v
            }
            Self::RuntimeModeSet { command_id, thread_id, runtime_mode } => {
                cid(command_id)?;
                tid(thread_id)?;
                models::runtime_mode_command(thread_id, command_id, runtime_mode)?
            }
            Self::InteractionModeSet { command_id, thread_id, interaction_mode } => {
                let mut v = simple("thread.interaction-mode.set", command_id, thread_id)?;
                v["interactionMode"] = json!(one_of("interactionMode", interaction_mode, INTERACTION_MODES)?);
                v
            }
            Self::ThreadFork { command_id, source_thread_id, target_thread_id, source_point, title, created_at, created_by, creation_source } => {
                let mut v = fork("thread.fork", command_id, source_thread_id, target_thread_id, source_point, created_at, created_by, creation_source)?;
                if let Some(t) = title {
                    v["title"] = json!(ids::text("title", t, TITLE_MAX)?);
                }
                v
            }
            Self::ThreadMergeBack { command_id, source_thread_id, target_thread_id, source_point, created_at, created_by, creation_source } => {
                fork("thread.merge_back", command_id, source_thread_id, target_thread_id, source_point, created_at, created_by, creation_source)?
            }
            Self::ThreadArchive { command_id, thread_id } => simple("thread.archive", command_id, thread_id)?,
            Self::ThreadUnarchive { command_id, thread_id } => simple("thread.unarchive", command_id, thread_id)?,
            Self::ThreadDelete { command_id, thread_id } => simple("thread.delete", command_id, thread_id)?,
            Self::ThreadSettle { command_id, thread_id, settled_at: None } => {
                tid(thread_id)?;
                with_id(command_id, sidebar::command("settle", thread_id, None))?
            }
            Self::ThreadSettle { command_id, thread_id, settled_at: Some(at) } => {
                let mut v = simple("thread.settle", command_id, thread_id)?;
                v["settledAt"] = json!(timestamp("settledAt", at)?);
                v
            }
            Self::ThreadUnsettle { command_id, thread_id } => {
                tid(thread_id)?;
                with_id(command_id, sidebar::command("unsettle", thread_id, None))?
            }
            Self::ThreadSnooze { command_id, thread_id, snoozed_until } => {
                tid(thread_id)?;
                with_id(command_id, sidebar::command("snooze", thread_id, Some(snoozed_until)))?
            }
            Self::ThreadUnsnooze { command_id, thread_id } => {
                tid(thread_id)?;
                with_id(command_id, sidebar::command("unsnooze", thread_id, None))?
            }
            Self::ThreadAutoSettle { command_id, thread_id, snapshot_at, settled_at } => {
                let mut v = simple("thread.auto-settle", command_id, thread_id)?;
                v["snapshotAt"] = json!(timestamp("snapshotAt", snapshot_at)?);
                if let Some(at) = settled_at {
                    v["settledAt"] = json!(timestamp("settledAt", at)?);
                }
                v
            }
            Self::ThreadAutoSettleSet { command_id, thread_id, enabled } => {
                let mut v = simple("thread.auto-settle.set", command_id, thread_id)?;
                v["enabled"] = json!(enabled);
                v
            }
            Self::ThreadPin { command_id, thread_id, order_key } => {
                let mut v = simple("thread.pin", command_id, thread_id)?;
                if let Some(k) = order_key {
                    v["orderKey"] = json!(ids::text("orderKey", k, 256)?);
                }
                v
            }
            Self::ThreadUnpin { command_id, thread_id } => simple("thread.unpin", command_id, thread_id)?,
            Self::ThreadPinReorder { command_id, thread_id, order_key } => {
                let mut v = simple("thread.pin.reorder", command_id, thread_id)?;
                v["orderKey"] = json!(ids::text("orderKey", order_key, 256)?);
                v
            }
            Self::ThreadMarkUnread { command_id, thread_id } => simple("thread.mark-unread", command_id, thread_id)?,
            Self::ThreadVisit { command_id, thread_id, visited_at } => {
                let mut v = simple("thread.visit", command_id, thread_id)?;
                v["visitedAt"] = json!(timestamp("visitedAt", visited_at)?);
                v
            }
            Self::QueuedRunCancel { command_id, thread_id, run_id } => {
                queued(command_id, thread_id, run_id, json!({"action":"cancel"}))?
            }
            Self::QueuedRunReorder { command_id, thread_id, run_id, before_run_id } => {
                if let Some(b) = before_run_id {
                    ids::id("beforeRunId", b)?;
                }
                queued(command_id, thread_id, run_id, json!({"action":"reorder","beforeRunId":before_run_id}))?
            }
            Self::QueuedRunEdit { command_id, thread_id, run_id, text, attachments, context } => {
                let mut input = json!({"action":"edit","text":text,"attachments":attachments});
                if let Some(c) = context {
                    input["context"] = object("context", c)?;
                }
                queued(command_id, thread_id, run_id, input)?
            }
            Self::QueuedMessagePromoteToSteer { command_id, thread_id, queued_run_id, target_run_id } => {
                ids::id("targetRunId", target_run_id)?;
                queued(command_id, thread_id, queued_run_id, json!({"action":"steer","targetRunId":target_run_id}))?
            }
            Self::QueueResume { command_id, thread_id } => {
                tid(thread_id)?;
                with_id(command_id, queue::command(&json!({"action":"resume","thread":thread_id})))?
            }
            Self::RuntimeRequestRespond { command_id, thread_id, request_id, response } => {
                tid(thread_id)?;
                ids::opaque("requestId", request_id)?;
                match response {
                    RuntimeResponse::Answers(answers) => with_id(
                        command_id,
                        questions::command(&json!({"thread":thread_id,"runtimeRequestId":request_id,"action":"respond","answers":answers})),
                    )?,
                    // `approvals::command` checks a decision against the live
                    // projection; `Command::approve` runs it. Here only the value is known.
                    RuntimeResponse::Decision(decision) => {
                        if !approvals::valid_decision(decision) {
                            return Err("invalid_approval_decision".into());
                        }
                        json!({"type":"runtime-request.respond","commandId":cid(command_id)?,"threadId":thread_id,"requestId":request_id,"decision":decision})
                    }
                }
            }
            Self::ThreadUserInputDismiss { command_id, thread_id, request_id } => {
                tid(thread_id)?;
                ids::opaque("requestId", request_id)?;
                with_id(command_id, questions::command(&json!({"thread":thread_id,"runtimeRequestId":request_id,"action":"dismiss"})))?
            }
            Self::CheckpointRollback { command_id, thread_id, scope_id, checkpoint_id, restore_files } => {
                let mut v = simple("checkpoint.rollback", command_id, thread_id)?;
                v["scopeId"] = json!(ids::id("scopeId", scope_id)?);
                v["checkpointId"] = json!(ids::id("checkpointId", checkpoint_id)?);
                put(&mut v, "restoreFiles", *restore_files);
                v
            }
            Self::ThreadPullRequestLink { command_id, thread_id, key, url, source, repository_web_url } => {
                let mut v = pull_request("thread.pull-request.link", command_id, thread_id, key)?;
                v["url"] = json!(ids::text("url", url, 2048)?);
                v["source"] = json!(one_of("source", source, PULL_REQUEST_LINK_SOURCES)?);
                if let Some(u) = repository_web_url {
                    v["repositoryWebUrl"] = json!(ids::text("repositoryWebUrl", u, 2048)?);
                }
                v
            }
            Self::ThreadPullRequestUnlink { command_id, thread_id, key } => {
                pull_request("thread.pull-request.unlink", command_id, thread_id, key)?
            }
            Self::ThreadPullRequestWatch { command_id, thread_id, key, watching, link } => {
                let mut v = pull_request("thread.pull-request.watch", command_id, thread_id, key)?;
                v["watching"] = json!(watching);
                if let Some(l) = link {
                    let l = object("link", l)?;
                    if !l["url"].is_string() || !l["source"].as_str().is_some_and(|s| PULL_REQUEST_LINK_SOURCES.contains(&s)) {
                        return Err(ids::invalid("link"));
                    }
                    v["link"] = l;
                }
                v
            }
            Self::Raw(v) => {
                let kind = v["type"].as_str().ok_or_else(|| ids::invalid("type"))?;
                if kind.is_empty() || kind.chars().any(char::is_control) {
                    return Err(ids::invalid("type"));
                }
                cid(v["commandId"].as_str().ok_or_else(|| ids::invalid("commandId"))?)?;
                v.clone()
            }
        })
    }

    /// Parse a contract-shaped command (`{"type":…,"commandId":…,…}`).
    /// Unknown members are `invalid_input`; unknown types are `invalid_type`.
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let kind = r.req_str("type")?;
        if RAW_COMMAND_TYPES.contains(&kind.as_str()) {
            let command = Self::Raw(v.clone());
            command.payload()?;
            return Ok(command);
        }
        let c = r.req_str("commandId")?;
        let command = match kind.as_str() {
            "thread.create" => Self::ThreadCreate(ThreadCreate {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                project_id: r.req_str("projectId")?,
                title: r.req_str("title")?,
                model_selection: r.req_value("modelSelection")?,
                runtime_mode: r.req_str("runtimeMode")?,
                interaction_mode: r.req_str("interactionMode")?,
                branch: r.nullable_str("branch")?.ok_or_else(|| ids::invalid("branch"))?,
                worktree_path: r.nullable_str("worktreePath")?.ok_or_else(|| ids::invalid("worktreePath"))?,
                created_by: r.req_str("createdBy")?,
                creation_source: r.req_str("creationSource")?,
                imported_native_thread: r.opt_value("importedNativeThread"),
            }),
            "message.dispatch" => {
                let mut m = MessageDispatch::new(&c, &r.req_str("threadId")?, &r.req_str("messageId")?, &r.req_str("text")?, &r.req_str("creationSource")?);
                m.attachments = r.req_array("attachments")?;
                m.created_by = r.req_str("createdBy")?;
                m.dispatch_mode = dispatch_mode(&r.req_value("dispatchMode")?)?;
                m.context = r.opt_value("context");
                m.title_seed = r.opt_str("titleSeed")?;
                m.model_selection = r.opt_value("modelSelection");
                m.source_plan_ref = r.opt_value("sourcePlanRef");
                m.restart_continuation_of_run_id = r.opt_str("restartContinuationOfRunId")?;
                m.usage_limit_continuation_of_run_id = r.opt_str("usageLimitContinuationOfRunId")?;
                m.manual_continuation_of_run_id = r.opt_str("manualContinuationOfRunId")?;
                m.usage_limit_recovery_request_id = r.opt_str("usageLimitRecoveryRequestId")?;
                m.delivery_intent = r.opt_str("deliveryIntent")?;
                Self::MessageDispatch(m)
            }
            "run.interrupt" => Self::RunInterrupt {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                run_id: r.req_str("runId")?,
                reason: r.opt_str("reason")?,
                hold_queue: r.opt_bool("holdQueue")?,
            },
            "subagent.stop" => Self::SubagentStop {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                run_id: r.req_str("runId")?,
                subagent_id: r.req_str("subagentId")?,
            },
            "thread.metadata.update" => Self::ThreadMetadataUpdate {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                update: MetadataUpdate {
                    title: r.opt_str("title")?,
                    regenerate_title: r.opt_bool("regenerateTitle")?,
                    branch: r.nullable_str("branch")?,
                    worktree_path: r.nullable_str("worktreePath")?,
                    expected_worktree_path: r.nullable_str("expectedWorktreePath")?,
                    expected_branch: r.nullable_str("expectedBranch")?,
                    expected_empty: r.opt_bool("expectedEmpty")?,
                    limit_recovery: r.nullable_value("limitRecovery"),
                    linked_pull_request: r.nullable_value("linkedPullRequest"),
                },
            },
            "thread.runtime-mode.set" => Self::RuntimeModeSet { command_id: c, thread_id: r.req_str("threadId")?, runtime_mode: r.req_str("runtimeMode")? },
            "thread.interaction-mode.set" => {
                Self::InteractionModeSet { command_id: c, thread_id: r.req_str("threadId")?, interaction_mode: r.req_str("interactionMode")? }
            }
            "thread.fork" => Self::ThreadFork {
                command_id: c,
                source_thread_id: r.req_str("sourceThreadId")?,
                target_thread_id: r.req_str("targetThreadId")?,
                source_point: source_point(&r.req_value("sourcePoint")?)?,
                title: r.opt_str("title")?,
                created_at: r.opt_str("createdAt")?,
                created_by: r.req_str("createdBy")?,
                creation_source: r.req_str("creationSource")?,
            },
            "thread.merge_back" => Self::ThreadMergeBack {
                command_id: c,
                source_thread_id: r.req_str("sourceThreadId")?,
                target_thread_id: r.req_str("targetThreadId")?,
                source_point: source_point(&r.req_value("sourcePoint")?)?,
                created_at: r.opt_str("createdAt")?,
                created_by: r.req_str("createdBy")?,
                creation_source: r.req_str("creationSource")?,
            },
            "thread.archive" => Self::ThreadArchive { command_id: c, thread_id: r.req_str("threadId")? },
            "thread.unarchive" => Self::ThreadUnarchive { command_id: c, thread_id: r.req_str("threadId")? },
            "thread.delete" => Self::ThreadDelete { command_id: c, thread_id: r.req_str("threadId")? },
            "thread.settle" => Self::ThreadSettle { command_id: c, thread_id: r.req_str("threadId")?, settled_at: r.opt_str("settledAt")? },
            "thread.unsettle" => {
                r.literal("reason", "user")?;
                Self::ThreadUnsettle { command_id: c, thread_id: r.req_str("threadId")? }
            }
            "thread.snooze" => Self::ThreadSnooze { command_id: c, thread_id: r.req_str("threadId")?, snoozed_until: r.req_str("snoozedUntil")? },
            "thread.unsnooze" => {
                r.literal("reason", "user")?;
                Self::ThreadUnsnooze { command_id: c, thread_id: r.req_str("threadId")? }
            }
            "thread.auto-settle" => Self::ThreadAutoSettle {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                snapshot_at: r.req_str("snapshotAt")?,
                settled_at: r.opt_str("settledAt")?,
            },
            "thread.auto-settle.set" => Self::ThreadAutoSettleSet { command_id: c, thread_id: r.req_str("threadId")?, enabled: r.req_bool("enabled")? },
            "thread.pin" => Self::ThreadPin { command_id: c, thread_id: r.req_str("threadId")?, order_key: r.opt_str("orderKey")? },
            "thread.unpin" => Self::ThreadUnpin { command_id: c, thread_id: r.req_str("threadId")? },
            "thread.pin.reorder" => Self::ThreadPinReorder { command_id: c, thread_id: r.req_str("threadId")?, order_key: r.req_str("orderKey")? },
            "thread.mark-unread" => Self::ThreadMarkUnread { command_id: c, thread_id: r.req_str("threadId")? },
            "thread.visit" => Self::ThreadVisit { command_id: c, thread_id: r.req_str("threadId")?, visited_at: r.req_str("visitedAt")? },
            "queued-run.cancel" => Self::QueuedRunCancel { command_id: c, thread_id: r.req_str("threadId")?, run_id: r.req_str("runId")? },
            "queued-run.reorder" => Self::QueuedRunReorder {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                run_id: r.req_str("runId")?,
                before_run_id: r.nullable_str("beforeRunId")?.ok_or_else(|| ids::invalid("beforeRunId"))?,
            },
            "queued-run.edit" => Self::QueuedRunEdit {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                run_id: r.req_str("runId")?,
                text: r.req_str("text")?,
                attachments: r.req_array("attachments")?,
                context: r.opt_value("context"),
            },
            "queued-message.promote-to-steer" => Self::QueuedMessagePromoteToSteer {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                queued_run_id: r.req_str("queuedRunId")?,
                target_run_id: r.req_str("targetRunId")?,
            },
            "queue.resume" => Self::QueueResume { command_id: c, thread_id: r.req_str("threadId")? },
            "runtime-request.respond" => {
                let thread_id = r.req_str("threadId")?;
                let request_id = r.req_str("requestId")?;
                let decision = r.opt_str("decision")?;
                let answers = r.opt_value("answers");
                let response = match (decision, answers) {
                    (Some(d), None) => RuntimeResponse::Decision(d),
                    (None, Some(Value::Object(a))) => RuntimeResponse::Answers(a),
                    (None, Some(_)) => return Err(ids::invalid("answers")),
                    _ => return Err("invalid_input".into()),
                };
                Self::RuntimeRequestRespond { command_id: c, thread_id, request_id, response }
            }
            "thread.user-input.dismiss" => Self::ThreadUserInputDismiss { command_id: c, thread_id: r.req_str("threadId")?, request_id: r.req_str("requestId")? },
            "checkpoint.rollback" => Self::CheckpointRollback {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                scope_id: r.req_str("scopeId")?,
                checkpoint_id: r.req_str("checkpointId")?,
                restore_files: r.opt_bool("restoreFiles")?,
            },
            "thread.pull-request.link" => Self::ThreadPullRequestLink {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                key: pull_request_key(&mut r)?,
                url: r.req_str("url")?,
                source: r.req_str("source")?,
                repository_web_url: r.opt_str("repositoryWebUrl")?,
            },
            "thread.pull-request.unlink" => {
                Self::ThreadPullRequestUnlink { command_id: c, thread_id: r.req_str("threadId")?, key: pull_request_key(&mut r)? }
            }
            "thread.pull-request.watch" => Self::ThreadPullRequestWatch {
                command_id: c,
                thread_id: r.req_str("threadId")?,
                key: pull_request_key(&mut r)?,
                watching: r.req_bool("watching")?,
                link: r.opt_value("link"),
            },
            _ => return Err(ids::invalid("type")),
        };
        r.done()?;
        command.payload()?;
        Ok(command)
    }
}

fn queued(command_id: &str, thread_id: &str, run_id: &str, mut input: Value) -> Result<Value> {
    ids::id("commandId", command_id)?;
    ids::id("threadId", thread_id)?;
    ids::id("runId", run_id)?;
    input["thread"] = json!(thread_id);
    input["runId"] = json!(run_id);
    let mut v = queue::command(&input)?;
    v["commandId"] = json!(command_id);
    Ok(v)
}
#[allow(clippy::too_many_arguments)]
fn fork(
    kind: &str,
    command_id: &str,
    source: &str,
    target: &str,
    point: &SourcePoint,
    created_at: &Option<String>,
    created_by: &str,
    creation_source: &str,
) -> Result<Value> {
    let point = match point {
        SourcePoint::LatestStable => json!({"type":"latest_stable"}),
        SourcePoint::Run { run_id } => json!({"type":"run","runId":ids::id("runId", run_id)?}),
        SourcePoint::Checkpoint { checkpoint_id } => json!({"type":"checkpoint","checkpointId":ids::id("checkpointId", checkpoint_id)?}),
    };
    let mut v = json!({"type":kind,"commandId":ids::id("commandId", command_id)?,"sourceThreadId":ids::id("sourceThreadId", source)?,
        "targetThreadId":ids::id("targetThreadId", target)?,"sourcePoint":point,
        "createdBy":one_of("createdBy", created_by, ACTORS)?,"creationSource":one_of("creationSource", creation_source, CREATION_SOURCES)?});
    if let Some(at) = created_at {
        v["createdAt"] = json!(timestamp("createdAt", at)?);
    }
    Ok(v)
}
fn pull_request(kind: &str, command_id: &str, thread_id: &str, key: &PullRequestKey) -> Result<Value> {
    if key.number == 0 {
        return Err(ids::invalid("number"));
    }
    Ok(json!({"type":kind,"commandId":ids::id("commandId", command_id)?,"threadId":ids::id("threadId", thread_id)?,
        "host":ids::text("host", &key.host, 255)?,"repository":ids::text("repository", &key.repository, 512)?,"number":key.number}))
}
fn pull_request_key(r: &mut Reader) -> Result<PullRequestKey> {
    Ok(PullRequestKey {
        host: r.req_str("host")?,
        repository: r.req_str("repository")?,
        number: r.req_u64("number")?,
    })
}
fn source_point(v: &Value) -> Result<SourcePoint> {
    let invalid = || ids::invalid("sourcePoint");
    let mut r = Reader::new(v).map_err(|_| invalid())?;
    let point = match r.req_str("type").map_err(|_| invalid())?.as_str() {
        "latest_stable" => SourcePoint::LatestStable,
        "run" => SourcePoint::Run { run_id: r.req_str("runId")? },
        "checkpoint" => SourcePoint::Checkpoint { checkpoint_id: r.req_str("checkpointId")? },
        _ => return Err(invalid()),
    };
    r.done()?;
    Ok(point)
}
fn dispatch_mode(v: &Value) -> Result<DispatchMode> {
    let invalid = || ids::invalid("dispatchMode");
    let mut r = Reader::new(v).map_err(|_| invalid())?;
    let mode = match r.req_str("type").map_err(|_| invalid())?.as_str() {
        "defer_start" => DispatchMode::DeferStart {
            workspace_strategy: r.opt_value("workspaceStrategy").map(|w| WorkspaceStrategy::from_json(&w)).transpose()?,
        },
        "steer_active" => DispatchMode::SteerActive { target_run_id: r.req_str("targetRunId")? },
        "restart_active" => DispatchMode::RestartActive { target_run_id: r.req_str("targetRunId")? },
        "queue_after_active" => DispatchMode::QueueAfterActive,
        "start_immediately" => DispatchMode::StartImmediately,
        _ => return Err(invalid()),
    };
    r.done()?;
    Ok(mode)
}

fn put(v: &mut Value, key: &str, value: Option<bool>) {
    if let Some(b) = value {
        v[key] = json!(b);
    }
}
fn object(field: &str, v: &Value) -> Result<Value> {
    Some(v).filter(|v| v.is_object()).cloned().ok_or_else(|| ids::invalid(field))
}
fn one_of<'a>(field: &str, value: &'a str, allowed: &[&str]) -> Result<&'a str> {
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(ids::invalid(field))
    }
}
fn runtime_mode(mode: &str) -> Result<&str> {
    if models::valid_runtime_mode(mode) {
        Ok(mode)
    } else {
        Err(ids::invalid("runtimeMode"))
    }
}
fn timestamp<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    chrono::DateTime::parse_from_rfc3339(value).map(|_| value).map_err(|_| ids::invalid(field))
}
fn free_text<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    if value.chars().count() > 4096 || value.contains('\0') {
        Err(ids::invalid(field))
    } else {
        Ok(value)
    }
}
fn attachments(field: &str, files: &[Value]) -> Result<Value> {
    if files.len() > ATTACHMENTS_MAX || !files.iter().all(Value::is_object) {
        return Err(ids::invalid(field));
    }
    Ok(json!(files))
}
/// Message text as ot3 sends it: inline attachment references become their plain names.
fn message_text(field: &str, text: &str, files: &[Value]) -> Result<String> {
    if text.chars().count() > TEXT_MAX || text.contains('\0') || (text.trim().is_empty() && files.is_empty()) {
        return Err(ids::invalid(field));
    }
    Ok(crate::composer::legacy_attachment_text(text, files))
}

/// Strict member reader: each read marks the member used; `done` rejects the rest.
struct Reader<'a> {
    map: &'a Map<String, Value>,
    used: Vec<&'static str>,
}
impl<'a> Reader<'a> {
    fn new(v: &'a Value) -> Result<Self> {
        Ok(Self { map: v.as_object().ok_or("invalid_input")?, used: vec![] })
    }
    fn raw(&mut self, k: &'static str) -> Option<&'a Value> {
        self.used.push(k);
        self.map.get(k)
    }
    fn req_str(&mut self, k: &'static str) -> Result<String> {
        self.opt_str(k)?.ok_or_else(|| ids::invalid(k))
    }
    fn opt_str(&mut self, k: &'static str) -> Result<Option<String>> {
        match self.raw(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(ids::invalid(k)),
        }
    }
    /// Present-and-null → `Some(None)`; absent → `None`.
    fn nullable_str(&mut self, k: &'static str) -> Result<Option<Option<String>>> {
        match self.raw(k) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(None)),
            Some(Value::String(s)) => Ok(Some(Some(s.clone()))),
            Some(_) => Err(ids::invalid(k)),
        }
    }
    fn nullable_value(&mut self, k: &'static str) -> Option<Value> {
        self.raw(k).cloned()
    }
    fn opt_bool(&mut self, k: &'static str) -> Result<Option<bool>> {
        match self.raw(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(_) => Err(ids::invalid(k)),
        }
    }
    fn req_bool(&mut self, k: &'static str) -> Result<bool> {
        self.opt_bool(k)?.ok_or_else(|| ids::invalid(k))
    }
    fn req_u64(&mut self, k: &'static str) -> Result<u64> {
        self.raw(k).and_then(Value::as_u64).ok_or_else(|| ids::invalid(k))
    }
    fn opt_value(&mut self, k: &'static str) -> Option<Value> {
        self.raw(k).filter(|v| !v.is_null()).cloned()
    }
    fn req_value(&mut self, k: &'static str) -> Result<Value> {
        self.opt_value(k).ok_or_else(|| ids::invalid(k))
    }
    fn req_array(&mut self, k: &'static str) -> Result<Vec<Value>> {
        self.raw(k).and_then(Value::as_array).cloned().ok_or_else(|| ids::invalid(k))
    }
    fn literal(&mut self, k: &'static str, expected: &str) -> Result<()> {
        if self.raw(k).and_then(Value::as_str) == Some(expected) {
            Ok(())
        } else {
            Err(ids::invalid(k))
        }
    }
    fn done(self) -> Result<()> {
        if self.map.keys().all(|k| self.used.contains(&k.as_str())) {
            Ok(())
        } else {
            Err("invalid_input".into())
        }
    }
}
