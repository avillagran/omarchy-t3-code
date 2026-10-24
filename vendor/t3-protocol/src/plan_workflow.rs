//! Pure native-contract projection and payload builders. No transport or side effects.
//! The caller must scope the projection to the selected environment and thread.
use serde_json::{json, Value};

pub type Result<T> = std::result::Result<T, &'static str>;
const MAX_MARKDOWN: usize = 120_000; // matches the current ot3 outgoing-message limit
const MAX_PLANS: usize = 256;

fn id(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| {
        !s.is_empty()
            && s.len() <= 2048
            && s.as_bytes()[0].is_ascii_alphanumeric()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_:%.-".contains(&b))
    })
}

/// Native ChatView.logic resolves unsupported/disabled modes to default. This is a
/// client preference, NOT a server.getSettings / server.updateSettings field.
pub fn mode(plan_mode_enabled: bool, provider: Option<&Value>, requested: &str) -> Value {
    let enabled = plan_mode_enabled
        && provider.is_some_and(|p| p.is_object() && p["showInteractionModeToggle"] != false);
    json!({"enabled": enabled, "interactionMode": if enabled && requested == "plan" { "plan" } else { "default" }})
}

pub fn title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let trimmed = line.trim_start_matches(' ');
        if line.len() - trimmed.len() > 3 {
            return None;
        }
        let depth = trimmed.bytes().take_while(|b| *b == b'#').count();
        if !(1..=6).contains(&depth) || !trimmed[depth..].starts_with(char::is_whitespace) {
            return None;
        }
        let value = trimmed[depth..].trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// Native proposedPlanText.ts removes the heading and optional `Summary` heading
/// for display only. `markdown` stays unmodified for the provider and exports.
pub fn display_markdown(markdown: &str) -> String {
    let source = markdown.trim_end();
    let mut lines: Vec<&str> = source.split('\n').collect();
    if lines.first().is_some_and(|line| {
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim_start_matches(' ');
        line.len() - trimmed.len() <= 3
            && (1..=6).contains(&trimmed.bytes().take_while(|b| *b == b'#').count())
            && trimmed[trimmed.bytes().take_while(|b| *b == b'#').count()..]
                .starts_with(char::is_whitespace)
    }) {
        lines.remove(0);
    }
    while lines.first().is_some_and(|s| s.trim().is_empty()) {
        lines.remove(0);
    }
    if lines.first().is_some_and(|line| {
        let l = line.trim_start_matches(' ');
        let depth = l.bytes().take_while(|b| *b == b'#').count();
        line.len() - l.len() <= 3
            && (1..=6).contains(&depth)
            && l[depth..].trim().eq_ignore_ascii_case("summary")
    }) {
        lines.remove(0);
        while lines.first().is_some_and(|s| s.trim().is_empty()) {
            lines.remove(0);
        }
    }
    lines.join("\n").replace("\r\n", "\n")
}

/// Unlike a deltas-based preview, only completed native plan artifacts are rendered.
/// Return all bounded proposed plans for their timeline items and one eligible ready
/// plan. Never infer a proposed plan from an assistant message or todo_list.
pub fn project(
    projection: &Value,
    thread_id: &str,
    latest_run_settled: bool,
    interaction_mode: &str,
    pending_user_inputs: usize,
    has_attachments: bool,
    render: impl Fn(&str) -> String,
) -> Result<Value> {
    if id(&json!(thread_id)).is_none() || projection["thread"]["id"] != thread_id {
        return Err("invalid_plan_scope");
    }
    let plans = projection["plans"]
        .as_array()
        .ok_or("invalid_plan_projection")?;
    if plans.len() > MAX_PLANS {
        return Err("plan_limit");
    }
    let mut items = Vec::new();
    let mut display_bytes = 0usize;
    for plan in plans {
        if projection["runs"].as_array().into_iter().flatten().any(|run|run["id"]==plan["runId"]&&run["status"]=="rolled_back"){continue}
        if plan["kind"] != "proposed_plan" {
            continue;
        }
        if plan["threadId"] != thread_id {
            return Err("invalid_plan_scope");
        }
        let plan_id = id(&plan["id"]).ok_or("invalid_plan_id")?;
        let status = plan["status"]
            .as_str()
            .filter(|s| ["draft", "active", "completed", "superseded"].contains(s))
            .ok_or("invalid_plan_status")?;
        let markdown = plan["markdown"].as_str().ok_or("invalid_plan_markdown")?;
        if markdown.len() > MAX_MARKDOWN {
            return Err("plan_limit");
        }
        let run_id = match &plan["runId"] {
            Value::Null => None,
            v => Some(id(v).ok_or("invalid_plan_run")?),
        };
        let display = display_markdown(markdown);
        let stamp = projection["turnItems"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find(|row| {
                let item = row.get("item").unwrap_or(row);
                (item["type"] == "proposed_plan" || item["type"] == "todo_list")
                    && item["planId"] == plan_id
            })
            .and_then(|row| {
                row.get("item")
                    .and_then(|i| i["updatedAt"].as_str())
                    .or_else(|| row["updatedAt"].as_str())
            })
            .or_else(|| projection["updatedAt"].as_str())
            .unwrap_or("");
        let rendered = render(&display);
        display_bytes += markdown.len() + display.len() + rendered.len();
        if display_bytes > 2 * 1024 * 1024 {
            return Err("plan_limit");
        }
        items.push(json!({"id":plan_id,"threadId":thread_id,"runId":run_id,"status":status,"markdown":markdown,
            "title":title(markdown).unwrap_or_else(||"Proposed plan".into()),"displayMarkdown":display,
            "rendered":rendered,"updatedAt":stamp}));
    }
    let latest_run = presented_latest_run(projection).and_then(|run| run["id"].as_str());
    let candidates: Vec<&Value> = items
        .iter()
        .filter(|p| latest_run.is_some_and(|id| p["runId"] == id))
        .collect();
    let mut candidates = if candidates.is_empty() {
        items.iter().collect::<Vec<_>>()
    } else {
        candidates
    };
    candidates.sort_by(|a, b| {
        a["updatedAt"]
            .as_str()
            .unwrap_or("")
            .cmp(b["updatedAt"].as_str().unwrap_or(""))
            .then_with(|| {
                a["id"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["id"].as_str().unwrap_or(""))
            })
    });
    let latest = if latest_run_settled {
        candidates.last().copied()
    } else {
        None
    };
    let ready = if pending_user_inputs == 0
        && interaction_mode == "plan"
        && latest_run_settled
        && !has_attachments
    {
        latest
            .filter(|p| {
                p["status"] == "active" && !p["markdown"].as_str().unwrap_or("").trim().is_empty()
            })
            .cloned()
    } else {
        None
    };
    Ok(json!({"plans":items,"ready":ready}))
}

/// The explicit ready-plan action differs from a nonempty follow-up draft:
/// feedback stays in plan mode, while an empty draft approves implementation.
pub fn follow_up(plan: &Value, thread_id: &str, draft: &str) -> Result<Value> {
    if plan["threadId"] != thread_id
        || id(&json!(thread_id)).is_none()
        || id(&plan["id"]).is_none()
        || plan["status"] != "active"
    {
        return Err("invalid_plan_scope");
    }
    let markdown = plan["markdown"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= MAX_MARKDOWN)
        .ok_or("invalid_plan_markdown")?;
    let draft = draft.trim();
    if draft.len() > MAX_MARKDOWN {
        return Err("plan_limit");
    }
    if !draft.is_empty() {
        return Ok(json!({"text":draft,"interactionMode":"plan"}));
    }
    let prompt = format!("PLEASE IMPLEMENT THIS PLAN:\n{}", markdown.trim());
    if prompt.chars().count() > MAX_MARKDOWN {
        return Err("plan_limit");
    }
    Ok(
        json!({"text":prompt,"interactionMode":"default","sourcePlanRef":{"threadId":thread_id,"planId":plan["id"]}}),
    )
}

/// Attach ONLY to message.dispatch, not thread.launch / thread.create.
/// The parent sets IDs, dispatchMode, model, runtime, credentials and outbox.
pub fn attach_source_ref(command: &mut Value, follow_up: &Value) -> Result<()> {
    if command["type"] != "message.dispatch" {
        return Err("invalid_plan_command");
    }
    let Some(reference) = follow_up.get("sourcePlanRef") else {
        return Ok(());
    };
    // A new-thread implementation deliberately targets a different thread;
    // the sourcePlanRef still names the original plan's owning thread.
    if id(&command["threadId"]).is_none() {
        return Err("invalid_plan_scope");
    }
    if id(&reference["threadId"]).is_none() || id(&reference["planId"]).is_none() {
        return Err("invalid_plan_scope");
    }
    command["sourcePlanRef"] =
        json!({"threadId":reference["threadId"],"planId":reference["planId"]});
    Ok(())
}

pub fn implementation_title(markdown: &str) -> String {
    let result =
        title(markdown).map_or_else(|| "Implement plan".to_owned(), |s| format!("Implement {s}"));
    result.chars().take(255).collect()
}

/// Native deriveLatestThreadRun: held queued runs do not own the outcome;
/// a usage-limited executed run stays visible ahead of later queued submissions.
fn presented_latest_run(projection: &Value) -> Option<&Value> {
    let runs = projection["runs"].as_array()?;
    let ordinal = |run: &Value| run["ordinal"].as_u64().unwrap_or(0);
    let latest = runs
        .iter()
        .filter(|run| !(run["status"] == "queued" && run["queueHeld"] == true))
        .max_by_key(|run| ordinal(run));
    let ended = |run: &Value| {
        run["completedAt"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.timestamp_millis())
            .unwrap_or(i64::MAX)
    };
    let executed = runs
        .iter()
        .filter(|run| {
            run["status"] != "queued"
                && !(run["status"] == "cancelled" && run["startedAt"].is_null())
        })
        .max_by_key(|run| (ended(run), ordinal(run)));
    if let Some(run) = executed.filter(|run| run["status"] == "failed") {
        let failure = projection["turnItems"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|entry| entry.get("item").unwrap_or(entry))
            .filter(|item| {
                item["type"] == "error"
                    && item["status"] == "failed"
                    && item["runId"] == run["id"]
                    && item["nodeId"] == run["rootNodeId"]
            })
            .max_by(|a, b| {
                a["updatedAt"]
                    .as_str()
                    .cmp(&b["updatedAt"].as_str())
                    .then_with(|| ordinal(a).cmp(&ordinal(b)))
                    .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
            })
            .map(|item| &item["failure"]);
        let session_error = projection["providerSessions"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find(|session| {
                session["providerInstanceId"] == projection["thread"]["providerInstanceId"]
            })
            .and_then(|session| session["lastError"].as_str());
        if failure.is_some_and(|failure| {
            failure["class"] == "usage_limit"
                && session_error.is_none_or(|error| failure["message"] == error)
        }) && runs
            .iter()
            .any(|candidate| ordinal(candidate) > ordinal(run))
        {
            return Some(run);
        }
    }
    latest
}

/// Native runtime.activeRunId comes only from preparing/starting/running runs.
/// A terminal presented run therefore cannot still be that active identity.
pub fn latest_run_settled(projection: &Value) -> bool {
    presented_latest_run(projection).is_some_and(|run| {
        run["id"].is_string()
            && matches!(
                run["status"].as_str(),
                Some("completed" | "interrupted" | "failed" | "cancelled" | "rolled_back")
            )
    })
}
/// Candidate readiness is native run/question state. Client-mode and attachment
/// gates are reapplied by the composer, which alone owns those local values.
pub fn view(projection: &Value, render: impl Fn(&str) -> String) -> Value {
    if projection.get("plans").is_none() {
        return json!({"plans":[],"ready":null});
    }
    match project(
        projection,
        projection["thread"]["id"].as_str().unwrap_or(""),
        latest_run_settled(projection),
        "plan",
        pending_user_inputs(projection),
        false,
        render,
    ) {
        Ok(view) => view,
        Err(error) => json!({"plans":[],"ready":null,"error":error}),
    }
}
fn pending_user_inputs(projection: &Value) -> usize {
    crate::questions::pending(projection)
        .as_array()
        .map(Vec::len)
        .unwrap_or(0)
}
