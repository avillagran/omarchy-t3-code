//! Renderer-neutral contracts shared by Desktop and Android.
use crate::{util::text, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
fn rows<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v[k].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn stamp(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| {
        v.as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.timestamp_millis())
            .unwrap_or(0)
    })
}
pub fn permission(v: &Value) -> Value {
    let definitions = json!([
 {"id":"approval-required","label":"Supervised","description":"Ask before commands and file changes.","icon":"lock"},
 {"id":"auto-accept-edits","label":"Auto-accept edits","description":"Auto-approve edits, ask before other actions.","icon":"pen-line"},
 {"id":"auto","label":"Auto","description":"Supported providers approve routine actions; others still ask.","icon":"sparkles"},
 {"id":"full-access","label":"Full access","description":"Allow commands and edits without prompts.","icon":"lock-open"}]);
    let provider = rows(&v["catalog"], "providers")
        .iter()
        .find(|p| p["instanceId"] == v["selection"]["instanceId"]);
    let supported = provider
        .map(|p| rows(p, "supportedRuntimeModes"))
        .unwrap_or(&[]);
    let choices = definitions
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| supported.is_empty() || supported.contains(&d["id"]))
        .cloned()
        .collect::<Vec<_>>();
    let current = choices
        .iter()
        .find(|d| d["id"] == v["runtimeMode"])
        .or(choices.first())
        .unwrap_or(&definitions[0]);
    json!({"current":current,"rows":choices})
}
fn merge(sets: &[&[Value]]) -> Result<Vec<Value>> {
    let mut map = BTreeMap::new();
    for set in sets {
        for row in *set {
            let id = row["id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("invalid_row_id")?;
            map.insert(id.to_string(), row.clone());
        }
    }
    let mut out = map.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| {
        stamp(&a["createdAt"])
            .cmp(&stamp(&b["createdAt"]))
            .then_with(|| text(a, "id").cmp(text(b, "id")))
    });
    Ok(out)
}
pub fn history(v: &Value) -> Result<Value> {
    let apply = v["cursor"].is_string()
        && v.get("page").is_some()
        && v["pageOwner"] == v["owner"]
        && v.get("pageCursor") == v.get("cursor");
    let older = merge(&[rows(v, "older"), if apply { rows(v, "page") } else { &[] }])?;
    let timeline = merge(&[&older, rows(v, "recent")])?;
    Ok(
        json!({"older":older,"timeline":timeline,"applied":apply,"cursor":if apply {&v["nextCursor"]}else{&v["cursor"]},"hasMoreHistory":if apply {v["hasMoreHistory"].as_bool().unwrap_or(false)}else{!v["cursor"].is_null()}}),
    )
}
pub fn pr_list(pages: &[Value]) -> Result<Value> {
    let mut map = BTreeMap::new();
    for page in pages {
        for r in rows(page, "items") {
            let number = r["number"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or("invalid_pull_request")?;
            let key = (
                text(r, "environmentId").to_string(),
                text(r, "host").to_ascii_lowercase(),
                text(r, "repository").to_ascii_lowercase(),
                number,
            );
            if key.1.is_empty() || key.2.is_empty() {
                return Err("invalid_pull_request".into());
            }
            map.insert(key, r.clone());
        }
    }
    let mut items = map.into_values().collect::<Vec<_>>();
    items.sort_by(|a, b| stamp(&b["updatedAt"]).cmp(&stamp(&a["updatedAt"])));
    Ok(json!({"items":items}))
}
const LIMIT: usize = 65536;
fn tail(s: &mut String) -> bool {
    if s.len() <= LIMIT {
        return false;
    }
    let mut cut = s.len() - LIMIT;
    while !s.is_char_boundary(cut) {
        cut += 1
    }
    s.drain(..cut);
    true
}
pub fn output(v: &Value) -> Result<Value> {
    let reference = &v["reference"];
    for k in ["threadId", "itemId"] {
        crate::api::ids::id(k, text(reference, k))?;
    }
    let mut state = if v["state"]["reference"] == *reference {
        v["state"].clone()
    } else {
        json!({"reference":reference})
    };
    let mut shown = text(&state, "text").to_string();
    let mut escape = text(&state, "escape").to_string();
    let mut cr = state["cr"] == true;
    let mut truncated = state["truncated"] == true;
    truncated |= tail(&mut shown);
    let mut running = state["running"] != false;
    for f in rows(v, "frames") {
        if f["kind"] != "append" && f["kind"] != "replace" {
            return Err("invalid_output_frame".into());
        }
        if f.get("reference").is_some_and(|r| r != reference) {
            continue;
        }
        if f["kind"] == "replace" {
            shown.clear();
            escape.clear();
            cr = false;
            truncated = false;
        }
        let chunk = f["text"].as_str().ok_or("invalid_output_frame")?;
        for c in chunk.chars() {
            if !escape.is_empty() {
                match escape.as_str() {
                    "esc" => {
                        escape = match c {
                            '[' => "csi",
                            ']' => "osc",
                            _ => "",
                        }
                        .into();
                    }
                    "csi" => {
                        if ('@'..='~').contains(&c) {
                            escape.clear()
                        }
                    }
                    "osc" => {
                        if c == '\u{7}' {
                            escape.clear()
                        } else if c == '\u{1b}' {
                            escape = "oscEsc".into()
                        }
                    }
                    "oscEsc" => {
                        escape = if c == '\\' { "" } else { "osc" }.into();
                    }
                    _ => escape.clear(),
                };
                continue;
            }
            if cr {
                if c != '\n' {
                    let start = shown.rfind('\n').map(|i| i + 1).unwrap_or(0);
                    shown.truncate(start);
                }
                cr = false;
            }
            match c {
                '\u{1b}' => escape = "esc".into(),
                '\r' => cr = true,
                c if c.is_control() && c != '\n' && c != '\t' => {}
                c => shown.push(c),
            }
            truncated |= tail(&mut shown);
        }
        truncated |= f["truncated"] == true;
        running = f["running"] != false;
    }
    if v["closed"] == true || v["error"].is_string() {
        running = false;
    }
    state = json!({"reference":reference,"text":shown,"escape":escape,"cr":cr,"truncated":truncated,"running":running});
    Ok(
        json!({"state":state,"text":shown,"running":running,"truncated":truncated,"error":v["error"]}),
    )
}
fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 256
        && k.bytes().all(|c| c.is_ascii_lowercase())
        && !k.ends_with('a')
}
fn midpoint(a: &str, b: &str) -> String {
    if !b.is_empty() {
        let mut n = 0;
        while n < b.len() && a.as_bytes().get(n).copied().unwrap_or(b'a') == b.as_bytes()[n] {
            n += 1
        }
        if n > 0 {
            return format!("{}{}", &b[..n], midpoint(a.get(n..).unwrap_or(""), &b[n..]));
        }
    }
    let da = a.as_bytes().first().map(|c| c - b'a').unwrap_or(0);
    let db = b.as_bytes().first().map(|c| c - b'a').unwrap_or(26);
    if db - da > 1 {
        return ((b'a' + (da + db).div_ceil(2)) as char).to_string();
    }
    if b.len() > 1 {
        return b[..1].into();
    }
    format!(
        "{}{}",
        (b'a' + da) as char,
        midpoint(a.get(1..).unwrap_or(""), "")
    )
}
pub fn pins(v: &Value) -> Result<Value> {
    let ids = rows(v, "orderedIds");
    if ids.len() > 10000 {
        return Err("invalid_pin_order".into());
    }
    let mut seen = HashSet::new();
    for id in ids {
        let id = id.as_str().ok_or("invalid_pin_order")?;
        crate::api::ids::id("threadId", id)?;
        if !seen.insert(id) {
            return Err("invalid_pin_order".into());
        }
    }
    let i = ids
        .iter()
        .position(|id| id == &v["movedId"])
        .ok_or("invalid_pin_order")?;
    let key = |n: usize| v["keys"][ids[n].as_str().unwrap()].as_str();
    let a = if i == 0 { Some("") } else { key(i - 1) };
    let b = if i + 1 == ids.len() {
        Some("")
    } else {
        key(i + 1)
    };
    if let (Some(a), Some(b)) = (a, b) {
        if (a.is_empty() || valid_key(a))
            && (b.is_empty() || valid_key(b))
            && (b.is_empty() || a < b)
        {
            let next = midpoint(a, b);
            if next.len() <= 256 {
                return Ok(json!([{"id":v["movedId"],"orderKey":next}]));
            }
        }
    }
    let mut width = 2;
    let mut space = 26u64.pow(width);
    while space <= ((ids.len() + 1) * 2) as u64 {
        width += 1;
        space *= 26
    }
    Ok(json!(ids
        .iter()
        .enumerate()
        .filter_map(|(n, id)| {
            let mut value = (space as f64 / (ids.len() + 1) as f64 * (n + 1) as f64).round() as u64;
            if value.is_multiple_of(26) {
                value += 1
            }
            let mut bytes = vec![b'a'; width as usize];
            for c in bytes.iter_mut().rev() {
                *c += (value % 26) as u8;
                value /= 26
            }
            let key = String::from_utf8(bytes).unwrap();
            (v["keys"][id.as_str().unwrap()] != key).then(|| json!({"id":id,"orderKey":key}))
        })
        .collect::<Vec<_>>()))
}
fn byte_offset(s: &str, n: u64) -> Result<usize> {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units == n {
            return Ok(i);
        }
        units += c.len_utf16() as u64;
    }
    if units == n {
        Ok(s.len())
    } else {
        Err("invalid_selection".into())
    }
}
fn edit(s: &str, start: usize, end: usize, insert: &str) -> Value {
    json!({"text":format!("{}{}{}",&s[..start],insert,&s[end..]),"cursor":s[..start].encode_utf16().count()+insert.encode_utf16().count(),"changed":true})
}
pub fn composer(v: &Value) -> Result<Value> {
    let s = text(v, "text");
    if s.len() > 4 * 1024 * 1024 {
        return Err("invalid_text".into());
    }
    if v.get("expectedText").is_some_and(|e| e != s) {
        return Err("stale_editor".into());
    }
    if v["composing"] == true {
        return Ok(json!({"changed":false}));
    }
    if v["action"] == "recall" {
        return Ok(json!(rows(v, "messages")
            .iter()
            .rev()
            .filter(|m| m["role"] == "user" && m["attribution"].is_null())
            .map(|m| {
                let raw = m["recallableText"].as_str().unwrap_or(text(m, "text"));
                let clean = raw
                    .rfind("<t3_context")
                    .filter(|_| raw.contains("</t3_context>"))
                    .map(|i| &raw[..i])
                    .unwrap_or(raw);
                clean.trim().to_string()
            })
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()));
    }
    let cursor = v["cursor"].as_u64().ok_or("invalid_selection")?;
    let end = byte_offset(s, cursor)?;
    match text(v, "action") {
        "replace" => {
            let start = byte_offset(s, v["start"].as_u64().ok_or("invalid_selection")?)?;
            if start > end {
                return Err("invalid_selection".into());
            }
            let expected = v["expectedRange"].as_str().ok_or("invalid_selection")?;
            if &s[start..end] != expected {
                return Err("stale_editor".into());
            }
            Ok(edit(s, start, end, text(v, "insert")))
        }
        "completion" => {
            let prefix = &s[..end];
            let start = prefix
                .char_indices()
                .rev()
                .find(|(_, c)| c.is_whitespace())
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);
            let token = &prefix[start..];
            let (kind, query) = match token.as_bytes().first() {
                Some(b'@') => ("path", &token[1..]),
                Some(b'$') => ("skill", &token[1..]),
                Some(b'/') if start == 0 || s[..start].ends_with('\n') => ("command", &token[1..]),
                _ => return Ok(Value::Null),
            };
            Ok(
                json!({"kind":kind,"query":query,"start":s[..start].encode_utf16().count(),"end":cursor,"expectedRange":token}),
            )
        }
        "list" => {
            let start = s[..end].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line = &s[start..end];
            let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
            let line_body = &line[indent_len..];
            let marker_end = line_body
                .find(char::is_whitespace)
                .unwrap_or(line_body.len());
            let marker = &line_body[..marker_end];
            let rest = &line_body[marker_end..];
            let ordered = marker
                .strip_suffix(['.', ')'])
                .filter(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
                .and_then(|n| n.parse::<u64>().ok());
            if rest.is_empty()
                || (!["-", "+", "*", "•", "◦", "‣", "–"].contains(&marker) && ordered.is_none())
            {
                return Ok(json!({"changed":false}));
            }
            let content = rest.trim_start();
            let task = ["[ ] ", "[x] ", "[X] "]
                .iter()
                .find(|p| content.starts_with(**p));
            let body = task.map(|p| &content[p.len()..]).unwrap_or(content);
            if body.trim().is_empty() && s[end..].split('\n').next().unwrap_or("").trim().is_empty()
            {
                return Ok(edit(s, start, end, ""));
            }
            let marker = if let Some(n) = ordered {
                format!(
                    "{}{}",
                    n.checked_add(1).ok_or("invalid_list")?,
                    marker.chars().last().unwrap()
                )
            } else {
                marker.to_string()
            };
            Ok(edit(
                s,
                end,
                end,
                &format!(
                    "\n{}{} {}",
                    &line[..indent_len],
                    marker,
                    if task.is_some() { "[ ] " } else { "" }
                ),
            ))
        }
        _ => Err("invalid_editor_action".into()),
    }
}
fn decode(s: &str) -> Result<String> {
    let mut bytes = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let b = s.as_bytes()[i];
        if b == b'%' {
            let pair = s.get(i + 1..i + 3).ok_or("invalid_reference")?;
            bytes.push(u8::from_str_radix(pair, 16).map_err(|_| "invalid_reference")?);
            i += 3;
        } else {
            bytes.push(b);
            i += 1;
        }
    }
    let out = String::from_utf8(bytes).map_err(|_| "invalid_reference")?;
    if out.chars().any(char::is_control) {
        return Err("invalid_reference".into());
    }
    Ok(out)
}
pub fn reference(v: &Value) -> Result<Value> {
    let href = text(v, "href").trim();
    if href.is_empty() || href.len() > 4096 || href.chars().any(char::is_control) {
        return Err("invalid_reference".into());
    }
    if let Some(anchor) = href.strip_prefix('#') {
        return Ok(json!({"kind":"anchor","anchor":decode(anchor)?}));
    }
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("https:") || lower.starts_with("http:") {
        let u = reqwest::Url::parse(href).map_err(|_| "invalid_reference")?;
        if !u.username().is_empty() || u.password().is_some() || u.host_str().is_none() {
            return Err("invalid_reference".into());
        }
        return Ok(json!({"kind":"external","url":u.as_str()}));
    }
    if lower.starts_with("t3-thread:") {
        let raw = href
            .get(15..)
            .filter(|_| lower.starts_with("t3-thread://v1/"))
            .ok_or("invalid_reference")?
            .split(['?', '#', '/'])
            .next()
            .unwrap_or("");
        let id = decode(raw)?;
        crate::api::ids::id("threadId", &id)?;
        let mut candidates = rows(v, "environments")
            .iter()
            .filter(|e| e["connected"] != false && rows(e, "threads").iter().any(|t| t["id"] == id))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|e| if e["id"] == v["environmentId"] { 0 } else { 1 });
        return Ok(match candidates.first() {
            Some(e) => json!({"kind":"thread","environmentId":e["id"],"threadId":id}),
            None => json!({"kind":"unsupported","reason":"thread_unavailable"}),
        });
    }
    let raw = if lower.starts_with("file://") {
        let u = reqwest::Url::parse(href).map_err(|_| "invalid_reference")?;
        if u.host_str().is_some_and(|h| h != "localhost") {
            return Err("invalid_reference".into());
        }
        u.path().to_string()
    } else {
        href.to_string()
    };
    let mut path = decode(&raw)?;
    for _ in 0..2 {
        if let Some((p, n)) = path.rsplit_once(':') {
            if !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()) {
                path = p.into();
            }
        }
    }
    if path.contains(':') || path.is_empty() {
        return Ok(json!({"kind":"unsupported","reason":"unsafe_scheme"}));
    }
    let resource = crate::documents::resource(text(v, "threadId"), &path)?;
    Ok(
        json!({"kind":"document","environmentId":v["environmentId"],"threadId":v["threadId"],"resource":resource}),
    )
}

/// T3 terminal assistant semantics: later nonempty assistant before next prompt wins.
pub fn message_actions(rows: &mut [Value]) {
    let mut following = false;
    for row in rows.iter_mut().rev() {
        let assistant = row["role"] == "assistant";
        let user = row["role"] == "user";
        row["commentary"] = json!(assistant && following);
        row["showMessageActions"] = json!(
            (assistant || user)
                && !(assistant && following)
                && row["streaming"] != true
                && (!text(row, "text").is_empty() || row["canRewindMessage"] == true)
        );
        if user {
            following = false;
        } else if assistant && !text(row, "text").is_empty() {
            following = true;
        }
    }
}
/// Source/project/sibling/catalog/draft precedence, without choosing a remote host locally.
pub fn new_thread(v: &Value) -> Result<Value> {
    let projects = rows(v, "projects");
    let query = text(v, "query").to_lowercase();
    let visible = projects
        .iter()
        .filter(|p| {
            query.is_empty()
                || text(p, "title").to_lowercase().contains(&query)
                || text(p, "workspaceRoot").to_lowercase().contains(&query)
        })
        .cloned()
        .collect::<Vec<_>>();
    let Some(project) = projects
        .iter()
        .find(|p| p["id"] == v["projectId"] && p["environmentId"] == v["environmentId"])
    else {
        return Ok(json!({"projects":visible,"ready":false,"missing":["project"]}));
    };
    let defaults = crate::native_settings::thread_defaults(&v["settings"], text(project, "id"));
    let source = &v["source"];
    let sibling = rows(v, "threads").iter().find(|t| {
        t["projectId"] == project["id"]
            && t["environmentId"] == project["environmentId"]
            && t["settled"] != true
            && t["modelSelection"].is_object()
    });
    let available = rows(&v["catalog"], "models")
        .iter()
        .find(|m| m["available"] != false)
        .map(|m| json!({"instanceId":m["instanceId"],"model":m["model"],"options":[]}));
    let selection = [
        source.get("modelSelection"),
        project.get("defaultModelSelection"),
        defaults.get("defaultModelSelection"),
        sibling.and_then(|s| s.get("modelSelection")),
        available.as_ref(),
        v["draft"].get("modelSelection"),
    ]
    .into_iter()
    .flatten()
    .find(|s| s.is_object())
    .cloned();
    let mode = [
        source.get("runtimeMode"),
        defaults.get("defaultRuntimeMode"),
        sibling.and_then(|s| s.get("runtimeMode")),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .find(|m| crate::models::valid_runtime_mode(m))
    .unwrap_or("approval-required");
    let mut missing = vec![];
    if selection.is_none() {
        missing.push("model")
    }
    if v["automatic"] == true {
        missing.push("host_route")
    }
    let mut strategy = v.get("workspaceStrategy").cloned();
    if strategy.is_none() {
        match project["defaultThreadEnvMode"].as_str() {
            Some("local") => strategy = Some(json!({"type":"root"})),
            Some("worktree") => missing.push("base_ref"),
            _ => missing.push("workspace_confirmation"),
        }
    }
    let mut result = json!({"projects":visible,"project":project,"modelSelection":selection,"runtimeMode":mode,"workspaceStrategy":strategy,"missing":missing,"ready":missing.is_empty()});
    if v["build"] == true && missing.is_empty() {
        crate::api::ids::id("threadId", text(v, "threadId"))?;
        let mut args = json!({"commandId":v["commandId"],"threadId":v["threadId"],"projectId":project["id"],"title":v["title"],"modelSelection":selection,"runtimeMode":mode,"interactionMode":v["interactionMode"].as_str().unwrap_or("default"),"workspaceStrategy":strategy});
        if let Some(message) = v.get("initialMessage") {
            args["initialMessage"] = message.clone();
        }
        let payload = crate::api::orchestration::LAUNCH_THREAD.payload(&args)?;
        result["request"] = json!({"fn":"orchestration.launchThread","args":payload});
    }
    Ok(result)
}
