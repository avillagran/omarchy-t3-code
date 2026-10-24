//! Checkout review reads use T3's VCS service, never the host filesystem or local Git.
use crate::{git_controls, json, protocol, text, Client, Environment, Result};
use serde_json::Value;
use std::collections::HashSet;

pub(crate) fn scope(c: &protocol::Connection, input: &Value) -> Result<Value> {
    git_controls::scope(
        &c.request_json("GET", "/api/orchestration/shell", None, true)?,
        input,
    )
}
fn name(v: &Value) -> Result<&str> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= 8192 && !s.contains('\0'))
        .ok_or("invalid_review_path".into())
}
fn option(input: &Value) -> Result<Value> {
    let mut p = json!({"cwd":input["cwd"],"ignoreWhitespace":input.get("ignoreWhitespace").and_then(Value::as_bool).unwrap_or(true)});
    if let Some(v) = input.get("baseRef").filter(|v| !v.is_null() && *v != "") {
        let s = name(v)?;
        if s.trim() != s {
            return Err("invalid_review_ref".into());
        }
        p["baseRef"] = v.clone();
    }
    if input
        .get("ignoreWhitespace")
        .is_some_and(|v| !v.is_boolean())
    {
        return Err("invalid_diff_option".into());
    }
    Ok(p)
}
/// Preserve complete native statistics even when the global patch was truncated. Older
/// servers without statistics fall back to the bounded existing checkpoint parser.
pub fn present(result: &Value, wanted: &str) -> Result<Value> {
    if !matches!(wanted, "working-tree" | "branch-range") {
        return Err("invalid_review_source".into());
    }
    let sources = result["sources"]
        .as_array()
        .filter(|s| s.len() <= 2)
        .ok_or("invalid_review_result")?;
    let mut seen = HashSet::new();
    for s in sources {
        if !matches!(text(s, "kind"), "working-tree" | "branch-range")
            || !seen.insert(text(s, "kind"))
        {
            return Err("invalid_review_result".into());
        }
    }
    let Some(s) = sources.iter().find(|s| s["kind"] == wanted) else {
        return Ok(json!({"files":[],"additions":0,"deletions":0,"source":null,"sources":[]}));
    };
    name(&s["diffHash"])?;
    if !s["truncated"].is_boolean() || !s["diff"].is_string() {
        return Err("invalid_review_result".into());
    }
    let mut additions = 0u64;
    let mut deletions = 0u64;
    let files = if let Some(files) = s.get("files") {
        let files = files
            .as_array()
            .filter(|fs| fs.len() <= git_controls::MAX_FILES)
            .ok_or("diff_limit_files")?;
        let mut paths = HashSet::new();
        let mut output = Vec::new();
        for f in files {
            let path = name(&f["path"])?;
            if !paths.insert(path) {
                return Err("invalid_review_result".into());
            }
            let old = if f["previousPath"].is_null() {
                path
            } else {
                name(&f["previousPath"])?
            };
            let a = f["additions"].as_u64().ok_or("invalid_review_result")?;
            let d = f["deletions"].as_u64().ok_or("invalid_review_result")?;
            additions = additions.checked_add(a).ok_or("invalid_review_result")?;
            deletions = deletions.checked_add(d).ok_or("invalid_review_result")?;
            output.push(json!({"path":path,"oldPath":old,"previousPath":f["previousPath"],"kind":if old!=path{"renamed"}else{"modified"},"additions":a,"deletions":d,"rows":[],"splitRows":[],"pending":true}));
        }
        output
    } else {
        let parsed = crate::diff_review::present(s, &json!({}))?;
        additions = parsed["additions"].as_u64().unwrap_or(0);
        deletions = parsed["deletions"].as_u64().unwrap_or(0);
        parsed["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                let mut f = f.clone();
                f["previousPath"] = if f["oldPath"] != f["path"] {
                    f["oldPath"].clone()
                } else {
                    Value::Null
                };
                f["pending"] = json!(true);
                f["rows"] = json!([]);
                f["splitRows"] = json!([]);
                f
            })
            .collect()
    };
    let metadata = |s: &Value| json!({"id":s["id"],"kind":s["kind"],"title":s["title"],"baseRef":s["baseRef"],"headRef":s["headRef"],"diffHash":s["diffHash"],"truncated":s["truncated"],"statisticsAvailable":s.get("files").is_some()});
    Ok(
        json!({"files":files,"additions":additions,"deletions":deletions,"source":metadata(s),"sources":sources.iter().map(metadata).collect::<Vec<_>>()}),
    )
}

pub(crate) fn authorize(e: &Environment, required: &str) -> Result<()> {
    let session = Client::shared()?.get(e, "/api/auth/session", false)?;
    let permissions = session.get("permissions").or_else(|| session.get("scopes"));
    if session["authenticated"] != true
        || !permissions
            .and_then(Value::as_array)
            .is_some_and(|s| s.iter().any(|v| v == required))
    {
        return Err("permission_denied".into());
    }
    Ok(())
}

/// Exact listRefs query contract used by native DiffPanel, with independent pagination
/// for local and remote refs. No local Git, refresh mutation, or checkout fallback.
pub fn refs_payload(input: &Value) -> Result<Value> {
    let kind = text(input, "refKind");
    if !matches!(kind, "local" | "remote") {
        return Err("invalid_review_ref_kind".into());
    }
    let query = input["query"]
        .as_str()
        .ok_or("invalid_review_query")?
        .trim();
    if query.encode_utf16().count() > 256 || query.contains('\0') {
        return Err("invalid_review_query".into());
    }
    let cursor = input
        .get("cursor")
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_u64()
                .filter(|n| *n <= 9_007_199_254_740_991)
                .ok_or("invalid_review_cursor")
        })
        .transpose()?;
    let mut p =
        json!({"cwd":input["cwd"],"includeMatchingRemoteRefs":true,"refKind":kind,"limit":100});
    if !query.is_empty() {
        p["query"] = json!(query);
    }
    if let Some(cursor) = cursor {
        p["cursor"] = json!(cursor);
    }
    Ok(p)
}
pub fn refs_present(raw: &Value, input: &Value) -> Result<Value> {
    let refs = raw["refs"]
        .as_array()
        .filter(|rs| rs.len() <= 100)
        .ok_or("invalid_review_refs")?;
    let mut names = HashSet::new();
    for r in refs {
        let n = name(&r["name"])?;
        if n.trim() != n
            || !names.insert(n)
            || !r["current"].is_boolean()
            || !r["isDefault"].is_boolean()
            || r.get("isRemote").is_some_and(|v| !v.is_boolean())
        {
            return Err("invalid_review_refs".into());
        }
        if !r["remoteName"].is_null() {
            name(&r["remoteName"])?;
        }
        if !r["worktreePath"].is_null() {
            name(&r["worktreePath"])?;
        }
    }
    let total = raw["totalCount"].as_u64().ok_or("invalid_review_refs")?;
    let cursor = input["cursor"].as_u64().unwrap_or(0);
    if !raw["nextCursor"].is_null()
        && raw["nextCursor"]
            .as_u64()
            .is_none_or(|n| n <= cursor || n > total || refs.is_empty())
    {
        return Err("invalid_review_cursor".into());
    }
    if !raw["isRepo"].is_boolean() || !raw["hasPrimaryRemote"].is_boolean() {
        return Err("invalid_review_refs".into());
    }
    Ok(
        json!({"refs":refs,"nextCursor":raw["nextCursor"],"totalCount":total,"isRepo":raw["isRepo"],"hasPrimaryRemote":raw["hasPrimaryRemote"],"refKind":input["refKind"],"query":text(input,"query").trim()}),
    )
}
pub fn refs(e: &Environment, input: &Value) -> Result<Value> {
    uuid::Uuid::parse_str(text(input, "requestId")).map_err(|_| "invalid_request_id")?;
    if input["environment"] != e.id {
        return Err("git_scope_changed".into());
    }
    let payload = refs_payload(input)?;
    authorize(e, "orchestration:read")?;
    let c = e.connection(false)?;
    let scoped = scope(&c, input)?;
    let mut result = refs_present(&c.rpc("vcs.listRefs", payload)?, input)?;
    if scope(&c, input)? != scoped {
        return Err("git_scope_changed".into());
    }
    result["scope"] = scoped;
    Ok(result)
}

pub fn execute(e: &Environment, input: &Value) -> Result<Value> {
    uuid::Uuid::parse_str(text(input, "requestId")).map_err(|_| "invalid_request_id")?;
    if input["environment"] != e.id {
        return Err("git_scope_changed".into());
    }
    let c = e.connection(false)?;
    authorize(e, "filesystem:read")?;
    let scoped = scope(&c, input)?;
    let mut payload = option(input)?;
    let raw = c.rpc("review.getDiffPreview", payload.clone())?;
    if raw["cwd"] != scoped["cwd"] {
        return Err("git_scope_changed".into());
    }
    let mut out = present(&raw, text(input, "sourceKind"))?;
    let mut snapshot = None;
    if let Some(path) = input.get("filePath").filter(|v| !v.is_null() && *v != "") {
        name(path)?;
        let f = out["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == *path)
            .ok_or("review_file_changed")?;
        payload["file"] =
            json!({"path":path,"previousPath":f["previousPath"],"sourceKind":input["sourceKind"]});
        let result = c.rpc("review.getDiffPreview", payload.clone())?;
        if result["cwd"] != scoped["cwd"] {
            return Err("git_scope_changed".into());
        }
        let source = result["sources"]
            .as_array()
            .and_then(|ss| ss.iter().find(|s| s["kind"] == input["sourceKind"]))
            .ok_or("invalid_review_result")?;
        let parsed = crate::diff_review::present(source, &json!({}))?;
        let file = parsed["files"]
            .as_array()
            .and_then(|fs| fs.iter().find(|f| f["path"] == *path))
            .cloned();
        out["file"]=file.unwrap_or_else(||json!({"path":path,"oldPath":f["oldPath"],"kind":f["kind"],"missing":true,"rows":[],"splitRows":[],"additions":f["additions"],"deletions":f["deletions"]}));
        out["file"]["truncated"] = source["truncated"].clone();
        if source["truncated"] == false && crate::review_document::has_text_patch(&out["file"]) {
            snapshot = Some((source.clone(), payload));
        }
    }
    // A checkout handoff during an RPC cannot display another workspace's result.
    if scope(&c, input)? != scoped {
        return Err("git_scope_changed".into());
    }
    if let Some((source, payload)) = snapshot {
        out["file"]["reviewSnapshotId"] = json!(crate::review_document::remember(
            &scoped,
            &source,
            &payload,
            &out["file"]
        )?);
    }
    out["scope"] = scoped;
    Ok(out)
}
