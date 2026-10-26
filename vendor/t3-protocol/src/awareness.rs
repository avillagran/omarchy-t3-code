//! Thread awareness for notifications and Live Updates (`awareness.view`):
//! T3's agent phases per thread, the ongoing card with its status bar chip,
//! attention and finished alerts deduplicated against a baseline the host
//! keeps, and absolute deadlines. Pure: the host passes the environments'
//! shells (as `sidebar.view` takes them), `now` and the previous `baseline`,
//! posts what comes back and stores the new `baseline` for the next call.
//!
//! Ported, not reinvented:
//! - Phases: T3 `packages/shared/src/agentAwareness.ts` (`projectThreadAwarenessV2`):
//!   starting, running, waiting_for_approval, waiting_for_input, completed and
//!   failed; subagents, completions whose stop was requested and settled
//!   finished threads have none. `stale`, T3's seventh phase, which its
//!   projection never produces, is a live phase of an environment that is not
//!   connected: its cached shell may be out of date.
//! - Card: T3 relay `infra/relay/src/agentActivity/agentActivityAggregate.ts`
//!   (`makeAggregateState`: live rows by priority, then the newest results, at
//!   most five), `fcmPayloads.ts` (`androidActivityData`: priority order, the
//!   latest expiry) and T3 Mobile's `AgentActivityPresentation.kt` (title,
//!   subtitle, chip).
//! - Lifetimes: `agentActivityPayloads.ts` (starting and running 2 h, waiting
//!   24 h after `updatedAt`) and `agentActivityAggregate.ts` (finished 15 min).
//! - Alerts: ot3 `src/main.rs` `notify` (the key, first observation silent) with
//!   T3 relay `agentActivityAlerts.ts` (groups, 2-minute freshness, titles) and
//!   T3 Mobile `AgentNotifications.kt` (the thread on screen is consumed).
//!
//! **The alert key.** A thread alerts when ot3 would: its `sidebar.view` row
//! turns `pending`, or stays pending with a new `revision` (reason, run,
//! request and completion stamp). So ready plans and review completions alert as
//! in ot3, and a second request on a thread already waiting alerts again. T3
//! decides how: attention (approval, question, ready plan) or finished (done,
//! failed), a finished alert only while its result is at most 2 minutes old, one
//! summary per group with several alerts, and the lifetimes above. Nothing alerts
//! for an environment without its baseline entry (first observation, cold
//! start), while an environment is not ready, for subagents, for settled or
//! snoozed threads (never pending) or for the thread on screen; the new state is
//! still written to the baseline, so it never alerts later.
//!
//! **Beyond the sources.** A ready plan alerts as `waiting_for_input` (T3 reads
//! it as completed and never alerts it). The chip follows the first live row,
//! since Android's chip has no failed, done or waiting kind. A finished row is
//! anchored on `latestRunCompletedAt` (T3 publishes `updatedAt` once, at the
//! result), so later edits never extend it, and a row is gone at its
//! `expiresAt`. `nextDeadline` also covers a snooze that ends with a reason,
//! when ot3 wakes the thread. Texts come in the host's `locale`.
use crate::sidebar::view as sidebar;
use crate::util::{epoch, text};
use crate::{i18n, Result};
use serde_json::{json, Map, Value};
use std::cmp::Reverse;
use std::collections::HashMap;

/// The baseline's format. A baseline of another version, or malformed, is
/// replaced silently, like a first observation.
pub const BASELINE_VERSION: u64 = 1;
/// T3 `RUNNING_AGENT_ACTIVITY_ROW_TTL_MS`: starting and running rows.
pub const RUNNING_TTL_MS: i64 = 2 * 60 * 60 * 1000;
/// T3 `WAITING_AGENT_ACTIVITY_ROW_TTL_MS`: every other live row.
pub const WAITING_TTL_MS: i64 = 24 * 60 * 60 * 1000;
/// T3 `TERMINAL_AGENT_ACTIVITY_DISPLAY_TTL_MS`: done and failed rows.
pub const FINISHED_TTL_MS: i64 = 15 * 60 * 1000;
/// T3 `TERMINAL_NOTIFICATION_FRESHNESS_MS`: a finished alert's longest delay.
pub const FRESHNESS_MS: i64 = 2 * 60 * 1000;
/// T3 `MAX_ACTIVITY_ROWS`.
pub const CARD_ROWS: usize = 5;
pub const LIFECYCLES: &[&str] = &["cold", "resumed", "running"];
const TEXT_MAX: usize = 120;
const STATUS_MAX: usize = 40;

/// T3 `backgroundWorkHoldsCompletion`: a subagent, monitor or background task.
fn background_holds(t: &Value) -> bool {
    crate::sidebar::background_work_holds(&t["pendingBackgroundTasks"])
}
fn run_status(t: &Value) -> &str {
    t["activityRunStatus"].as_str().or_else(|| t["status"].as_str()).unwrap_or("")
}
/// T3 `resolveThreadAwarenessPhaseV2`. A status T3's schema does not define
/// has no phase.
fn resolve(t: &Value) -> Option<&'static str> {
    let run = run_status(t);
    let request = &t["pendingRuntimeRequest"];
    if request["kind"] == "user_input" && (request["blocking"] != false || !matches!(run, "preparing" | "starting" | "running")) {
        return Some("waiting_for_input");
    }
    if !request.is_null() && request["kind"] != "user_input" && request["kind"] != "auth_refresh" {
        return Some("waiting_for_approval");
    }
    match run {
        "preparing" | "starting" => Some("starting"),
        "running" | "waiting" => Some("running"),
        "completed" if background_holds(t) => Some("running"),
        "completed" => Some("completed"),
        "failed" => Some("failed"),
        _ => None,
    }
}
/// T3 `projectThreadAwarenessV2`'s phase for one shell thread, or `None` when
/// T3 shows nothing for it.
pub fn phase(t: &Value) -> Option<&'static str> {
    if t["lineage"]["relationshipToParent"] == "subagent" {
        return None;
    }
    let phase = resolve(t)?;
    if phase == "completed" && t["latestRunStopRequested"] == true {
        return None;
    }
    if t["settledOverride"] == "settled" && matches!(phase, "completed" | "failed") && !background_holds(t) {
        return None;
    }
    Some(phase)
}
/// T3 `activityPhasePriority`: approval and input, failed, starting and running, the rest.
pub fn priority(phase: &str) -> u8 {
    match phase {
        "waiting_for_approval" | "waiting_for_input" => 0,
        "failed" => 1,
        "starting" | "running" => 2,
        _ => 3,
    }
}
fn finished(phase: &str) -> bool {
    matches!(phase, "completed" | "failed")
}
/// When a row leaves the card: 2 h after `updatedAt` while starting or running,
/// 24 h while waiting, 15 min after the result once finished. Only the thread's
/// own stamps count, so replaying the same shell never moves it.
pub fn expires_at(phase: &str, updated_at: i64, result_at: i64) -> i64 {
    match phase {
        "starting" | "running" => updated_at + RUNNING_TTL_MS,
        "completed" | "failed" => result_at + FINISHED_TTL_MS,
        _ => updated_at + WAITING_TTL_MS,
    }
}

/// Which alerts the host wants (T3 `RelayAgentAwarenessPreferences`); `plan`
/// is ot3's ready plan and follows `input` unless given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub approval: bool,
    pub input: bool,
    pub plan: bool,
    pub completion: bool,
    pub failure: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Preferences { approval: true, input: true, plan: true, completion: true, failure: true }
    }
}
impl Preferences {
    /// `{approval?, input?, plan?, completion?, failure?}`, booleans, true when absent.
    pub fn from_json(v: Option<&Value>) -> Result<Self> {
        let Some(v) = v else { return Ok(Self::default()) };
        let map = v.as_object().ok_or("invalid_preferences")?;
        if map.keys().any(|k| !["approval", "input", "plan", "completion", "failure"].contains(&k.as_str())) {
            return Err("invalid_preferences".into());
        }
        let flag = |k: &str| match map.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(_) => Err("invalid_preferences"),
        };
        let input = flag("input")?.unwrap_or(true);
        Ok(Preferences {
            approval: flag("approval")?.unwrap_or(true),
            input,
            plan: flag("plan")?.unwrap_or(input),
            completion: flag("completion")?.unwrap_or(true),
            failure: flag("failure")?.unwrap_or(true),
        })
    }
}

/// One `awareness.view` call.
pub struct Input<'a> {
    /// `[{id, name?, shell?, state?}]`, as `sidebar.view` takes them.
    pub environments: &'a [Value],
    pub now: i64,
    /// The previous call's `baseline`.
    pub baseline: Option<&'a Value>,
    /// The thread the user is looking at: `(environmentId, threadId)`.
    pub on_screen: Option<(&'a str, &'a str)>,
    pub preferences: Preferences,
    /// `cold` drops the baseline (every environment is a first observation);
    /// `resumed` and `running` use it.
    pub lifecycle: &'a str,
    /// Locale list for the texts (`fr_CA.UTF-8:en`); English by default.
    pub locale: &'a str,
}

/// T3 `truncateText` after `fcmPayloads.ts`'s whitespace clean-up.
fn clean(s: &str, max: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= max {
        return s;
    }
    format!("{}...", s.chars().take(max - 3).collect::<String>().trim_end())
}
/// The row's status: ot3's label (`sidebar.view` `statusLabel`), else the
/// phase's in ot3's words; a stale row is Waiting, as T3 words it.
fn status_source(row: &Value, phase: &str) -> &'static str {
    let label = sidebar::label(sidebar::detail(row));
    match phase {
        "stale" => "Waiting",
        _ if !label.is_empty() => label,
        "waiting_for_approval" => "Approval",
        "waiting_for_input" => "Input",
        "completed" => "Done",
        "failed" => "Failed",
        _ => "Working",
    }
}
/// When the phase began, only from a stamp that stays put: the request's
/// creation, the run's start, the result.
fn since(phase: &str, raw: &Value, row: &Value) -> Option<i64> {
    let at = match phase {
        "waiting_for_approval" | "waiting_for_input" => epoch(&raw["pendingRuntimeRequest"]["createdAt"]),
        "starting" | "running" if matches!(run_status(raw), "preparing" | "starting" | "running" | "waiting") => row["startedAt"].as_i64().unwrap_or(0),
        "completed" | "failed" => row["latestRunCompletedAt"].as_i64().unwrap_or(0),
        _ => 0,
    };
    (at > 0).then_some(at)
}
/// The previous baseline's environments, unless it is of another version or malformed.
fn previous(baseline: Option<&Value>) -> Option<&Map<String, Value>> {
    let b = baseline?;
    (b["version"].as_u64() == Some(BASELINE_VERSION)).then(|| b["environments"].as_object()).flatten()
}

struct Row<'a> {
    env: &'a str,
    id: &'a str,
    order: usize,
    phase: &'static str,
    /// Newest first within a priority: the result once finished, else `updatedAt`.
    at: i64,
    expires_at: i64,
    since: Option<i64>,
    title: String,
    project: String,
    host: String,
    status: String,
}
impl Row<'_> {
    fn route(&self) -> Value {
        json!({"environmentId":self.env,"threadId":self.id})
    }
    fn json(&self) -> Value {
        let mut v = json!({"route":self.route(),"phase":self.phase,"title":self.title,"project":self.project,"host":self.host,"status":self.status,"expiresAt":self.expires_at});
        if let Some(since) = self.since {
            v["since"] = json!(since);
        }
        v
    }
}
struct Alert<'a> {
    id: &'a str,
    order: usize,
    attention: bool,
    phase: &'static str,
    at: i64,
    json: Value,
}

/// The whole view: `{counts, rows, card, alerts, summaries, baseline, nextDeadline}`.
pub fn view(input: &Input) -> Result<Value> {
    let now = input.now;
    let tr = |source: &str, n: Option<usize>| i18n::t(input.locale, source, n.map(|n| json!({"n":n})).as_ref());
    let snapshot = sidebar::snapshot(input.environments, now)?;
    let threads = snapshot["threads"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let environments = snapshot["environments"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let order: HashMap<&str, usize> = environments.iter().enumerate().map(|(i, e)| (text(e, "id"), i)).collect();
    let ready = |env: &str| environments.iter().any(|e| text(e, "id") == env && e["state"] == "ready");
    let host: HashMap<&str, String> = environments.iter().map(|e| (text(e, "id"), clean(text(e, "name"), TEXT_MAX))).collect();
    let raw: HashMap<(&str, &str), &Value> = input
        .environments
        .iter()
        .flat_map(|e| e["shell"]["threads"].as_array().into_iter().flatten().map(move |t| ((text(e, "id"), text(t, "id")), t)))
        .collect();
    let previous = if input.lifecycle == "cold" { None } else { previous(input.baseline) };
    let seen = |env: &str| previous.and_then(|p| p.get(env)).and_then(Value::as_object);

    // Ready environments start over from what they show; the others keep their entry.
    let mut baseline = Map::new();
    for e in environments {
        let id = text(e, "id");
        if ready(id) {
            baseline.insert(id.to_string(), json!({}));
        } else if let Some(old) = seen(id) {
            baseline.insert(id.to_string(), Value::Object(old.clone()));
        }
    }
    let mut rows = vec![];
    let mut alerts = vec![];
    let mut deadlines = vec![];
    for t in threads.iter().filter(|t| t["relationshipToParent"] != "subagent") {
        let (env, id) = (text(t, "environmentId"), text(t, "id"));
        let Some(raw) = raw.get(&(env, id)).copied() else { continue };
        let order = order.get(env).copied().unwrap_or(usize::MAX);
        let updated_at = t["updatedAt"].as_i64().unwrap_or(0);
        let result_at = t["latestRunCompletedAt"].as_i64().filter(|n| *n > 0).unwrap_or(updated_at);
        let live = ready(env);
        if let Some(real) = phase(raw) {
            let expires_at = expires_at(real, updated_at, result_at);
            if now < expires_at {
                let shown = if !live && !finished(real) { "stale" } else { real };
                rows.push(Row {
                    env,
                    id,
                    order,
                    phase: shown,
                    at: if finished(shown) { result_at } else { updated_at },
                    expires_at,
                    since: if shown == "stale" { None } else { since(shown, raw, t) },
                    title: clean(text(t, "title"), TEXT_MAX),
                    project: clean(text(t, "project"), TEXT_MAX),
                    host: host.get(env).cloned().unwrap_or_default(),
                    status: clean(&tr(status_source(t, shown), None), STATUS_MAX),
                });
            }
        }
        if !live {
            continue;
        }
        // A snooze that ends with a reason turns the thread pending (ot3's wake-up).
        let snoozed_until = epoch(&raw["snoozedUntil"]);
        if t["snoozed"] == true && !t["reason"].is_null() && snoozed_until > now {
            deadlines.push(snoozed_until);
        }
        if t["pending"] != true {
            continue;
        }
        let revision = text(t, "revision");
        baseline[env][id] = json!(revision);
        let Some(before) = seen(env) else { continue };
        if before.get(id).and_then(Value::as_str) == Some(revision) {
            continue;
        }
        let question = raw["pendingRuntimeRequest"]["kind"] == "user_input";
        let p = &input.preferences;
        let (attention, phase, allowed) = match text(t, "reason") {
            "request" if question => (true, "waiting_for_input", p.input),
            "request" => (true, "waiting_for_approval", p.approval),
            // A ready plan waits for the user's answer: T3 has no phase of its own for it.
            "plan" => (true, "waiting_for_input", p.plan),
            "review" => (false, "completed", p.completion),
            "failed" => (false, "failed", p.failure),
            _ => continue,
        };
        let fresh = attention || (result_at > 0 && now - result_at <= FRESHNESS_MS);
        if !allowed || !fresh || input.on_screen == Some((env, id)) {
            continue;
        }
        let status = clean(&tr(status_source(t, phase), None), STATUS_MAX);
        let mut alert = json!({
            "id":serde_json::to_string(&json!([env, id, phase, revision])).unwrap(),
            "group":if attention { "attention" } else { "finished" },"phase":phase,
            "title":clean(text(t, "title"), TEXT_MAX),"body":format!("{status}: {}", clean(text(t, "project"), TEXT_MAX)),
            "status":status,"route":{"environmentId":env,"threadId":id},
        });
        if !attention {
            alert["expiresAt"] = json!(result_at + FINISHED_TTL_MS);
        }
        alerts.push(Alert { id, order, attention, phase, at: if attention { updated_at } else { result_at }, json: alert });
    }

    rows.sort_by_key(|r| (priority(r.phase), Reverse(r.at), r.order, r.id));
    alerts.sort_by_key(|a| (!a.attention, priority(a.phase), Reverse(a.at), a.order, a.id));
    let card = card(&rows, &tr);
    let summaries = [true, false]
        .iter()
        .filter_map(|&attention| {
            let group = alerts.iter().filter(|a| a.attention == attention).collect::<Vec<_>>();
            (group.len() > 1).then(|| {
                let lines = group.iter().map(|a| a.json["title"].clone()).collect::<Vec<_>>();
                let body = lines.iter().map(|l| l.as_str().unwrap_or("")).collect::<Vec<_>>().join(", ");
                let title = tr(if attention { "{n} agents need attention" } else { "{n} agents finished" }, Some(group.len()));
                json!({"group":if attention { "attention" } else { "finished" },"title":title,"body":body,"lines":lines,"route":group[0].json["route"]})
            })
        })
        .collect::<Vec<_>>();
    deadlines.extend(rows.iter().map(|r| r.expires_at));
    deadlines.extend(alerts.iter().filter_map(|a| a.json["expiresAt"].as_i64()));
    Ok(json!({
        "counts":sidebar::counts(threads),
        "rows":rows.iter().map(Row::json).collect::<Vec<_>>(),
        "card":card,
        "alerts":alerts.into_iter().map(|a| a.json).collect::<Vec<_>>(),
        "summaries":summaries,
        "baseline":{"version":BASELINE_VERSION,"environments":baseline},
        "nextDeadline":deadlines.into_iter().filter(|d| *d > now).min(),
    }))
}

/// The ongoing card (T3 `makeAggregateState` and `androidActivityData`, worded
/// as T3 Mobile's `ActivityPresentation`), or null when no row is left.
fn card(rows: &[Row], tr: &dyn Fn(&str, Option<usize>) -> String) -> Value {
    let live = rows.iter().filter(|r| !finished(r.phase)).collect::<Vec<_>>();
    let mut results = rows.iter().filter(|r| finished(r.phase)).collect::<Vec<_>>();
    results.sort_by_key(|r| (Reverse(r.at), r.order, r.id));
    let mut shown = live.iter().take(CARD_ROWS).chain(results.iter()).take(CARD_ROWS).copied().collect::<Vec<_>>();
    shown.sort_by_key(|r| priority(r.phase));
    let Some(hero) = shown.first() else { return Value::Null };
    let active = live.len();
    let attention = shown.iter().filter(|r| priority(r.phase) == 0).count();
    let failed = shown.iter().filter(|r| r.phase == "failed").count();
    let title = if shown.len() == 1 {
        hero.title.clone()
    } else if attention == 1 {
        tr("1 needs you", None)
    } else if attention > 1 {
        tr("{n} need you", Some(attention))
    } else if active > 0 && failed > 0 {
        tr("{n} failed", Some(failed))
    } else if active > 0 {
        tr("{n} working", Some(active))
    } else if failed > 0 {
        tr("Finished, {n} failed", Some(failed))
    } else {
        tr("All finished", None)
    };
    let subtitle = if shown.len() == 1 {
        [hero.project.as_str(), hero.host.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ")
    } else {
        let one_project = shown.iter().all(|r| r.project == hero.project) && !hero.project.is_empty();
        let count = if active > 0 { tr("{n} active", Some(active)) } else { tr("{n} threads", Some(shown.iter().filter(|r| finished(r.phase)).count())) };
        one_project.then(|| hero.project.clone()).into_iter().chain([count]).collect::<Vec<_>>().join(" · ")
    };
    let mut card = json!({
        "active":active > 0,"phase":hero.phase,"title":title,"subtitle":subtitle,"status":hero.status,
        "expiresAt":shown.iter().map(|r| r.expires_at).max(),"route":hero.route(),
        "rows":shown.iter().map(|r| r.json()).collect::<Vec<_>>(),
    });
    // The chip follows the first live row: Android's chip has no failed, done or waiting kind.
    if let Some(first) = live.first() {
        let waiting = live.iter().filter(|r| priority(r.phase) == 0).count();
        card["chip"] = match first.phase {
            "waiting_for_approval" => json!({"kind":"approve","count":waiting}),
            "waiting_for_input" => json!({"kind":"answer","count":waiting}),
            "running" if active > 1 => json!({"kind":"live","count":active}),
            _ => json!({"kind":"working","count":active}),
        };
    }
    if let Some(since) = hero.since {
        card["since"] = json!(since);
    }
    card
}
