//! Read-only native Pierre review snapshots. Documents receive data, never an RPC bridge.
use crate::{json, text, Environment, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::Read,
    sync::{Mutex, OnceLock},
};
static SNAPSHOTS: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
fn snapshots() -> &'static Mutex<HashMap<String, Value>> {
    SNAPSHOTS.get_or_init(Default::default)
}
pub(crate) fn has_text_patch(file: &Value) -> bool {
    file["binary"] != true
        && file["missing"] != true
        && file["rows"]
            .as_array()
            .is_some_and(|rs| rs.iter().any(|r| r["kind"] == "hunk"))
}
pub(crate) fn remember(
    scope: &Value,
    source: &Value,
    payload: &Value,
    file: &Value,
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut cache = snapshots().lock().map_err(|_| "local_error")?;
    if cache.len() >= 8 {
        cache.clear();
    }
    let mut file = file.clone();
    if let Some(meta) = file.as_object_mut() {
        meta.remove("rows");
        meta.remove("splitRows");
    }
    cache.insert(
        id.clone(),
        json!({"scope":scope,"source":source,"payload":payload,"file":file}),
    );
    Ok(id)
}
pub(crate) fn remember_checkpoint(
    environment: &str,
    thread: &str,
    tag: &str,
    payload: &Value,
    result: &Value,
    files: &[Value],
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let metadata = files
        .iter()
        .filter(|f| has_text_patch(f))
        .map(|f| {
            let mut f = f.clone();
            let m = f.as_object_mut().unwrap();
            m.remove("rows");
            m.remove("splitRows");
            f
        })
        .collect::<Vec<_>>();
    let mut cache = snapshots().lock().map_err(|_| "local_error")?;
    if cache.len() >= 8 {
        cache.clear();
    }
    cache.insert(id.clone(),json!({"kind":"checkpoint","scope":{"environment":environment,"thread":thread,"fromTurnCount":result["fromTurnCount"],"toTurnCount":result["toTurnCount"]},"source":{"diff":result["diff"],"diffHash":format!("{:x}",Sha256::digest(text(result,"diff")))},"tag":tag,"payload":payload,"files":metadata}));
    Ok(id)
}
fn open_checkpoint(e: &Environment, input: &Value, snapshot: &Value) -> Result<Value> {
    crate::working_review::authorize(e, "orchestration:read")?;
    let path = input["filePath"].as_str().ok_or("review_file_changed")?;
    let file = snapshot["files"]
        .as_array()
        .and_then(|fs| fs.iter().find(|f| f["path"] == path))
        .ok_or("review_file_changed")?;
    let c = crate::working_review::connection(e)?;
    let fresh = c.rpc(text(snapshot, "tag"), snapshot["payload"].clone())?;
    let scope = &snapshot["scope"];
    if fresh["threadId"] != scope["thread"]
        || fresh["fromTurnCount"] != scope["fromTurnCount"]
        || fresh["toTurnCount"] != scope["toTurnCount"]
    {
        return Err("invalid_diff_range".into());
    }
    let patch = fresh["diff"].as_str().ok_or("invalid_diff_result")?;
    if format!("{:x}", Sha256::digest(patch)) != snapshot["source"]["diffHash"] {
        return Err("review_file_changed".into());
    }
    let mut selected = snapshot.clone();
    selected["file"] = file.clone();
    // Native DiffPanel supplies no full-file loader for historical checkpoints.
    render(&selected, None, "", input)
}
fn bundle() -> Result<String> {
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(&include_bytes!("../ui/document/diff-renderer.bundle.js.gz")[..])
        .take(12 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "viewer_unavailable")?;
    if bytes.len() > 12 * 1024 * 1024 {
        return Err("viewer_unavailable".into());
    }
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}
pub fn render(
    snapshot: &Value,
    full: Option<Value>,
    full_error: &str,
    input: &Value,
) -> Result<Value> {
    let file = &snapshot["file"];
    let layout = match input
        .get("layout")
        .and_then(Value::as_str)
        .unwrap_or("stacked")
    {
        "stacked" => "unified",
        "split" => "split",
        _ => return Err("invalid_diff_option".into()),
    };
    let wrap = input.get("wrap").and_then(Value::as_bool).unwrap_or(true);
    let appearance = if input["theme"]["appearance"] == "light" {
        "light"
    } else {
        "dark"
    };
    let config=json!({"patch":snapshot["source"]["diff"],"cacheKey":snapshot["source"]["diffHash"],"path":file["path"],"oldPath":file["oldPath"],"appearance":appearance,"layout":layout,"wrap":wrap,"full":full,"canLoadContext":snapshot["kind"]!="checkpoint","fullError":full_error}).to_string().replace('<',"\\u003c");
    let html=format!("<!doctype html><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline' data:; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; frame-src 'none'; worker-src 'none'; form-action 'none'\"><style>html,body{{margin:0;font:13px monospace}}diffs-container{{display:block}}#error{{white-space:pre-wrap;color:var(--destructive)}}</style><div id=\"error\"></div><diffs-container></diffs-container><script>window.__t3Diff={config};</script><script src=\"data:text/javascript;base64,{}\"></script>",bundle()?);
    Ok(
        json!({"html":true,"markdown":false,"internalReview":true,"title":file["path"],"contents":html,"baseUrl":"about:blank","theme":input["theme"],"contextError":full_error,"labels":input["labels"]}),
    )
}
pub fn open(e: &Environment, input: &Value) -> Result<Value> {
    let id = text(input, "reviewSnapshotId");
    uuid::Uuid::parse_str(id).map_err(|_| "review_file_changed")?;
    let snapshot = snapshots()
        .lock()
        .map_err(|_| "local_error")?
        .get(id)
        .cloned()
        .ok_or("review_snapshot_expired")?;
    let scope = &snapshot["scope"];
    if scope["environment"] != e.id
        || input["environment"] != e.id
        || input["thread"] != scope["thread"]
    {
        return Err("git_scope_changed".into());
    }
    if snapshot["kind"] == "checkpoint" {
        return open_checkpoint(e, input, &snapshot);
    }
    if input
        .get("filePath")
        .is_some_and(|p| p != &snapshot["file"]["path"])
    {
        return Err("review_file_changed".into());
    }
    crate::working_review::authorize(e, "filesystem:read")?;
    let c = crate::working_review::connection(e)?;
    if crate::working_review::scope(&c, scope)? != *scope {
        return Err("git_scope_changed".into());
    }
    let file = &snapshot["file"];
    let source = &snapshot["source"];
    let kind = match text(file, "kind") {
        "added" => "new",
        "deleted" => "deleted",
        "renamed" => {
            if file["additions"] == 0 && file["deletions"] == 0 {
                "rename-pure"
            } else {
                "rename-changed"
            }
        }
        _ => "change",
    };
    let mut full = None;
    let mut error = String::new();
    if !matches!(kind, "new" | "deleted") {
        match c.rpc("review.getDiffFileContents",json!({"cwd":scope["cwd"],"sourceKind":source["kind"],"baseRef":source["baseRef"],"headRef":source["headRef"],"changeType":kind,"oldPath":file["oldPath"],"newPath":file["path"]})){
   Ok(v) if v["oldContents"].as_str().is_some_and(|s|s.len()<=1024*1024)&&v["newContents"].as_str().is_some_and(|s|s.len()<=1024*1024)=>full=Some(v),
   Ok(_)=>error="review_context_limit".into(),Err(e)=>error=e
  }
    }
    let fresh = c.rpc("review.getDiffPreview", snapshot["payload"].clone())?;
    let fresh_source = fresh["sources"]
        .as_array()
        .and_then(|ss| ss.iter().find(|s| s["kind"] == source["kind"]))
        .ok_or("review_file_changed")?;
    if fresh["cwd"] != scope["cwd"]
        || ["diffHash", "baseRef", "headRef"]
            .iter()
            .any(|k| fresh_source[k] != source[k])
        || crate::working_review::scope(&c, scope)? != *scope
    {
        return Err("review_file_changed".into());
    }
    render(&snapshot, full, &error, input)
}
