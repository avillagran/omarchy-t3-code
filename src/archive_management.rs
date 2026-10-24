//! Native archived-thread screen. Reads/mutations stay on the row's original environment.
use crate::{json, protocol, read_json, save_json, text, valid_id, xdg, Environment, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
static VIEWS: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
fn views() -> &'static Mutex<HashMap<String, Value>> {
    VIEWS.get_or_init(Default::default)
}
fn permitted(c: &protocol::Connection, required: &str) -> Result<bool> {
    let s = c.request_json("GET", "/api/auth/session", None, false)?;
    Ok(s["authenticated"] == true
        && s.get("permissions")
            .or_else(|| s.get("scopes"))
            .and_then(Value::as_array)
            .is_some_and(|p| p.iter().any(|p| p == required)))
}
fn authorize(c: &protocol::Connection, required: &str) -> Result<()> {
    if permitted(c, required)? {
        Ok(())
    } else {
        Err("permission_denied".into())
    }
}
fn project_ids(input: &Value) -> Result<Option<Vec<String>>> {
    if input
        .get("projectIds")
        .ok_or("invalid_archive_scope")?
        .is_null()
    {
        return Ok(None);
    }
    let ids = input["projectIds"]
        .as_array()
        .filter(|a| a.len() <= 512)
        .ok_or("invalid_archive_scope")?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for id in ids {
        let id = id
            .as_str()
            .filter(|s| valid_id(s))
            .ok_or("invalid_archive_scope")?;
        if seen.insert(id) {
            out.push(id.into());
        }
    }
    Ok(Some(out))
}
fn includes(ids: &Option<Vec<String>>, project: &str) -> bool {
    ids.as_ref()
        .is_none_or(|ids| ids.iter().any(|p| p == project))
}
fn revision(row: &Value) -> String {
    format!("{:x}",Sha256::digest(json!({"id":row["id"],"projectId":row["projectId"],"title":row["title"],"archivedAt":row["archivedAt"],"createdAt":row["createdAt"]}).to_string()))
}
pub fn present(raw: &Value, environment: &str, ids: Option<Vec<String>>) -> Result<Value> {
    if raw["schemaVersion"].as_u64().is_none_or(|n| n == 0)
        || raw["snapshotSequence"].as_u64().is_none()
    {
        return Err("invalid_archive_snapshot".into());
    }
    let projects = raw["projects"]
        .as_array()
        .filter(|p| p.len() <= 10000)
        .ok_or("archive_limit")?;
    let threads = raw["threads"]
        .as_array()
        .filter(|p| p.len() <= 10000)
        .ok_or("archive_limit")?;
    let mut seen = HashSet::new();
    for t in threads {
        if !valid_id(text(t, "id"))
            || !valid_id(text(t, "projectId"))
            || !seen.insert(text(t, "id"))
            || t["archivedAt"].as_str().filter(|s| !s.is_empty()).is_none()
            || !t["deletedAt"].is_null()
            || t["title"]
                .as_str()
                .is_none_or(|s| s.len() > 4096 || s.contains('\0'))
        {
            return Err("invalid_archive_snapshot".into());
        }
    }
    let mut groups = Vec::new();
    let mut project_keys = HashSet::new();
    for p in projects {
        let id = text(p, "id");
        if !valid_id(id) || !project_keys.insert(id) {
            return Err("invalid_archive_snapshot".into());
        }
        if !includes(&ids, id) {
            continue;
        }
        let mut rows=threads.iter().filter(|t|t["projectId"]==id).map(|t|json!({"id":t["id"],"projectId":id,"environment":environment,"title":t["title"],"archivedAt":t["archivedAt"],"createdAt":t["createdAt"],"revision":revision(t)})).collect::<Vec<_>>();
        rows.sort_by(|a, b| {
            text(b, "archivedAt")
                .cmp(text(a, "archivedAt"))
                .then_with(|| text(b, "id").cmp(text(a, "id")))
        });
        if !rows.is_empty() {
            groups.push(json!({"project":{"id":p["id"],"title":p["title"],"workspaceRoot":p["workspaceRoot"],"projectIcon":p["projectIcon"]},"environment":environment,"threads":rows}));
        }
    }
    Ok(json!({"groups":groups,"snapshotSequence":raw["snapshotSequence"]}))
}
fn action_path(id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id).map_err(|_| "invalid_request_id")?;
    Ok(xdg("XDG_STATE_HOME", ".local/state")
        .join("outbox/archive-actions")
        .join(format!("{id}.json")))
}
fn thread_path(environment: &str, origin: &str, thread: &str) -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state")
        .join("outbox/archive-threads")
        .join(format!(
            "{:x}.json",
            Sha256::digest(format!("{environment}\0{origin}\0{thread}"))
        ))
}
fn blocking(r: &Value) -> bool {
    matches!(text(r, "state"), "pending" | "accepted" | "uncertain")
}
fn receipt(r: &Value) -> Value {
    json!({"actionId":r["actionId"],"durable":true,"originKey":format!("{:x}",Sha256::digest(text(r,"origin"))),"environment":r["environment"],"threadId":r["threadId"],"projectId":r["projectId"],"title":r["title"],"operation":r["operation"],"state":r["state"],"error":r["error"]})
}
fn save(r: &Value, admit: bool) -> Result<()> {
    save_json(&action_path(text(r, "actionId"))?, r)?;
    let latest = thread_path(
        text(r, "environment"),
        text(r, "origin"),
        text(r, "threadId"),
    );
    if admit || !latest.exists() || read_json(&latest)?["actionId"] == r["actionId"] {
        save_json(&latest, r)?;
    }
    Ok(())
}
fn matches(e: &Environment, r: &Value) -> Result<()> {
    if r["environment"] != e.id || r["origin"] != e.base_url {
        return Err("archive_scope_changed".into());
    }
    Ok(())
}
fn raw(c: &protocol::Connection) -> Result<Value> {
    let v = c.rpc("orchestration.getArchivedShellSnapshot", json!({}))?;
    present(&v, "validation", None)?;
    Ok(v)
}
fn archived<'a>(snapshot: &'a Value, id: &str) -> Option<&'a Value> {
    snapshot["threads"]
        .as_array()?
        .iter()
        .find(|t| t["id"] == id && !t["archivedAt"].is_null() && t["deletedAt"].is_null())
}
fn observe_with(c: &protocol::Connection, r: &mut Value, archive: &Value) -> Result<()> {
    let main = c.request_json("GET", "/api/orchestration/shell", None, true)?;
    if main["schemaVersion"].as_u64().is_none_or(|n| n == 0)
        || main["snapshotSequence"].as_u64().is_none()
    {
        return Err("invalid_archive_snapshot".into());
    }
    let threads = main["threads"]
        .as_array()
        .ok_or("invalid_archive_snapshot")?;
    let archived_threads = main["archivedThreads"]
        .as_array()
        .ok_or("invalid_archive_snapshot")?;
    // Acknowledgement is not completion; both read models must have caught up.
    let minimum = r["sequence"].as_u64().unwrap_or(0);
    if main["snapshotSequence"].as_u64().unwrap_or(0) < minimum
        || archive["snapshotSequence"].as_u64().unwrap_or(0) < minimum
    {
        return Ok(());
    }
    let archived_in_main = archived_threads
        .iter()
        .any(|t| t["id"] == r["threadId"] && t["deletedAt"].is_null());
    let t = threads
        .iter()
        .find(|t| t["id"] == r["threadId"] && t["deletedAt"].is_null());
    let in_archive = archived(archive, text(r, "threadId")).is_some();
    let done = if r["operation"] == "unarchive" {
        !in_archive
            && t.is_some_and(|t| t["archivedAt"].is_null() && t["projectId"] == r["projectId"])
    } else {
        !in_archive && !archived_in_main && t.is_none()
    };
    if done {
        r["state"] = json!("complete");
        r["error"] = json!("");
    } else if r["state"] == "pending" {
        r["state"] = json!("uncertain");
        r["error"] = json!("rpc_outcome_unknown");
    }
    Ok(())
}
pub fn read(e: &Environment, input: &Value) -> Result<Value> {
    uuid::Uuid::parse_str(text(input, "requestId")).map_err(|_| "invalid_request_id")?;
    if input["environment"] != e.id {
        return Err("archive_scope_changed".into());
    }
    let ids = project_ids(input)?;
    let c = e.connection(false)?;
    authorize(&c, "orchestration:read")?;
    let snapshot = raw(&c)?;
    let mut data = present(&snapshot, &e.id, ids.clone())?;
    data["writable"] = json!(e.connection(true)
        .and_then(|c| permitted(&c, "orchestration:operate"))
        .unwrap_or(false));
    let mut operations = Vec::new();
    let directory = xdg("XDG_STATE_HOME", ".local/state").join("outbox/archive-threads");
    if let Ok(files) = std::fs::read_dir(directory) {
        for path in files.take(4096).flatten() {
            let Ok(mut r) = read_json(&path.path()) else {
                continue;
            };
            if matches(e, &r).is_err() || !blocking(&r) || !includes(&ids, text(&r, "projectId")) {
                continue;
            }
            if let Ok(_guard) = crate::checkpoint_journal::lock(&e.id, text(&r, "threadId")) {
                r = read_json(&action_path(text(&r, "actionId"))?)?;
                if blocking(&r) {
                    if let Err(error) = observe_with(&c, &mut r, &snapshot) {
                        r["error"] = json!(error);
                    }
                }
                save(&r, false)?;
            }
            operations.push(receipt(&r));
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    data["viewId"] = json!(id);
    data["originKey"] = json!(format!("{:x}", Sha256::digest(&e.base_url)));
    data["environment"] = json!(e.id);
    data["projectIds"] = input["projectIds"].clone();
    data["requestId"] = input["requestId"].clone();
    data["operations"] = json!(operations);
    let mut cache = views().lock().map_err(|_| "local_error")?;
    if cache.len() >= 32 {
        cache.clear();
    }
    cache.insert(
        id,
        json!({"origin":e.base_url,"environment":e.id,"data":data}),
    );
    Ok(data)
}
/// Exact native commands only. Archive deletion deliberately performs no worktree cleanup.
fn action_inner(e: &Environment, input: &Value) -> Result<Value> {
    let id = text(input, "actionId");
    let path = action_path(id)?;
    let thread = text(input, "thread");
    if !valid_id(thread) || input["environment"] != e.id {
        return Err("archive_scope_changed".into());
    }
    let _guard = crate::checkpoint_journal::lock(&e.id, thread)?;
    crate::checkpoint_journal::ensure_available(&e.id, thread)?;
    let c = e.connection(true)?;
    authorize(&c, "orchestration:read")?;
    let new_action = !path.exists();
    let mut record = if !new_action {
        let mut r = read_json(&path)?;
        matches(e, &r)?;
        if r["threadId"] != thread || r["operation"] != input["operation"] {
            return Err("retry_payload_changed".into());
        }
        if !blocking(&r) {
            return Ok(receipt(&r));
        }
        let snapshot = raw(&c)?;
        observe_with(&c, &mut r, &snapshot)?;
        save(&r, false)?;
        if r["state"] == "complete" || input["retry"] != true {
            return Ok(receipt(&r));
        }
        let t = archived(&snapshot, thread).ok_or("archive_thread_changed")?;
        if revision(t) != r["revision"] {
            return Err("archive_thread_changed".into());
        }
        r
    } else {
        authorize(&c, "orchestration:operate")?;
        if !matches!(text(input, "operation"), "unarchive" | "delete") {
            return Err("invalid_archive_action".into());
        }
        if input["operation"] == "delete" && input["confirmed"] != true {
            return Err("confirmation_required".into());
        }
        let view = views()
            .lock()
            .map_err(|_| "local_error")?
            .get(text(input, "viewId"))
            .cloned()
            .ok_or("archive_snapshot_expired")?;
        if view["origin"] != e.base_url || view["environment"] != e.id {
            return Err("archive_scope_changed".into());
        }
        let expected = view["data"]["groups"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["threads"].as_array().unwrap())
            .find(|t| t["id"] == thread)
            .ok_or("archive_thread_changed")?;
        let fresh = raw(&c)?;
        let row = archived(&fresh, thread).ok_or("archive_thread_changed")?;
        if revision(row) != expected["revision"] {
            return Err("archive_thread_changed".into());
        }
        let latest = thread_path(&e.id, &e.base_url, thread);
        if latest.exists() {
            let previous = read_json(&latest)?;
            if blocking(&previous) {
                return Err("archive_operation_pending".into());
            }
        }
        json!({"actionId":id,"environment":e.id,"origin":e.base_url,"threadId":thread,"projectId":expected["projectId"],"title":expected["title"],"operation":input["operation"],"revision":expected["revision"],"state":"pending","error":"","command":{"type":if input["operation"]=="delete"{"thread.delete"}else{"thread.unarchive"},"threadId":thread,"commandId":format!("omarchy-archive-{id}")}})
    };
    authorize(&c, "orchestration:operate")?;
    record["state"] = json!("pending");
    record["error"] = json!("");
    save(&record, new_action)?;
    match crate::write::send_command(&c, &record["command"]) {
        Ok(ack) if ack["sequence"].as_u64().is_some() => {
            record["state"] = json!("accepted");
            record["sequence"] = ack["sequence"].clone();
        }
        Ok(_) => {
            record["state"] = json!("uncertain");
            record["error"] = json!("invalid_archive_ack");
        }
        Err(error) => {
            record["state"] = json!("uncertain");
            record["error"] = json!(error);
        }
    }
    save(&record, false)?;
    if let Err(error) = raw(&c).and_then(|snapshot| observe_with(&c, &mut record, &snapshot)) {
        record["error"] = json!(error);
    }
    save(&record, false)?;
    Ok(receipt(&record))
}
pub fn action(e: &Environment, input: &Value) -> Result<Value> {
    match action_inner(e, input) {
        Ok(r) => Ok(r),
        Err(error) => {
            if let Ok(path) = action_path(text(input, "actionId")) {
                if path.exists() {
                    if let Ok(r) = read_json(&path) {
                        if matches(e, &r).is_ok()
                            && r["threadId"] == input["thread"]
                            && r["operation"] == input["operation"]
                        {
                            let mut r = receipt(&r);
                            if blocking(&r) {
                                r["state"] = json!("uncertain");
                            }
                            r["error"] = json!(error);
                            return Ok(r);
                        }
                    }
                }
            }
            Err(error)
        }
    }
}
pub fn observe(e: &Environment, input: &Value) -> Result<Value> {
    let mut r = read_json(&action_path(text(input, "actionId"))?)?;
    matches(e, &r)?;
    if input["environment"] != e.id || input["thread"] != r["threadId"] {
        return Err("archive_scope_changed".into());
    }
    let c = e.connection(false)?;
    authorize(&c, "orchestration:read")?;
    if blocking(&r) {
        if let Ok(_guard) = crate::checkpoint_journal::lock(&e.id, text(&r, "threadId")) {
            r = read_json(&action_path(text(&r, "actionId"))?)?;
            if !blocking(&r) {
                return Ok(receipt(&r));
            }
            let snapshot = raw(&c)?;
            observe_with(&c, &mut r, &snapshot)?;
            save(&r, false)?;
        }
    }
    Ok(receipt(&r))
}
