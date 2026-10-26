//! Domain operations: the library's pure presentation and command builders
//! behind one registry, so a host without Rust (Kotlin through JNI) calls them
//! by name with JSON instead of re-deriving T3's rules. No connection and no
//! network; whatever depends on the current time takes `now` as an argument.
//! The JSON ABI reaches them with `{"op":"domain","fn":"queue.derive","args":{…}}`
//! and `describe` lists them under `domain`.
use crate::api::{ids, Arg, ArgType, Args};
use crate::{activity, approvals, awareness, changes, code_blocks, composer, connection_catalog, history, i18n, markdown, models, projection, projects, questions, queue, sidebar, timeline, Result};
use serde_json::{json, Value};

mod git;
mod presentation;
mod review;
mod settings;
mod usage;

/// One registry row. Built with `domain_fn!`.
#[derive(Clone, Copy)]
pub struct DomainFn {
    /// `<module>.<function>`, camelCase.
    pub name: &'static str,
    pub module: &'static str,
    pub args: &'static [Arg],
    /// What comes back, in one line, as `describe` publishes it.
    pub returns: &'static str,
    run: fn(&Args) -> Result<Value>,
}
impl DomainFn {
    /// Validate `args` against the declared members and their JSON types, then run.
    /// Unknown members are `invalid_input`; a missing or mistyped one is `invalid_<field>`.
    pub fn call(&self, args: &Value) -> Result<Value> {
        let args = Args::checked(args, self.args, false)?;
        for arg in self.args {
            if let Some(value) = args.get(arg.name) {
                if !typed(arg.ty, value) {
                    return Err(ids::invalid(arg.name));
                }
            }
        }
        (self.run)(&args)
    }
    pub fn describe(&self) -> Value {
        let args = self
            .args
            .iter()
            .map(|a| {
                let mut v = json!({"name":a.name,"type":a.ty.as_str(),"required":a.required});
                if let ArgType::Enum(values) = a.ty {
                    v["values"] = json!(values);
                }
                v
            })
            .collect::<Vec<_>>();
        json!({"fn":self.name,"module":self.module,"args":args,"returns":self.returns})
    }
}
/// The JSON type a declared argument must have. Content rules stay with each function.
fn typed(ty: ArgType, value: &Value) -> bool {
    match ty {
        ArgType::Bool => value.is_boolean(),
        ArgType::Integer => value.is_i64() || value.is_u64(),
        ArgType::Number => value.is_number(),
        ArgType::Object | ArgType::Command => value.is_object(),
        ArgType::Array => value.is_array(),
        ArgType::Enum(values) => value.as_str().is_some_and(|s| values.contains(&s)),
        ArgType::Id | ArgType::Slug | ArgType::Token | ArgType::Opaque | ArgType::Path | ArgType::Text => value.is_string(),
    }
}

/// Build a registry row:
/// `domain_fn!{name: "queue.derive", module: "queue", args: [(projection, Object, required)],
/// returns: "object {items, …}", run: |a| Ok(queue::derive(a.req_object("projection")?))}`.
macro_rules! domain_fn {
    (name: $name:literal, module: $module:literal, args: [$(($arg:ident, $ty:expr, $req:ident)),* $(,)?],
     returns: $returns:literal, run: $run:expr $(,)?) => {
        $crate::domain::DomainFn {
            name: $name,
            module: $module,
            args: &[$($crate::api::Arg {
                name: stringify!($arg),
                ty: {
                    #[allow(unused_imports)]
                    use $crate::api::ArgType::*;
                    $ty
                },
                required: $crate::domain::domain_fn!(@required $req),
            }),*],
            returns: $returns,
            run: $run,
        }
    };
    (@required required) => { true };
    (@required optional) => { false };
}
pub(crate) use domain_fn;

const SNOOZE_ACTIONS: &[&str] = &["settle", "unsettle", "snooze", "unsnooze"];
const THREAD_ACTIONS: &[&str] = &["thread-rename", "regenerate-title", "pin", "unpin", "mark-unread", "auto-settle", "archive", "delete-thread"];
const QUEUE_ACTIONS: &[&str] = &["cancel", "reorder", "steer", "resume", "edit"];
const DECISIONS: &[&str] = &["accept", "acceptForSession", "acceptAlways", "decline", "cancel"];
const QUESTION_ACTIONS: &[&str] = &["respond", "dismiss"];
const SUGGESTION_KINDS: &[&str] = &["skill", "command"];
const GROUPING_MODES: &[&str] = &["repository", "repository_path", "separate"];
pub(crate) static NULL: Value = Value::Null;

fn sidebar_view(a: &Args) -> Result<Value> {
    let now = a.i64("now")?.unwrap_or(0);
    let snapshot = sidebar::view::snapshot(a.req_array("environments")?, now)?;
    let state = sidebar::view::ViewState {
        filter: a.str("filter")?.unwrap_or("all"),
        project: a.str("project")?.unwrap_or(""),
        environment: a.str("environmentId")?.filter(|e| !e.is_empty()),
        query: a.str("query")?.unwrap_or(""),
        grouped: a.bool("grouped")?.unwrap_or(false),
        subthreads_expanded: a.get("subthreadsExpanded").unwrap_or(&NULL),
        selected: a.str("selectedKey")?.unwrap_or(""),
        returns: a.get("returns").unwrap_or(&NULL),
        grouping: sidebar::view::Grouping {
            mode: a.str("groupingMode")?,
            overrides: a.get("groupingOverrides").unwrap_or(&NULL),
            native: a.get("nativeGrouping").unwrap_or(&NULL),
        },
    };
    let mut view = sidebar::view::view(&snapshot, &state, now);
    let identities = projects::identities(snapshot["projects"].as_array().map(Vec::as_slice).unwrap_or(&[]), a.get("colors").unwrap_or(&NULL), a.str("accent")?);
    view["projectIdentities"] = json!(identities.into_iter().map(|i| (i["key"].as_str().unwrap_or("").to_string(), i)).collect::<serde_json::Map<_, _>>());
    Ok(view)
}

fn awareness_view(a: &Args) -> Result<Value> {
    let on_screen = match a.get("onScreen") {
        None => None,
        Some(s) => {
            let id = |k: &str| s[k].as_str().filter(|v| !v.is_empty());
            Some(id("environmentId").zip(id("threadId")).ok_or("invalid_onScreen")?)
        }
    };
    awareness::view(&awareness::Input {
        environments: a.req_array("environments")?,
        now: a.i64("now")?.unwrap_or(0),
        baseline: a.get("baseline"),
        on_screen,
        preferences: awareness::Preferences::from_json(a.get("preferences"))?,
        lifecycle: a.str("lifecycle")?.unwrap_or("running"),
        locale: a.str("locale")?.unwrap_or("en"),
    })
}

pub const FUNCTIONS: &[DomainFn] = &[
    domain_fn! {
        name: "projection.apply", module: "projection",
        args: [(projection, Object, required), (event, Object, optional), (events, Array, optional)],
        returns: "object {projection, changed, applied}: `event` then each of `events` applied in order",
        run: |a| {
            let mut p = a.req_object("projection")?.clone();
            let events = a.get("event").into_iter().chain(a.array("events")?.into_iter().flatten()).collect::<Vec<_>>();
            if events.is_empty() {
                return Err("invalid_event".into());
            }
            let mut applied=0;for e in events {if projection::try_apply(&mut p,&e)? {applied+=1;}}
            Ok(json!({"projection":p,"changed":applied > 0,"applied":applied}))
        },
    },
    domain_fn! {
        name: "sidebar.status", module: "sidebar",
        args: [(thread, Object, required)],
        returns: "string: approval, input, waiting, working, goal, limited, failed or ready",
        run: |a| Ok(json!(sidebar::sidebar_status(a.req_object("thread")?))),
    },
    domain_fn! {
        name: "sidebar.metadata", module: "sidebar",
        args: [(thread, Object, required), (now, Integer, required)],
        returns: "object {sidebarStatus, woke, wokeAt, snoozed, canSnooze, snoozedUntil, hasRuntimeError, runtimeErrorClass, providerHistory, pullRequests, linkedPullRequest, branchPullRequest, pullRequestBadge}",
        run: |a| Ok(sidebar::metadata(a.req_object("thread")?, a.i64("now")?.unwrap_or(0))),
    },
    domain_fn! {
        name: "sidebar.sortTimestamps", module: "sidebar",
        args: [(thread, Object, required)],
        returns: "object {inboxAt, activeAnchorAt, lastSentAt, settledSortAt} (ms)",
        run: |a| Ok(sidebar::sort_timestamps(a.req_object("thread")?)),
    },
    domain_fn! {
        name: "sidebar.command", module: "sidebar",
        args: [(action, Enum(SNOOZE_ACTIONS), required), (threadId, Id, required), (snoozedUntil, Text, optional)],
        returns: "command: thread.settle, thread.unsettle, thread.snooze or thread.unsnooze (without commandId)",
        run: |a| sidebar::command(a.req("action")?, a.req("threadId")?, a.str("snoozedUntil")?),
    },
    domain_fn! {
        name: "sidebar.threadAction", module: "sidebar",
        args: [
            (thread, Id, required), (action, Enum(THREAD_ACTIONS), required), (title, Text, optional),
            (enabled, Bool, optional), (confirmed, Bool, optional), (working, Bool, optional),
        ],
        returns: "command for the thread actions menu (without commandId)",
        run: |a| sidebar::thread_action(a.all()),
    },
    domain_fn! {
        name: "sidebar.snapshot", module: "sidebar",
        args: [(environments, Array, required), (now, Integer, required)],
        returns: "object {environments [{id, name, state}], stale, projects, threads, working, pending}: each environment {id, name?, shell?, state?} as ot3's rows, newest first",
        run: |a| sidebar::view::snapshot(a.req_array("environments")?, a.i64("now")?.unwrap_or(0)),
    },
    domain_fn! {
        name: "sidebar.view", module: "sidebar",
        args: [
            (environments, Array, required), (now, Integer, required), (filter, Enum(sidebar::view::FILTERS), optional),
            (project, Text, optional), (environmentId, Text, optional), (query, Text, optional), (grouped, Bool, optional),
            (subthreadsExpanded, Object, optional), (selectedKey, Text, optional), (returns, Object, optional),
            (groupingMode, Enum(GROUPING_MODES), optional), (groupingOverrides, Object, optional), (nativeGrouping, Object, optional),
            (colors, Object, optional), (accent, Text, optional),
        ],
        returns: "object {sections {active, working, snoozed, settled}, sectionCounts, counts, filterCounts, projects, projectGroups, projectScope, projectIdentities {key: identity}, environments, stale, working, pending, threads, returns}; rows carry detail, state, statusLabel, statusRole and time",
        run: sidebar_view,
    },
    domain_fn! {
        name: "sidebar.detail", module: "sidebar",
        args: [(thread, Object, required), (now, Integer, optional)],
        returns: "object {detail, state, statusLabel, statusRole, time} of one sidebar row",
        run: |a| {
            let t = a.req_object("thread")?;
            let detail = sidebar::view::detail(t);
            Ok(json!({"detail":detail,"state":sidebar::view::state(t),"statusLabel":sidebar::view::label(detail),
                "statusRole":sidebar::view::role(detail),"time":sidebar::view::time(t, a.i64("now")?.unwrap_or(0))}))
        },
    },
    domain_fn! {
        name: "awareness.view", module: "awareness",
        args: [
            (environments, Array, required), (now, Integer, required), (baseline, Object, optional), (onScreen, Object, optional),
            (preferences, Object, optional), (lifecycle, Enum(awareness::LIFECYCLES), optional), (locale, Text, optional),
        ],
        returns: "object {counts, rows [{route {environmentId, threadId}, phase, title, project, host, status, since?, expiresAt}], card {active, phase, title, subtitle, status, chip? {kind working, live, approve or answer, count}, since?, expiresAt, route, rows (at most 5)} or null, alerts [{id, group attention or finished, phase, title, body, status, route, expiresAt?}], summaries [{group, title, body, lines, route}], baseline, nextDeadline}: T3's phases, card and lifetimes and ot3's alert key over sidebar.view's environments; pass baseline back on the next call and call again at nextDeadline (ms); texts in locale",
        run: awareness_view,
    },
    domain_fn! {
        name: "projects.identity", module: "projects",
        args: [(projects, Array, required), (colors, Object, optional), (accent, Text, optional)],
        returns: "array of {key, monogram, label, kind, icon, colorSlot, colorKey, color, fill}, one per project, slots over all of them",
        run: |a| Ok(json!(projects::identities(a.req_array("projects")?, a.get("colors").unwrap_or(&NULL), a.str("accent")?))),
    },
    domain_fn! {
        name: "projects.monogram", module: "projects",
        args: [(name, Text, required)],
        returns: "string: the two-letter monogram of a project title",
        run: |a| Ok(json!(projects::monogram(a.req("name")?))),
    },
    domain_fn! {
        name: "timeline.view", module: "timeline",
        args: [
            (projection, Object, required), (historyCursor, Opaque, optional), (hasMoreHistory, Bool, optional),
            (payloadBudgetExceeded, Bool, optional), (mathRenderingEnabled, Bool, optional),
        ],
        returns: "object {timeline [rows with role user, assistant, activity, changes, context, plan, secret, subagents or error, each with rev; assistant rows and plans carry markdown.blocks], threadState, activity, approvals, questions, tasks, queue, planView, historyCursor, hasMoreHistory, payloadBudgetExceeded}",
        run: |a| {
            let mut view = timeline::view(a.req_object("projection")?, a.all(), &timeline::Render::neutral(a.bool("mathRenderingEnabled")?.unwrap_or(false)))?;
            // The message rows are the timeline's user and assistant rows.
            view.as_object_mut().unwrap().remove("messages");
            Ok(view)
        },
    },
    domain_fn! {
        name: "timeline.liveActivity", module: "timeline",
        args: [(projection, Object, required)],
        returns: "array of the active run's in-flight activity {id, type, category, title, command, runId, status, failed, active, inspector}, newest first (at most 3)",
        run: |a| Ok(timeline::live_activity(a.req_object("projection")?)),
    },
    domain_fn! {
        name: "activity.describe", module: "activity",
        args: [(row, Object, required)],
        returns: "object {id, type, category, title, command, runId, status, failed, active} of a turn item (or its projected row), or null for messages, checkpoints, plans and errors; never the tool's input or output",
        run: |a| Ok(activity::describe(a.req_object("row")?).unwrap_or(Value::Null)),
    },
    domain_fn! {
        name: "changes.summaries", module: "changes",
        args: [(projection, Object, required)],
        returns: "array of {id, runId, capturedAt, fromTurnCount, toTurnCount, files [{path, kind, additions, deletions}], additions, deletions}, one per run with ready checkpoint files",
        run: |a| Ok(json!(changes::summaries(a.req_object("projection")?))),
    },
    domain_fn! {
        name: "markdown.blocks", module: "markdown",
        args: [(markdown, Text, required), (mathRenderingEnabled, Bool, optional)],
        returns: "array of blocks {type: paragraph, heading, list, quote, code, table, image, math or rule} with flat inline spans (text with strong/emphasis/strikethrough/link, code, break, image, math); links classified github, web, thread, file or anchor; unsafe links dropped",
        run: |a| Ok(json!(markdown::blocks(a.req("markdown")?, a.bool("mathRenderingEnabled")?.unwrap_or(false)))),
    },
    domain_fn! {
        name: "markdown.link", module: "markdown",
        args: [(href, Text, required)],
        returns: "object {href, kind: github, web, thread, file or anchor, fileType?} or null when ot3 would not follow the link",
        run: |a| Ok(markdown::link(a.req("href")?).unwrap_or(Value::Null)),
    },
    domain_fn! {
        name: "queue.derive", module: "queue",
        args: [(projection, Object, required)],
        returns: "object {items, hiddenMessageIds, acknowledgedMessageIds, held, activeRunId, canReorder, canSteer}",
        run: |a| Ok(queue::derive(a.req_object("projection")?)),
    },
    domain_fn! {
        name: "queue.command", module: "queue",
        args: [
            (thread, Id, required), (action, Enum(QUEUE_ACTIONS), required), (runId, Id, optional),
            (beforeRunId, Id, optional), (targetRunId, Id, optional), (text, Text, optional),
            (attachments, Array, optional), (context, Object, optional),
        ],
        returns: "command: queued-run.cancel/reorder/edit, queued-message.promote-to-steer or queue.resume (without commandId)",
        run: |a| queue::command(a.all()),
    },
    domain_fn! {
        name: "approvals.pending", module: "approvals",
        args: [(projection, Object, required)],
        returns: "array of {requestId, requestKind, createdAt, responseCapability, detail?, appName?, options}, oldest first",
        run: |a| Ok(approvals::pending(a.req_object("projection")?)),
    },
    domain_fn! {
        name: "approvals.command", module: "approvals",
        args: [(thread, Id, required), (runtimeRequestId, Opaque, required), (decision, Enum(DECISIONS), required), (projection, Object, required)],
        returns: "command runtime-request.respond, only for a pending live request and an offered decision",
        run: |a| approvals::command(a.all(), a.req_object("projection")?),
    },
    domain_fn! {
        name: "questions.pending", module: "questions",
        args: [(projection, Object, required)],
        returns: "array of {requestId, createdAt, questions, responseCapability, dismissible}, oldest first",
        run: |a| Ok(questions::pending(a.req_object("projection")?)),
    },
    domain_fn! {
        name: "questions.command", module: "questions",
        args: [(thread, Id, required), (runtimeRequestId, Opaque, required), (action, Enum(QUESTION_ACTIONS), required), (answers, Object, optional)],
        returns: "command runtime-request.respond (answers) or thread.user-input.dismiss",
        run: |a| questions::command(a.all()),
    },
    domain_fn! {
        name: "models.catalog", module: "models",
        args: [(config, Object, required)],
        returns: "object {models, providers, favorites, favoritesReady, capabilities, scratchWorkspaceRoot} from server.getConfig",
        run: |a| Ok(models::catalog(a.req_object("config")?)),
    },
    domain_fn! {
        name: "models.preferences", module: "models",
        args: [(settings, Object, required)],
        returns: "object {favorites, favoritesReady} from server.getSettings",
        run: |a| Ok(models::preferences(a.req_object("settings")?)),
    },
    domain_fn! {
        name: "models.favoritePayload", module: "models",
        args: [(provider, Text, required), (model, Text, required), (favorite, Bool, required)],
        returns: "object {patch} for server.updateSettings",
        run: |a| models::favorite_payload(a.req("provider")?, a.req("model")?, a.bool("favorite")?.unwrap_or(false)),
    },
    domain_fn! {
        name: "models.runtimeModeCommand", module: "models",
        args: [(threadId, Id, required), (commandId, Id, required), (runtimeMode, Enum(models::RUNTIME_MODES), required)],
        returns: "command thread.runtime-mode.set",
        run: |a| models::runtime_mode_command(a.req("threadId")?, a.req("commandId")?, a.req("runtimeMode")?),
    },
    domain_fn! {
        name: "composer.paths", module: "composer",
        args: [(result, Object, required)],
        returns: "array of {kind, label, description, insert, icon} (at most 40) from projects.searchEntries",
        run: |a| Ok(json!(composer::paths(a.req_object("result")?))),
    },
    domain_fn! {
        name: "composer.catalog", module: "composer",
        args: [(provider, Object, required), (cwd, Path, required), (kind, Enum(SUGGESTION_KINDS), required), (query, Text, optional)],
        returns: "array of {kind, label, description, insert, icon} (at most 40): skills and slash commands",
        run: |a| Ok(json!(composer::catalog(a.req_object("provider")?, a.req("cwd")?, a.req("kind")?, a.str("query")?.unwrap_or("")))),
    },
    domain_fn! {
        name: "composer.fileLink", module: "composer",
        args: [(path, Path, required)],
        returns: "string: the Markdown file link T3's composer inserts",
        run: |a| Ok(json!(composer::file_link(a.req("path")?))),
    },
    domain_fn! {
        name: "composer.attachmentReference", module: "composer",
        args: [(attachment, Object, required)],
        returns: "string: the inline t3-context reference of an uploaded attachment",
        run: |a| Ok(json!(composer::attachment_reference(a.req_object("attachment")?))),
    },
    domain_fn! {
        name: "history.path", module: "history",
        args: [(threadId, Id, required), (cursor, Opaque, required)],
        returns: "string: the history page path for request",
        run: |a| Ok(json!(history::path(a.req("threadId")?, a.req("cursor")?)?)),
    },
    domain_fn! {
        name: "history.envelope", module: "history",
        args: [(page, Object, required), (threadId, Id, required)],
        returns: "object {projection {thread, messages, turnItems}, historyCursor, hasMoreHistory}",
        run: |a| history::envelope(a.req_object("page")?, a.req("threadId")?),
    },
    domain_fn! {
        name: "connectionCatalog.pairingTarget", module: "connectionCatalog",
        args: [(link, Text, required), (pairingCode, Text, optional)],
        returns: "object {httpBaseUrl, pairingCode}: a pairing link, QR payload or host plus code",
        run: |a| {
            let (base, code) = connection_catalog::pairing_target(a.req("link")?, a.str("pairingCode")?.unwrap_or(""))?;
            Ok(json!({"httpBaseUrl":base,"pairingCode":code}))
        },
    },
    domain_fn! {
        name: "codeBlocks.parse", module: "codeBlocks",
        args: [(markdown, Text, required)],
        returns: "array of {index, language, code, command}: command only for a complete single-line shell fence",
        run: |a| Ok(json!(code_blocks::parse(a.req("markdown")?))),
    },
    domain_fn! {
        name: "i18n.translate", module: "i18n",
        args: [(catalog, Object, required), (locale, Text, required), (source, Text, required)],
        returns: "string: the translation, the English source as fallback",
        run: |a| Ok(json!(i18n::translate(a.req_object("catalog")?, a.req("locale")?, a.req("source")?))),
    },
    domain_fn! {
        name: "i18n.normalize", module: "i18n",
        args: [(locale, Text, required)],
        returns: "string: a POSIX or BCP 47 locale as lowercase BCP 47 (fr_CA.UTF-8 → fr-ca)",
        run: |a| Ok(json!(i18n::normalize(a.req("locale")?))),
    },
];

/// The registry: this file's functions, then each area's.
const MODULES: &[&[DomainFn]] = &[FUNCTIONS, review::FUNCTIONS, usage::FUNCTIONS, git::FUNCTIONS, settings::FUNCTIONS, presentation::FUNCTIONS];
/// Every domain function, in a stable order.
pub fn functions() -> impl Iterator<Item = &'static DomainFn> {
    MODULES.iter().flat_map(|m| m.iter())
}
pub fn function(name: &str) -> Option<&'static DomainFn> {
    functions().find(|f| f.name == name)
}
/// The `domain` section of `describe`.
pub fn describe() -> Value {
    json!(functions().map(DomainFn::describe).collect::<Vec<_>>())
}
/// Call a domain function by name with JSON arguments (what `{"op":"domain"}` does).
pub fn call(name: &str, args: &Value) -> Result<Value> {
    function(name).ok_or("unknown_function")?.call(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_names_are_unique_and_prefixed_by_their_module() {
        let mut names = functions().map(|f| f.name).collect::<Vec<_>>();
        let count = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), count);
        for f in functions() {
            assert!(f.name.starts_with(&format!("{}.", f.module)), "{}", f.name);
            let mut args = f.args.iter().map(|a| a.name).collect::<Vec<_>>();
            args.sort();
            args.dedup();
            assert_eq!(args.len(), f.args.len(), "{}", f.name);
            assert!(!f.returns.is_empty());
        }
    }
    #[test]
    fn arguments_are_checked_by_name_and_json_type() {
        assert_eq!(call("nope.nope", &json!({})).unwrap_err(), "unknown_function");
        assert_eq!(call("codeBlocks.parse", &json!([])).unwrap_err(), "invalid_input");
        assert_eq!(call("codeBlocks.parse", &json!({})).unwrap_err(), "invalid_markdown");
        assert_eq!(call("codeBlocks.parse", &json!({"markdown":7})).unwrap_err(), "invalid_markdown");
        assert_eq!(call("codeBlocks.parse", &json!({"markdown":"x","extra":1})).unwrap_err(), "invalid_input");
        assert_eq!(call("sidebar.command", &json!({"action":"nap","threadId":"t"})).unwrap_err(), "invalid_action");
        assert_eq!(call("sidebar.metadata", &json!({"thread":{},"now":1.5})).unwrap_err(), "invalid_now");
        assert_eq!(call("projection.apply", &json!({"projection":{}})).unwrap_err(), "invalid_event");
    }
}
