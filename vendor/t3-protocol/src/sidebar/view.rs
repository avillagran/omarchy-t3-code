//! The sidebar over several environments' shells, as ot3 shows it: rows per
//! environment (ot3 `normalize` and the enrichment of its `snapshot`), then
//! ot3's `SidebarModel.js` and `ProjectModel.js` over all of them: the row
//! detail and shelf bucket, filters, search, project groups, the active,
//! working, snoozed and settled shelves with sub-threads nested under their
//! parents, and the counts. Pure: the host passes `now` and its view state.
use crate::util::{epoch, text, truth, valid_id};
use crate::{sidebar, Result};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

/// ot3 `is_settled`: an explicit override wins over `settledAt`.
pub fn is_settled(t: &Value) -> bool {
    t["settledOverride"] == "settled" || t["settledOverride"] == true || (t["settledOverride"] != "active" && truth(&t["settledAt"]))
}

/// One environment's shell threads as sidebar rows (ot3 `normalize`), newest first.
/// Deleted and archived threads are dropped; a malformed shell is `malformed`.
pub fn rows(d: &Value, env: &str, now: i64) -> Result<Vec<Value>> {
    let rows = d["threads"].as_array().ok_or("malformed")?;
    let projects: HashMap<&str, &str> = d["projects"].as_array().into_iter().flatten().map(|p| (text(p, "id"), text(p, "title"))).collect();
    // A "waiting" run has ended its provider turn and only drains effects: still working in T3.
    let active = |s: &str| ["preparing", "queued", "starting", "running", "waiting"].contains(&s);
    let mut result = vec![];
    let mut ids = HashSet::new();
    for t in rows {
        let id = text(t, "id");
        if !valid_id(id) || !ids.insert(id) || !t["title"].is_string() {
            return Err("malformed".into());
        }
        if truth(&t["deletedAt"]) || truth(&t["archivedAt"]) {
            continue;
        }
        let status = t.get("status").unwrap_or(&Value::Null).as_str().unwrap_or("unknown");
        if !t["status"].is_null() && !t["status"].is_string() {
            return Err("malformed".into());
        }
        // T3 shellRuntime: a run waiting only on t3_thread_wait/delegate_task(wait)
        // reads as Waiting; background work keeps a thread waiting, not working,
        // and a command left running (a dev server) holds nothing.
        let run_active = active(status) || active(text(t, "activityRunStatus"));
        let blocked_on_wait = t["blockedOnThreadWait"] == true;
        let working = run_active && !blocked_on_wait;
        let background = status != "failed" && t["pendingBackgroundTasks"].as_array().is_some_and(|tasks| tasks.iter().any(|task| task["awaited"] == true || matches!(task["kind"].as_str(),Some("subagent"|"background_task")|None)));
        // An async question (blocking:false) does not stop a working run.
        let request = !t["pendingRuntimeRequest"].is_null() && !(t["pendingRuntimeRequest"]["blocking"] == false && run_active);
        let mut reason = if request {
            "request"
        } else if truth(&t["hasActionableProposedPlan"]) {
            "plan"
        } else if status == "failed" {
            "failed"
        } else {
            ""
        };
        let completed = epoch(&t["latestRunCompletedAt"]);
        if reason.is_empty()
            && !working
            && status == "completed"
            && t["latestRunStopRequested"] != true
            // T3 hasUnseenCompletion: on shells that track visits, a thread never visited is not "Done".
            && (t.get("lastVisitedAt").is_none() || epoch(&t["lastVisitedAt"]) > 0)
            && completed > epoch(&t["lastVisitedAt"])
        {
            reason = "review";
        }
        if is_settled(t) {
            reason = "";
        }
        let snoozed = epoch(&t["snoozedUntil"]) > now;
        let req = &t["pendingRuntimeRequest"];
        let request_id = if truth(req) { req["id"].as_str().filter(|s| !s.is_empty()).or_else(|| req["requestId"].as_str()).unwrap_or("pending") } else { "" };
        let stamp = if completed != 0 {
            completed.to_string()
        } else if !text(t, "latestRunId").is_empty() {
            String::new()
        } else {
            t["updatedAt"].as_str().map(str::to_string).unwrap_or_else(|| t["updatedAt"].to_string())
        };
        let revision = format!("{:x}", Sha256::digest([reason, text(t, "latestRunId"), request_id, &stamp].join("\0").as_bytes()));
        let project = text(t, "projectId");
        let mut row = json!({"id":id,"environmentId":env,"key":format!("{env}/{id}"),"title":t["title"],"project":projects.get(project).copied().unwrap_or(project),"status":status,"working":working,"pending":!reason.is_empty()&&!snoozed,"reason":if reason.is_empty(){Value::Null}else{json!(reason)},"snoozed":snoozed,"updatedAt":epoch(&t["updatedAt"]),"lastVisitedAt":epoch(&t["lastVisitedAt"]),"usageLimitResetAt":epoch(&t["usageLimitResetAt"]),"latestRunCompletedAt":completed,"revision":revision,"parentThreadId":t["lineage"]["parentThreadId"]});
        // Waiting on its own agents or background work, not on the user.
        if !working && (background || blocked_on_wait) {
            row["backgroundWaiting"] = json!(true);
        }
        if let Some(relationship) = t["lineage"]["relationshipToParent"].as_str() {
            row["relationshipToParent"] = json!(relationship);
        }
        // Match native isProviderNativeSubagentThread; MCP children remain writable.
        if t["lineage"]["relationshipToParent"] == "subagent" && t["creationSource"] == "provider" {
            row["providerSubagent"] = json!(true);
        }
        for (key, value) in sidebar::sort_timestamps(t).as_object().unwrap() {
            row[key] = value.clone();
        }
        result.push(row);
    }
    result.sort_by(|a, b| epoch(&b["updatedAt"]).cmp(&epoch(&a["updatedAt"])).then_with(|| text(a, "key").cmp(text(b, "key"))));
    Ok(result)
}

/// One environment's projects and fully enriched rows (ot3 `snapshot`, per environment).
pub fn environment(d: &Value, env: &str, name: &str, now: i64) -> Result<(Vec<Value>, Vec<Value>)> {
    let projects = d["projects"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| json!({"id":p["id"],"environmentId":env,"key":format!("{}/{}",env,text(p,"id")),"title":p["title"],"workspaceRoot":p["workspaceRoot"],"repositoryIdentity":p["repositoryIdentity"],"faviconPath":p["faviconPath"],"projectIcon":p["projectIcon"],"defaultThreadEnvMode":p["defaultThreadEnvMode"],"autoPull":p["autoPull"],"isScratch":p["isScratch"],"defaultModelSelection":p["defaultModelSelection"]}))
        .collect();
    let mut items = rows(d, env, now)?;
    let raw: HashMap<&str, &Value> = d["threads"].as_array().into_iter().flatten().map(|t| (text(t, "id"), t)).collect();
    for item in &mut items {
        let Some(t) = raw.get(text(item, "id")) else { continue };
        for (key, value) in sidebar::metadata(t, now).as_object().unwrap() {
            item[key] = value.clone();
        }
        if item["snoozed"] == false && !text(item, "reason").is_empty() {
            item["pending"] = json!(true);
        }
        let runtime = if t["runtime"].is_object() { &t["runtime"] } else { &t["session"] };
        // Newer servers keep the failure on the thread itself (lastError/lastErrorClass).
        let last_error = runtime["lastError"].as_str().or(t["lastError"].as_str()).map(str::trim).filter(|s| !s.is_empty());
        item["runtimeError"] = json!(last_error.map(|s| s.chars().take(600).collect::<String>()));
        if last_error.is_some() {
            item["hasRuntimeError"] = json!(true);
            if item["runtimeErrorClass"].is_null() {
                item["runtimeErrorClass"] = t["lastErrorClass"].clone();
            }
        }
        item["compactActivityAt"] = json!(epoch(&t["latestUserAuthoredMessageAt"]).max(epoch(&t["latestRunCompletedAt"])));
        item["compactRunId"] = t["latestRunId"].clone();
        item["lastSentAt"] = json!(epoch(t.get("latestUserAuthoredMessageAt").unwrap_or(&t["latestRunRequestedAt"])).max(epoch(&t["createdAt"])));
        item["environmentName"] = json!(name);
        item["projectIsScratch"] = json!(d["projects"].as_array().into_iter().flatten().find(|p| p["id"] == t["projectId"]).is_some_and(|p| p["isScratch"] == true));
        item["projectId"] = t["projectId"].clone();
        item["projectKey"] = json!(format!("{}/{}", env, text(t, "projectId")));
        item["settled"] = json!(is_settled(t));
        item["pinned"] = json!(truth(&t["pinnedAt"]));
        item["startedAt"] = json!(epoch(&t["activityRunStartedAt"]).max(epoch(&t["latestRunStartedAt"])));
        item["provider"] = t["providerInstanceId"].clone();
        item["branch"] = t["branch"].clone();
        item["model"] = t["modelSelection"]["model"].clone();
        for field in ["modelSelection", "runtimeMode", "interactionMode", "activeRunId", "worktreePath", "pinnedAt", "pinOrderKey", "activeOrderKey", "autoSettleDisabledAt", "titleRegeneration"] {
            item[field] = t[field].clone();
        }
        item["projectMonogram"] = json!(crate::projects::monogram(text(item, "project")));
    }
    Ok((projects, items))
}

/// Every environment's rows in one list (ot3 `snapshot`). Each environment is
/// `{id, name?, shell?, state?}`: `shell` is its `orchestration.getShellSnapshot`
/// (or the live shell stream's state); `state` is the host's connection state and
/// overrides `ready` (a cached shell stays listed while `unreachable`).
pub fn snapshot(environments: &[Value], now: i64) -> Result<Value> {
    let mut threads = vec![];
    let mut states = vec![];
    let mut projects = vec![];
    let mut seen = HashSet::new();
    for e in environments {
        let id = e["id"].as_str().filter(|id| !id.is_empty() && id.len() <= 512 && !id.contains('/') && !id.chars().any(char::is_control)).ok_or("invalid_environments")?;
        if !seen.insert(id) {
            return Err("invalid_environments".into());
        }
        let name = e["name"].as_str().unwrap_or(id);
        let host = e["state"].as_str().filter(|s| !s.is_empty());
        let state = match e.get("shell").filter(|s| !s.is_null()) {
            None => host.unwrap_or("unavailable").to_string(),
            Some(shell) => match environment(shell, id, name, now) {
                Ok((mut p, mut items)) => {
                    projects.append(&mut p);
                    threads.append(&mut items);
                    host.unwrap_or("ready").to_string()
                }
                Err(error) => error,
            },
        };
        states.push(json!({"id":id,"name":name,"state":state}));
    }
    threads.sort_by(|a, b| b["updatedAt"].as_i64().cmp(&a["updatedAt"].as_i64()).then_with(|| text(a, "key").cmp(text(b, "key"))));
    let working = threads.iter().filter(|t| t["working"] == true).count();
    let pending = threads.iter().filter(|t| t["pending"] == true).count();
    Ok(json!({"stale":states.iter().any(|e|e["state"]!="ready"),"environments":states,"projects":projects,"threads":threads,"working":working,"pending":pending}))
}

// SidebarModel.js

/// The row label's state: approval, input, plan, waiting, goal, working,
/// limited, failed, woke, done or "" (nothing to say).
pub fn detail(t: &Value) -> &'static str {
    let Some(s) = t.get("sidebarStatus") else {
        let legacy = legacy_state(t);
        return if legacy == "waiting" {
            if t["reason"] == "plan" {
                "plan"
            } else {
                "input"
            }
        } else if legacy == "idle" || (legacy == "done" && !truth(&t["pending"])) {
            ""
        } else {
            legacy
        };
    };
    let pending = truth(&t["pending"]);
    match s.as_str().unwrap_or("") {
        "approval" => "approval",
        "input" => "input",
        "ready" if pending && t["reason"] == "plan" => "plan",
        "ready" if truth(&t["woke"]) && !truth(&t["snoozed"]) => "woke",
        "ready" if pending && t["reason"] == "review" => "done",
        "ready" | "" => "",
        "waiting" => "waiting",
        "goal" => "goal",
        "working" => "working",
        "limited" => "limited",
        "failed" => "failed",
        // A status this build does not know keeps its own bucket out of every shelf.
        _ => "unknown",
    }
}
/// The shelf and filter bucket: waiting (needs you), working, failed, done, woke or idle.
pub fn state(t: &Value) -> &'static str {
    if t.get("sidebarStatus").is_none() {
        return legacy_state(t);
    }
    match detail(t) {
        "approval" | "input" | "plan" => "waiting",
        "goal" | "working" => "working",
        "limited" | "failed" => "failed",
        "done" => "done",
        "woke" => "woke",
        _ => "idle",
    }
}
fn legacy_state(t: &Value) -> &'static str {
    if truth(&t["pending"]) && matches!(t["reason"].as_str(), Some("request" | "plan" | "waiting")) {
        "waiting"
    } else if truth(&t["working"]) {
        "working"
    } else if t["status"] == "failed" {
        "failed"
    } else if t["status"] == "completed" {
        "done"
    } else {
        "idle"
    }
}
/// The English label ot3 shows for a detail (T3 SidebarThreadRow; "Plan ready" is ot3's).
pub fn label(detail: &str) -> &'static str {
    match detail {
        "approval" => "Approval",
        "input" => "Input",
        "plan" => "Plan ready",
        "waiting" => "Waiting",
        "goal" => "Goal",
        "working" => "Working",
        "limited" => "Limited",
        "failed" => "Failed",
        "woke" => "Woke",
        "done" => "Done",
        _ => "",
    }
}
/// The colour role of a detail (ot3 `T3Palette.detailColor`): working, done,
/// failed, warning, input, waiting or muted (foreground at 0.6).
pub fn role(detail: &str) -> &'static str {
    match detail {
        "working" | "goal" => "working",
        "done" => "done",
        "failed" => "failed",
        "limited" | "woke" | "approval" => "warning",
        "input" => "input",
        "plan" => "waiting",
        _ => "muted",
    }
}

/// Which project a filter covers: one key, or every member of a project group.
#[derive(Clone, Debug, PartialEq)]
pub enum Scope {
    All,
    One(String),
    Many(Vec<String>),
}
impl Scope {
    fn covers(&self, key: &Value) -> bool {
        match self {
            Scope::All => true,
            Scope::One(k) => key.as_str() == Some(k),
            Scope::Many(keys) => key.as_str().is_some_and(|k| keys.iter().any(|m| m == k)),
        }
    }
    fn to_json(&self) -> Value {
        match self {
            Scope::All => json!(""),
            Scope::One(k) => json!(k),
            Scope::Many(keys) => json!(keys),
        }
    }
}
pub const FILTERS: &[&str] = &["all", "working", "done", "failed", "pending", "settled"];
fn subagent(t: &Value) -> bool {
    t["relationshipToParent"] == "subagent"
}
fn matches(t: &Value, filter: &str, scope: &Scope, environment: Option<&str>) -> bool {
    if subagent(t) || environment.is_some_and(|e| t["environmentId"] != e) {
        return false;
    }
    scope.covers(&t["projectKey"])
        && match filter {
            "all" => true,
            "settled" => truth(&t["settled"]),
            "pending" => truth(&t["pending"]),
            f => state(t) == f,
        }
}
/// ot3 `Sidebar.rows`: matching rows, then a settled header and (when expanded,
/// filtering on settled or searching) the settled rows.
fn filtered<'a>(threads: &'a [Value], filter: &str, scope: &Scope, environment: Option<&str>, expanded: bool, query: &str) -> Vec<&'a Value> {
    let needle = query.to_lowercase();
    let (mut result, mut settled) = (vec![], vec![]);
    for t in threads.iter().filter(|t| matches(t, filter, scope, environment)) {
        let hay = [&t["title"], &t["project"], &t["environmentId"]].iter().map(|v| js_string(v)).collect::<Vec<_>>().join(" ").to_lowercase();
        if needle.is_empty() || hay.contains(&needle) {
            if truth(&t["settled"]) {
                settled.push(t)
            } else {
                result.push(t)
            }
        }
    }
    if expanded || filter == "settled" || !needle.is_empty() {
        result.append(&mut settled);
    }
    result
}
fn js_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
/// The header's count pills: unsettled top-level rows per bucket.
pub fn counts(threads: &[Value]) -> Value {
    let mut totals = [("working", 0), ("done", 0), ("waiting", 0), ("failed", 0)];
    for t in threads.iter().filter(|t| !truth(&t["settled"]) && !subagent(t)) {
        let s = state(t);
        if let Some(n) = totals.iter_mut().find(|(k, _)| *k == s) {
            n.1 += 1;
        }
    }
    json!(totals.iter().map(|(k, n)| (k.to_string(), json!(n))).collect::<Map<_, _>>())
}
/// Work that needs no decision folds into the Working shelf; pinned rows stay put.
fn in_working_shelf(t: &Value) -> bool {
    state(t) == "working" && !truth(&t["pinned"])
}
/// ot3 `observeReturns`: a thread that leaves Working returns to the inbox at `now`.
/// Pass the result back as `returns` on the next call.
pub fn observe_returns(previous: &Value, threads: &[Value], now: i64) -> Value {
    let present: HashSet<&str> = threads.iter().map(|t| text(t, "key")).collect();
    let working: BTreeMap<String, Value> = threads.iter().filter(|t| state(t) == "working").map(|t| (text(t, "key").to_string(), json!(true))).collect();
    let mut returned = Map::new();
    for (k, at) in previous["returned"].as_object().into_iter().flatten() {
        if present.contains(k.as_str()) {
            returned.insert(k.clone(), at.clone());
        }
    }
    for k in previous["working"].as_object().into_iter().flatten().map(|(k, _)| k) {
        if present.contains(k.as_str()) && !working.contains_key(k) {
            returned.insert(k.clone(), json!(now));
        }
    }
    json!({"working":working,"returned":returned})
}
/// A JS `a || b || 0` over numbers: the first non-zero one.
fn first(values: &[&Value]) -> f64 {
    values.iter().map(|v| v.as_f64().unwrap_or(0.)).find(|n| *n != 0.).unwrap_or(0.)
}
fn by_number(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}
/// Thread ids (then environments) case-insensitively, then by code unit: a stable
/// stand-in for T3's `localeCompare` identity tie-break.
fn tie(a: &Value, b: &Value) -> Ordering {
    let fold = |v: &Value| js_string(v).to_ascii_lowercase();
    fold(&a["id"]).cmp(&fold(&b["id"])).then_with(|| js_string(&a["id"]).cmp(&js_string(&b["id"]))).then_with(|| fold(&a["environmentId"]).cmp(&fold(&b["environmentId"])))
}
fn returned_at(t: &Value, returned: &Value) -> f64 {
    t["inboxAt"].as_f64().unwrap_or(0.).max(returned[text(t, "key")].as_f64().unwrap_or(0.))
}
/// Pinned first by their order key, then the inbox (grouped) or T3's active order.
fn compare_active(a: &Value, b: &Value, grouped: bool, returned: &Value) -> Ordering {
    let (pa, pb) = (truth(&a["pinned"]), truth(&b["pinned"]));
    if pa != pb {
        return if pa { Ordering::Less } else { Ordering::Greater };
    }
    if pa {
        // pinnedAt only breaks ties when both are numbers, as in ot3.
        let at = |t: &Value| if truth(&t["pinnedAt"]) { t["pinnedAt"].as_f64() } else { Some(0.) };
        let pinned = match (at(a), at(b)) {
            (Some(x), Some(y)) => by_number(x, y),
            _ => Ordering::Equal,
        };
        return js_string(&a["pinOrderKey"]).cmp(&js_string(&b["pinOrderKey"])).then(pinned).then_with(|| tie(a, b));
    }
    if grouped {
        return by_number(returned_at(b, returned), returned_at(a, returned)).then_with(|| tie(a, b));
    }
    let (ka, kb) = (&a["activeOrderKey"], &b["activeOrderKey"]);
    match (ka.is_null(), kb.is_null()) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => js_string(ka).cmp(&js_string(kb)).then_with(|| tie(a, b)),
        (true, true) => by_number(b["activeAnchorAt"].as_f64().unwrap_or(0.), a["activeAnchorAt"].as_f64().unwrap_or(0.)).then_with(|| tie(a, b)),
    }
}
fn compare_settled(a: &Value, b: &Value) -> Ordering {
    by_number(first(&[&b["settledSortAt"], &b["updatedAt"]]), first(&[&a["settledSortAt"], &a["updatedAt"]])).then_with(|| tie(a, b))
}

/// Sub-threads (subagents) by their parent's key, newest first.
fn children_by_parent(threads: &[Value]) -> HashMap<String, Vec<&Value>> {
    let mut map: HashMap<String, Vec<&Value>> = HashMap::new();
    for t in threads.iter().filter(|t| subagent(t) && truth(&t["parentThreadId"])) {
        map.entry(format!("{}/{}", js_string(&t["environmentId"]), js_string(&t["parentThreadId"]))).or_default().push(t);
    }
    for kids in map.values_mut() {
        kids.sort_by(|a, b| by_number(first(&[&b["lastSentAt"], &b["updatedAt"]]), first(&[&a["lastSentAt"], &a["updatedAt"]])).then_with(|| tie(a, b)));
    }
    map
}
fn descendant_keys(key: &str, map: &HashMap<String, Vec<&Value>>, depth: usize) -> Vec<String> {
    let mut out = vec![];
    if depth > 4 {
        return out;
    }
    for c in map.get(key).into_iter().flatten() {
        out.push(text(c, "key").to_string());
        out.extend(descendant_keys(text(c, "key"), map, depth + 1));
    }
    out
}
/// A parent opens on its own while a child works or waits, or holds the
/// selection; an explicit choice (`overrides[key]`) always wins.
fn subthreads_expanded(key: &str, map: &HashMap<String, Vec<&Value>>, overrides: &Value, selected: &str) -> bool {
    if let Some(choice) = overrides.get(key) {
        return truth(choice);
    }
    map.get(key).into_iter().flatten().any(|c| matches!(state(c), "working" | "waiting")) || descendant_keys(key, map, 0).iter().any(|k| k == selected)
}
fn child_summary(kids: &[&Value]) -> Value {
    let mut s = json!({"count":kids.len(),"working":0,"waiting":0,"failed":0,"done":0});
    for c in kids {
        let k = state(c);
        if let Some(n) = s[k].as_u64() {
            s[k] = json!(n + 1);
        }
    }
    s
}
/// Each section lists sub-threads right under their parent, up to depth 4.
fn nest(rows: Vec<&Value>, map: &HashMap<String, Vec<&Value>>, overrides: &Value, selected: &str) -> Vec<Value> {
    fn add(t: &Value, depth: usize, parent: &str, map: &HashMap<String, Vec<&Value>>, overrides: &Value, selected: &str, out: &mut Vec<Value>) {
        let kids = if depth < 4 { map.get(text(t, "key")).cloned().unwrap_or_default() } else { vec![] };
        let mut row = t.clone();
        let mut open = false;
        if !kids.is_empty() || depth > 0 {
            row["depth"] = json!(depth);
            row["subthreadParentKey"] = json!(parent);
            if !kids.is_empty() {
                open = subthreads_expanded(text(t, "key"), map, overrides, selected);
                row["subthreads"] = child_summary(&kids);
                row["subthreadsExpanded"] = json!(open);
            }
        }
        out.push(row);
        if open {
            for c in kids {
                add(c, depth + 1, text(t, "key"), map, overrides, selected, out);
            }
        }
    }
    let mut out = vec![];
    for t in rows {
        if map.is_empty() {
            out.push(t.clone());
        } else if !(subagent(t) && truth(&t["parentThreadId"])) {
            add(t, 0, "", map, overrides, selected, &mut out);
        }
    }
    out
}

// ProjectModel.js

fn clean_path(value: &Value) -> String {
    let p = js_string(value).replace('\\', "/");
    if p.chars().count() > 1 {
        p.trim_end_matches('/').to_string()
    } else {
        p
    }
}
fn physical(p: &Value) -> String {
    format!("{}:{}", js_string(&p["environmentId"]), clean_path(&p["workspaceRoot"]))
}
/// How projects group: T3's sidebarProjectGroupingMode (`repository`,
/// `repository_path` or `separate`), per-project overrides keyed by
/// `<environmentId>:<workspaceRoot>` (`inherit` follows the mode), and the
/// desktop client's own setting as the fallback (`native`).
#[derive(Clone, Copy)]
pub struct Grouping<'a> {
    pub mode: Option<&'a str>,
    pub overrides: &'a Value,
    pub native: &'a Value,
}
fn mode<'a>(p: &Value, g: &Grouping<'a>) -> String {
    let global = g.mode.filter(|m| !m.is_empty()).or_else(|| g.native["mode"].as_str().filter(|m| !m.is_empty())).unwrap_or("repository");
    let key = physical(p);
    match g.overrides[&key].as_str().filter(|m| !m.is_empty()) {
        Some("inherit") => global.to_string(),
        Some(own) => own.to_string(),
        None => g.native["overrides"][&key].as_str().filter(|m| !m.is_empty()).unwrap_or(global).to_string(),
    }
}
fn group_key(p: &Value, g: &Grouping) -> String {
    let m = mode(p, g);
    let r = &p["repositoryIdentity"];
    let canonical = r["origin"]["canonicalKey"].as_str().filter(|k| !k.is_empty()).or_else(|| r["canonicalKey"].as_str().filter(|k| !k.is_empty()));
    let Some(canonical) = canonical.filter(|_| m != "separate") else { return js_string(&p["key"]) };
    let (base, cwd) = (clean_path(&r["rootPath"]), clean_path(&p["workspaceRoot"]));
    let suffix = match cwd.strip_prefix(&format!("{base}/")) {
        Some(rest) if m == "repository_path" && !base.is_empty() => format!("::{rest}"),
        _ => String::new(),
    };
    format!("repository/{canonical}{suffix}")
}
/// Projects grouped by repository (or path, or not at all), in first-seen order.
pub fn groups(projects: &[Value], g: &Grouping) -> Vec<Value> {
    let mut result: Vec<Value> = vec![];
    for p in projects {
        let key = group_key(p, g);
        match result.iter_mut().find(|r| r["key"] == key.as_str()) {
            Some(r) => r["members"].as_array_mut().unwrap().push(p.clone()),
            None => result.push(json!({"key":key,"title":p["title"],"members":[p]})),
        }
    }
    result
}
/// The project keys a filter (a group key or one project's key) covers.
pub fn scope(filter: &str, groups: &[Value]) -> Scope {
    let group = groups.iter().find(|g| g["key"] == filter || g["members"].as_array().into_iter().flatten().any(|m| m["key"] == filter));
    match group {
        Some(g) => Scope::Many(g["members"].as_array().into_iter().flatten().map(|m| js_string(&m["key"])).collect()),
        None if filter.is_empty() => Scope::All,
        None => Scope::One(filter.to_string()),
    }
}

/// The time shown at a row's end (ot3 `T3Panel.timeLabel`): elapsed while
/// working, the snooze countdown, else the age. `label` is for `now`; hosts
/// that tick between views recompute it from `kind` and `at`.
pub fn time(t: &Value, now: i64) -> Value {
    let (kind, at) = if truth(&t["working"]) {
        ("elapsed", t["startedAt"].as_i64().unwrap_or(0))
    } else if truth(&t["snoozed"]) && truth(&t["snoozedUntil"]) {
        ("countdown", epoch(&t["snoozedUntil"]))
    } else if truth(&t["settled"]) {
        ("age", first(&[&t["settledSortAt"], &t["updatedAt"]]) as i64)
    } else {
        ("age", t["updatedAt"].as_i64().unwrap_or(0))
    };
    json!({"kind":kind,"at":at,"label":time_label(kind, at, now)})
}
/// `12s`, `3m 5s`; `now`, `5m`, `3h`, `2d` (countdowns round up); `—` without a time.
pub fn time_label(kind: &str, at: i64, now: i64) -> String {
    match kind {
        "elapsed" if at == 0 => String::new(),
        "elapsed" => {
            let s = (now - at).max(0) / 1000;
            if s < 60 {
                format!("{s}s")
            } else {
                format!("{}m {}s", s / 60, s % 60)
            }
        }
        "countdown" => {
            let left = at - now;
            if at == 0 || left <= 0 {
                "now".into()
            } else if left < 3_600_000 {
                format!("{}m", ((left + 59_999) / 60_000).max(1))
            } else if left < 86_400_000 {
                format!("{}h", (left + 3_599_999) / 3_600_000)
            } else {
                format!("{}d", (left + 86_399_999) / 86_400_000)
            }
        }
        _ if at == 0 => "—".into(),
        _ => {
            let mins = (now - at).max(0) / 60_000;
            if mins < 1 {
                "now".into()
            } else if mins < 60 {
                format!("{mins}m")
            } else if mins < 1440 {
                format!("{}h", mins / 60)
            } else {
                format!("{}d", mins / 1440)
            }
        }
    }
}

/// What the host keeps between views and passes back.
pub struct ViewState<'a> {
    pub filter: &'a str,
    pub project: &'a str,
    pub environment: Option<&'a str>,
    pub query: &'a str,
    /// "Group working, snoozed and settled threads" (ot3's compact sidebar).
    pub grouped: bool,
    pub subthreads_expanded: &'a Value,
    pub selected: &'a str,
    /// The previous view's `returns`.
    pub returns: &'a Value,
    pub grouping: Grouping<'a>,
}
fn present(rows: Vec<Value>, now: i64) -> Vec<Value> {
    rows.into_iter()
        .map(|mut row| {
            let d = detail(&row);
            row["detail"] = json!(d);
            row["state"] = json!(state(&row));
            row["statusLabel"] = json!(label(d));
            row["statusRole"] = json!(role(d));
            row["time"] = time(&row, now);
            row
        })
        .collect()
}
fn top_level(rows: &[Value]) -> usize {
    rows.iter().filter(|r| !truth(&r["depth"])).count()
}
/// The whole sidebar: ot3's sections, ordered and nested, with the counts.
pub fn view(snapshot: &Value, v: &ViewState, now: i64) -> Value {
    let threads = snapshot["threads"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let projects = snapshot["projects"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let groups = groups(projects, &v.grouping);
    let scope = scope(v.project, &groups);
    let returns = if v.grouped { observe_returns(v.returns, threads, now) } else { json!({"working":null,"returned":{}}) };
    let returned = &returns["returned"];
    let map = children_by_parent(threads);
    let inbox = filtered(threads, v.filter, &scope, v.environment, false, v.query).into_iter().filter(|t| !truth(&t["settled"]) && !truth(&t["snoozed"])).collect::<Vec<_>>();
    let mut active = inbox.iter().copied().filter(|t| !v.grouped || !in_working_shelf(t)).collect::<Vec<_>>();
    active.sort_by(|a, b| compare_active(a, b, v.grouped, returned));
    let mut working = if v.grouped { inbox.iter().copied().filter(|t| in_working_shelf(t)).collect::<Vec<_>>() } else { vec![] };
    working.sort_by(|a, b| by_number(first(&[&b["lastSentAt"], &b["startedAt"], &b["updatedAt"]]), first(&[&a["lastSentAt"], &a["startedAt"], &a["updatedAt"]])));
    let all = filtered(threads, v.filter, &scope, v.environment, true, v.query);
    let mut settled = all.iter().copied().filter(|t| truth(&t["settled"]) && !truth(&t["snoozed"])).collect::<Vec<_>>();
    settled.sort_by(|a, b| compare_settled(a, b));
    let snoozed = all.iter().copied().filter(|t| truth(&t["snoozed"])).collect::<Vec<_>>();
    let section = |rows: Vec<&Value>| present(nest(rows, &map, v.subthreads_expanded, v.selected), now);
    let (active, working, snoozed, settled) = (section(active), section(working), section(snoozed), section(settled));
    let filter_counts = FILTERS.iter().map(|f| (f.to_string(), json!(threads.iter().filter(|t| matches(t, f, &scope, v.environment)).count()))).collect::<Map<_, _>>();
    json!({
        "environments":snapshot["environments"],"stale":snapshot["stale"],"working":snapshot["working"],"pending":snapshot["pending"],
        "projects":projects,"projectGroups":groups,"projectScope":scope.to_json(),
        "sections":{"active":active,"working":working,"snoozed":snoozed,"settled":settled},
        "sectionCounts":{"active":top_level(&active),"working":top_level(&working),"snoozed":top_level(&snoozed),"settled":top_level(&settled)},
        "counts":counts(threads),"filterCounts":filter_counts,"returns":returns,
        "threads":present(threads.to_vec(), now),
    })
}
