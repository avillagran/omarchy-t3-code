//! A thread as ot3 shows it: its message rows, the conversation timeline
//! (messages, merged activity blocks, changes, context boundaries, plans,
//! secret requests, subagent cards and provider errors, each with a `rev`
//! fingerprint), the live activity, the thread state and the cards above the
//! composer. Byte for byte ot3's `normalize_messages`, `conversation_timeline`,
//! `live_activity`, `context_window`, `thread_workflows` and the subagent
//! summaries, with the renderers as parameters instead of ot3's Qt Markdown.
use crate::util::{epoch, text, truth, valid_id};
use crate::{activity, approvals, changes, checkpoint_revert, code_blocks, compaction, documents, lifecycle, markdown, message_text, plan_workflow, prompt_recall, questions, queue, sidebar, tasks, tool_inspector, Result};
use serde_json::{json, Value};
use std::collections::HashSet;

pub mod minimap;

/// A host's rich-text renderer for one message: its display text, its code
/// blocks (`code_blocks::parse`) and whether math is on (ot3: Qt rich text).
pub type MessageRenderer = dyn Fn(&str, &[Value], bool) -> String;

/// How message and plan Markdown is turned into the host's rich text.
pub struct Render<'a> {
    /// T3's `mathRenderingEnabled` client setting.
    pub math: bool,
    /// `None` leaves `rendered` out of the message rows.
    pub message: Option<&'a MessageRenderer>,
    /// The renderer `plan_workflow::view` passes plan Markdown through.
    pub plan: &'a dyn Fn(&str) -> String,
    /// Add `markdown::blocks` to assistant messages and plans (and drop the
    /// plans' `rendered`), for hosts that draw the blocks themselves.
    pub blocks: bool,
}
impl Render<'_> {
    /// No rich text: renderer-neutral blocks for assistant messages and plans.
    pub fn neutral(math: bool) -> Render<'static> {
        Render { math, message: None, plan: &|markdown: &str| markdown.to_string(), blocks: true }
    }
}

/// The runs a checkpoint rollback undid: their rows leave the conversation.
pub fn rolled_back_runs(projection: &Value) -> HashSet<String> {
    projection["runs"].as_array().into_iter().flatten().filter(|r| r["status"] == "rolled_back").filter_map(|r| r["id"].as_str().map(str::to_owned)).collect()
}
/// The run in progress: the newest preparing/starting/running/waiting run, or the
/// thread's `activeRunId` when the projection has no runs at all.
pub fn active_run_id(p: &Value) -> &str {
    p["runs"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|r| matches!(text(r, "status"), "preparing" | "starting" | "running" | "waiting") && !crate::compaction::idle_run(p,text(r,"id")))
        .map(|r| text(r, "id"))
        .unwrap_or_else(|| if p["runs"].as_array().is_some_and(|r| !r.is_empty()) { "" } else { text(&p["thread"], "activeRunId") })
}

/// The message rows of a projection: queued, hidden and rolled-back messages
/// dropped; display text, search text, code blocks, tables, images, file
/// references, rewind and fork data per row.
pub fn messages(p: &Value, r: &Render) -> Result<Vec<Value>> {
    let rows = p["messages"].as_array().ok_or("malformed")?;
    let rolled = rolled_back_runs(p);
    let queue = queue::derive(p);
    let hidden = queue["hiddenMessageIds"].as_array().unwrap();
    let mut messages = vec![];
    for m in rows {
        if hidden.contains(&m["id"]) || rolled.contains(text(m, "runId")) {
            continue;
        }
        if !m["text"].is_string() || !m["role"].is_string() {
            return Err("malformed".into());
        }
        let (attribution, raw) = prompt_recall::attribution(m);
        let display = message_text::plain_qt(raw).unwrap_or_else(|| raw.to_string());
        let blocks = code_blocks::parse(&display);
        let search = markdown::search_text(&display);
        let images = markdown::images(&display);
        let mut row = json!({"id":text(m,"id"),"role":m["role"],
            "revertReference":if m["role"]=="user"{checkpoint_revert::reference(p,text(m,"id")).ok()}else{None},
            "canRewindMessage":checkpoint_revert::can_preview(p,m),
            "recallableText":if m["role"]=="user"&&attribution.is_none(){json!(prompt_recall::recall(raw))}else{Value::Null},
            "attribution":attribution,
            "senderThreadId":if attribution==Some("agent"){m["senderThreadId"].as_str().filter(|id|valid_id(id)).map(String::from)}else{None},
            "inputIntent":if m["role"]=="user"{checkpoint_revert::message_item(p,m).and_then(|i|i["inputIntent"].as_str()).map(String::from)}else{None},
            "forkRunId":if m["role"]=="assistant"&&m["streaming"]!=true&&m["runId"].is_string(){m["runId"].clone()}else{Value::Null},
            "streaming":m["streaming"]==true,"text":display,"searchText":if m["role"]=="user"{display.clone()}else{search},
            "attachments":m["attachments"].as_array().cloned().unwrap_or_default(),
            "markdownImages":if m["role"]=="assistant"{images}else{vec![]},"fileReferences":markdown::file_references(&display),
            "tables":if m["role"]=="assistant"&&display.contains('|'){json!(markdown::tables(&display))}else{json!([])},
            "createdAt":epoch(&m["createdAt"])});
        if let Some(render) = r.message {
            row["rendered"] = json!(render(&display, &blocks, r.math));
        }
        if r.blocks && m["role"] == "assistant" {
            row["blocks"] = json!(markdown::blocks(&display, r.math));
        }
        row["codeBlocks"] = json!(blocks);
        messages.push(row);
    }
    // One fork action per run: on its last response.
    let mut seen_runs = HashSet::new();
    for m in messages.iter_mut().rev() {
        if let Some(run) = m["forkRunId"].as_str().map(str::to_string) {
            if !seen_runs.insert(run) {
                m["forkRunId"] = Value::Null;
            }
        }
    }
    Ok(messages)
}

/// ot3 `normalize_messages`: the whole thread view of projection `p`; `history`
/// carries the envelope's `historyCursor`, `hasMoreHistory` and `payloadBudgetExceeded`.
pub fn view(p: &Value, history: &Value, r: &Render) -> Result<Value> {
    let messages = messages(p, r)?;
    let queue = queue::derive(p);
    let active = active_run_id(p);
    let status = p["runs"].as_array().and_then(|r| r.last()).map(|r| text(r, "status")).unwrap_or("idle");
    let started = p["runs"].as_array().into_iter().flatten().find(|r| text(r, "id") == active).map(|r| epoch(&r["startedAt"]).max(epoch(&r["createdAt"]))).unwrap_or(0);
    let mut sorting = p["thread"].as_object().cloned().map(Value::Object).unwrap_or_else(|| json!({}));
    sorting["latestRun"] = p["runs"].as_array().and_then(|r| r.last()).cloned().unwrap_or(Value::Null);
    if sorting["latestRun"].is_object() && sorting["latestRun"].get("requestedAt").is_none() {
        sorting["latestRun"]["requestedAt"] = sorting["latestRun"]["createdAt"].clone();
    }
    let mut thread_state = json!({"working":!active.is_empty(),"startedAt":started,"activeRunId":if active.is_empty(){Value::Null}else{json!(active)},"status":status,"providerInstanceId":p["thread"]["providerInstanceId"],"interactionMode":p["thread"]["interactionMode"],"latestRunSettled":plan_workflow::latest_run_settled(p)});
    for (key, value) in sidebar::sort_timestamps(&sorting).as_object().unwrap() {
        if key != "lastSentAt" && value.as_i64().unwrap_or(0) > 0 {
            thread_state[key] = value.clone();
        }
    }
    for (key, value) in thread_workflows(p).as_object().unwrap() {
        thread_state[key] = value.clone();
    }
    thread_state["contextWindow"] = context_window(p);
    thread_state["guestShare"] = p["thread"]["guestShare"].clone();
    // This thread's subagents, newest first: the composer bar shows the live
    // ones, the conversation cards and the Agents panel all of them.
    thread_state["subagents"] = json!(p["subagents"].as_array().into_iter().flatten().rev().take(48).map(subagent_summary).collect::<Vec<_>>());
    let mut view = json!({"threadState":thread_state,"approvals":approvals::pending(p),"questions":questions::pending(p),"tasks":tasks::derive(p),"queue":queue,
        "planView":plan_workflow::view(p,r.plan),"timeline":conversation(p,&messages,r.plan),"messages":messages,"activity":live_activity(p),
        "historyCursor":history["historyCursor"],"hasMoreHistory":truth(&history["hasMoreHistory"]),"payloadBudgetExceeded":truth(&history["payloadBudgetExceeded"])});
    if r.blocks {
        for plan in view["planView"]["plans"].as_array_mut().into_iter().flatten() {
            plan_blocks(plan, r.math);
        }
        plan_blocks(&mut view["planView"]["ready"], r.math);
        for row in view["timeline"].as_array_mut().into_iter().flatten().filter(|row| row["role"] == "plan") {
            plan_blocks(&mut row["plan"], r.math);
        }
    }
    Ok(view)
}
/// A plan card's blocks in place of its `rendered` text.
pub(crate) fn plan_blocks(plan: &mut Value, math: bool) {
    if let Some(fields) = plan.as_object_mut() {
        fields.remove("rendered");
        let blocks = markdown::blocks(fields.get("displayMarkdown").and_then(Value::as_str).unwrap_or(""), math);
        fields.insert("blocks".into(), json!(blocks));
    }
}

/// One line of a subagent's free text (progress or result), whitespace collapsed.
fn subagent_line(value: &Value, max: usize) -> Value {
    let raw = value.as_str().unwrap_or("");
    if raw.trim_start().starts_with("Child task ended with status") {
        return Value::Null;
    }
    let line = raw.split_whitespace().collect::<Vec<_>>().join(" ").replace("**", "").replace('`', "");
    if line.is_empty() {
        return Value::Null;
    }
    let mut out = line.chars().take(max).collect::<String>();
    if line.chars().count() > max {
        out.push('…');
    }
    json!(out)
}
/// T3 formatSubagentDisplayTitle: no "Subagent:" prefix; an agent path shows its last name.
pub fn subagent_title(s: &Value) -> String {
    let raw = s["workflow"]["name"].as_str().or(s["title"].as_str()).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| text(s, "prompt"));
    let mut title = raw.trim();
    if let Some(rest) = title.strip_prefix("Subagent:") {
        title = rest.trim();
    }
    if title.starts_with('/') && !title.contains(char::is_whitespace) {
        title = title.rsplit('/').find(|p| !p.is_empty()).unwrap_or(title);
    }
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        "Subagent".into()
    } else {
        title.chars().take(120).collect()
    }
}
/// What the panel shows of one projected subagent (T3 OrchestrationV2Subagent).
pub fn subagent_summary(s: &Value) -> Value {
    let w = &s["workflow"];
    let agents = w["agents"].as_array().map(|a| a.iter().take(32).map(|m| json!({"index":m["index"],"label":m["label"].as_str().map(|l|l.chars().take(120).collect::<String>()),"state":m["state"],"childThreadId":m["childThreadId"],"phaseTitle":m["phaseTitle"],"model":m["model"],"startedAt":m["startedAt"],"durationMs":m["durationMs"]})).collect::<Vec<_>>());
    json!({"id":s["id"],"runId":s["runId"],"title":subagent_title(s),"prompt":subagent_line(&s["prompt"],200),"status":s["status"],"childThreadId":s["childThreadId"],"origin":s["origin"],"model":s["model"],"providerInstanceId":s["providerInstanceId"],"progress":subagent_line(&s["progress"],240),"result":subagent_line(&s["result"],600),"startedAt":epoch(&s["startedAt"]),"completedAt":epoch(&s["completedAt"]),"updatedAt":epoch(&s["updatedAt"]),"workflow":w.is_object(),"scriptPath":w["runHandles"]["scriptPath"],"phases":w["phases"].as_array().map(|a|a.len()),"agents":w["agents"].as_array().map(|a|a.len()),"workflowAgents":agents})
}
/// Port of T3 contextWindow.ts: the newest provider turn usage, else the active provider
/// thread's context usage, else the last compaction's token count.
pub fn context_window(p: &Value) -> Value {
    let num = |v: &Value| v.as_f64().filter(|n| n.is_finite());
    let build = |used: f64, max: Option<f64>, u: &Value| {
        let pct = max.filter(|m| *m > 0.0).map(|m| (used / m * 100.0).min(100.0));
        json!({"usedTokens":used.max(0.0),"maxTokens":max,"usedPercentage":pct,"remainingTokens":max.map(|m|(m-used).round().max(0.0)),"inputTokens":num(&u["inputTokens"]),"cachedInputTokens":num(&u["cachedInputTokens"]),"outputTokens":num(&u["outputTokens"]),"reasoningOutputTokens":num(&u["reasoningOutputTokens"]),"totalProcessedTokens":num(&u["totalProcessedTokens"]),"compactsAutomatically":u.get("compactsAutomatically").cloned().unwrap_or(json!(true)),"autoCompactThreshold":num(&u["autoCompactThreshold"]),"cost":num(&u["cost"])})
    };
    let live = p["providerTurns"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| &t["tokenUsage"])
        .filter(|u| u.is_object())
        .fold(None::<&Value>, |best, u| match best {
            Some(b) if compaction::timestamp(&u["updatedAt"]) < compaction::timestamp(&b["updatedAt"]) => Some(b),
            _ => Some(u),
        });
    if let Some(u) = live {
        if let Some(used) = num(&u["usedTokens"]) {
            let mut out = build(used, num(&u["maxTokens"]), u);
            out["totalProcessedTokens"] = Value::Null;
            out["autoCompactThreshold"] = Value::Null;
            out["cost"] = Value::Null;
            out["compactsAutomatically"] = json!(true);
            return out;
        }
    }
    let active = &p["thread"]["activeProviderThreadId"];
    if let Some(u) = p["providerThreads"].as_array().into_iter().flatten().find(|t| !active.is_null() && &t["id"] == active).map(|t| &t["contextUsage"]).filter(|u| u.is_object()) {
        if let Some(used) = num(&u["usedTokens"]) {
            return build(used, num(&u["maxTokens"]), u);
        }
    }
    if let Some(c) = p["turnItems"].as_array().into_iter().flatten().rev().find(|i| i["type"] == "compaction" && num(&i["afterTokenCount"]).is_some_and(|n| n >= 0.0)) {
        let mut out = build(num(&c["afterTokenCount"]).unwrap(), None, &Value::Null);
        out["totalProcessedTokens"] = json!(num(&c["beforeTokenCount"]));
        out["compactsAutomatically"] = json!(true);
        return out;
    }
    Value::Null
}
/// Ports of T3 threadWorkflows/threadRelationships: where a fork merges back and
/// which finished run it would carry.
pub fn thread_workflows(p: &Value) -> Value {
    let thread = &p["thread"];
    let target = if thread["lineage"]["relationshipToParent"] != "fork" {
        Value::Null
    } else if thread["forkedFrom"]["type"] == "run" {
        thread["forkedFrom"]["threadId"].clone()
    } else {
        thread["lineage"]["parentThreadId"].clone()
    };
    let runs = p["runs"].as_array().cloned().unwrap_or_default();
    let ordinal = |r: &Value| r["ordinal"].as_i64().unwrap_or(-1);
    let latest = runs.iter().filter(|r| matches!(text(r, "status"), "waiting" | "completed")).max_by_key(|r| ordinal(r));
    let run = latest.filter(|l| !runs.iter().any(|r| ordinal(r) > ordinal(l) && matches!(text(r, "status"), "preparing" | "starting" | "running"))).map(|r| r["id"].clone()).unwrap_or(Value::Null);
    json!({"mergeBackTargetThreadId":target,"mergeBackRunId":run})
}
/// A turn item's compact activity plus its tool-inspector reference. Never
/// exposes tool inputs, outputs or reasoning bodies.
fn inspectable_activity(row: &Value) -> Option<Value> {
    let mut activity = activity::describe(row)?;
    if let Ok(reference) = tool_inspector::compact_reference(row) {
        activity["inspector"] = reference;
    }
    Some(activity)
}
/// The active run's in-flight activity, newest first (at most 3): the working pill.
pub fn live_activity(projection: &Value) -> Value {
    let run = active_run_id(projection);
    json!(projection["turnItems"].as_array().into_iter().flatten().filter_map(inspectable_activity).filter(|a| !run.is_empty() && text(a, "runId") == run && a["active"] == true).rev().take(3).collect::<Vec<_>>())
}

/// ot3 `conversation_timeline`: `messages` (from `messages`) and the projection's
/// turn items from the first message on, in time order; consecutive activity of
/// one run merges into a block (count, up to 600 entries) and a run's subagent
/// cards into one group. Each row gets `rev`, a fingerprint of its content.
pub fn conversation(projection: &Value, messages: &[Value], plan: &dyn Fn(&str) -> String) -> Vec<Value> {
    let rolled = rolled_back_runs(projection);
    let mut rows = messages.to_vec();
    let first = messages.first().map(|m| epoch(&m["createdAt"])).unwrap_or(0);
    let plan_view = plan_workflow::view(projection, plan);
    let items = projection["turnItems"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    for row in &items[items.len().saturating_sub(600)..] {
        let native = activity::item(row);
        if rolled.contains(text(native, "runId")) {
            continue;
        }
        if native["type"] == "proposed_plan" {
            if let Some(plan) = plan_view["plans"].as_array().into_iter().flatten().find(|p| p["id"] == native["planId"]) {
                let time = epoch(&native["startedAt"]).max(epoch(&native["updatedAt"]));
                if time >= first {
                    rows.push(json!({"id":format!("plan/{}",text(native,"id")),"role":"plan","createdAt":time,"plan":plan,"text":""}));
                }
                continue;
            }
        }
        if native["type"] == "secret_request" {
            // An agent's request_secret: answered here, the value goes to the server only.
            let time = epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
            let local = native["threadId"].is_null() || native["threadId"] == projection["thread"]["id"];
            if time >= first {
                rows.push(json!({"id":format!("secret/{}",text(native,"id")),"role":"secret","createdAt":time,"text":"","secret":{"turnItemId":native["id"],"threadId":native["threadId"],"label":native["label"],"reason":native["reason"],"placeholder":native["placeholder"],"status":native["secretStatus"],"local":local}}))
            }
            continue;
        }
        if let Some(mut a) = documents::mcp_app(native) {
            // An MCP app is drawn like an HTML render; its requests go back through mcpApps.*.
            if a["mcpApp"]["threadId"].is_null() {
                a["mcpApp"]["threadId"] = projection["thread"]["id"].clone();
            }
            let time = epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
            if time >= first {
                rows.push(json!({"id":format!("app/{}",text(native,"id")),"role":"assistant","createdAt":time,"text":"","attachments":[a]}))
            }
        }
        if let Some(a) = documents::html_render(native) {
            let time = epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
            if time >= first {
                rows.push(json!({"id":format!("html/{}",text(native,"id")),"role":"assistant","createdAt":time,"text":"","attachments":[a]}))
            }
        }
        if native["type"] == "error" {
            // A provider failure: show its own words, not only the run's "failed" status.
            let time = epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
            let f = &native["failure"];
            let message = f["message"].as_str().map(str::trim).filter(|m| !m.is_empty()).map(|m| m.chars().take(2000).collect::<String>());
            // Nested errors belong to the item that owns them (T3 shows root failures only).
            if time >= first && message.is_some() && native["parentItemId"].is_null() {
                rows.push(json!({"id":format!("error/{}",text(native,"id")),"role":"error","createdAt":time,"text":"","error":{"message":message,"title":native["title"],"status":native["status"],"class":f["class"],"code":f["code"],"retryable":f["retryable"],"retry":native["retry"],"resetAt":f["resetAt"],"runId":native["runId"],"providerInstanceId":projection["runs"].as_array().into_iter().flatten().find(|r|r["id"]==native["runId"]).map(|r|r["providerInstanceId"].clone()).unwrap_or(Value::Null)}}))
            }
            continue;
        }
        if native["type"] == "subagent" {
            // Delegated work gets its own card, not a line in the run's tool log.
            let time = epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
            if time >= first {
                let live = projection["subagents"].as_array().into_iter().flatten().find(|s| !native["subagentId"].is_null() && s["id"] == native["subagentId"]).cloned().unwrap_or_else(|| json!({"id":native["subagentId"],"title":native["title"],"prompt":native["prompt"],"status":native["status"],"childThreadId":native["childThreadId"],"origin":native["origin"],"progress":native["progress"],"result":native["result"],"startedAt":native["startedAt"],"completedAt":native["completedAt"],"providerInstanceId":native["providerInstanceId"]}));
                let mut card = subagent_summary(&live);
                card["itemId"] = native["id"].clone();
                if let Ok(reference) = tool_inspector::compact_reference(row) {
                    card["inspector"] = reference;
                }
                rows.push(json!({"id":format!("agents/{}",text(native,"id")),"role":"subagents","createdAt":time,"text":"","runId":native["runId"],"subagents":[card]}));
            }
            continue;
        }
        if let Some(boundary) = lifecycle::describe(row, projection["runs"].as_array().map(Vec::as_slice).unwrap_or(&[])) {
            let time = epoch(&native["startedAt"]).max(epoch(&row["createdAt"]));
            if time >= first {
                rows.push(json!({"id":format!("context/{}",text(native,"id")),"role":"context","createdAt":time,"context":boundary,"text":""}))
            }
            continue;
        }
        let Some(mut activity) = inspectable_activity(row) else { continue };
        let item = activity::item(row);
        let time = epoch(&item["startedAt"]).max(epoch(&row["createdAt"]));
        if time < first {
            continue;
        }
        activity["active"] = json!(activity["active"] == true && text(&activity, "runId") == active_run_id(projection) && !active_run_id(projection).is_empty());
        rows.push(json!({"id":format!("work/{}",text(item,"id")),"role":"activity","createdAt":time,"activity":activity,"text":""}));
    }
    for c in changes::summaries(projection) {
        if rolled.contains(text(&c, "runId")) {
            continue;
        }
        let time = epoch(&c["capturedAt"]);
        if time >= first {
            rows.push(json!({"id":format!("changes/{}",text(&c,"id")),"role":"changes","createdAt":time,"changes":c,"text":""}))
        }
    }
    rows.sort_by_key(|r| epoch(&r["createdAt"]));
    let mut grouped: Vec<Value> = vec![];
    for row in rows {
        if let Some(last) = grouped.last_mut() {
            if row["role"] == "subagents" && last["role"] == "subagents" && row["runId"] == last["runId"] {
                let mut all = last["subagents"].as_array().cloned().unwrap_or_default();
                for s in row["subagents"].as_array().into_iter().flatten() {
                    if all.len() < 64 && !all.iter().any(|a| a["id"] == s["id"] && !s["id"].is_null()) {
                        all.push(s.clone())
                    }
                }
                last["subagents"] = json!(all);
                continue;
            }
            if row["role"] == "activity" && last["role"] == "activity" && row["activity"]["runId"] == last["activity"]["runId"] {
                let count = last["activity"]["count"].as_u64().unwrap_or(1) + 1;
                let mut entries = last["activity"]["entries"].as_array().cloned().unwrap_or_else(|| vec![last["activity"].clone()]);
                if entries.len() < 600 {
                    entries.push(row["activity"].clone())
                }
                let id = last["id"].clone();
                let active = if last["activity"]["active"] == true && row["activity"]["active"] != true { Some(last["activity"].clone()) } else { None };
                *last = row;
                if let Some(a) = active {
                    last["activity"] = a;
                }
                last["id"] = id;
                last["activity"]["count"] = json!(count);
                last["activity"]["entries"] = json!(entries);
                continue;
            }
        }
        grouped.push(row)
    }
    crate::compaction::timeline(projection,&mut grouped);
    // A content fingerprint per row lets a view skip unchanged rows on each
    // streamed update without comparing whole rows.
    for row in grouped.iter_mut() {
        row["rev"] = json!(revision(row))
    }
    grouped
}
/// 16 hex digits hashing a row's JSON (std's SipHash with fixed keys): equal
/// rows have equal revisions within one build of the library.
pub fn revision(row: &Value) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    row.to_string().hash(&mut h);
    format!("{:016x}", h.finish())
}
