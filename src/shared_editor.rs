//! Offline editor branches. Wire documents contain only protocol-v1 fields.
use crate::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};

fn directory() -> Result<PathBuf> {
    let p = xdg("XDG_STATE_HOME", ".local/state").join("shared-editor");
    fs::create_dir_all(p.parent().unwrap()).map_err(|_| "draft_save_failed")?;
    match fs::DirBuilder::new().mode(0o700).create(&p) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(_) => return Err("draft_save_failed".into()),
    }
    let m = fs::symlink_metadata(&p).map_err(|_| "draft_read_failed")?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.permissions().mode() & 0o077 != 0 {
        return Err("shared_drafts_private_state_required".into());
    }
    Ok(p)
}
fn path(authority: &str, key: &str) -> Result<PathBuf> {
    Ok(directory()?.join(format!(
        "{:x}.json",
        Sha256::digest(format!("{authority}/{key}"))
    )))
}
fn locked() -> Result<fs::File> {
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(directory()?.join("editor.lock"))
        .map_err(|_| "draft_save_failed")?;
    let m = f.metadata().map_err(|_| "draft_read_failed")?;
    if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.permissions().mode() & 0o077 != 0
    {
        return Err("shared_drafts_private_state_required".into());
    }
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("shared_drafts_busy".into());
    }
    Ok(f)
}
fn read(p: &Path) -> Result<Value> {
    use std::io::Read;
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(p)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(_) => return Err("shared_drafts_private_state_required".into()),
    };
    let m = file.metadata().map_err(|_| "draft_read_failed")?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.permissions().mode() & 0o077 != 0
        || m.len() > 2 * 1024 * 1024
    {
        return Err("shared_drafts_private_state_required".into());
    }
    let mut bytes = vec![];
    file.take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "draft_read_failed")?;
    serde_json::from_slice(&bytes).map_err(|_| "draft_read_failed".into())
}

/// Lossless or fail closed: pending uploads, local recovery and unsupported
/// provider options never become a truncated shared payload.
pub fn payload(d: &Value, environment: &str) -> Result<Value> {
    if d["queueEdit"].is_object() || d["attempt"].is_object() {
        return Err("draft_locked".into());
    }
    let mut p = json!({"schema":1,"text":d["text"].as_str().unwrap_or(""),"attachments":[]});
    for f in d["attachments"].as_array().into_iter().flatten() {
        let a = &f["attachment"];
        if f["uploading"] == true || f["error"].is_string() || !a["id"].is_string() {
            return Err("draft_locked".into());
        }
        p["attachments"].as_array_mut().unwrap().push(json!({"id":a["id"],"name":a["name"].as_str().or(f["name"].as_str()).unwrap_or(""),"sourceEnvironment":environment}));
    }
    let m = &d["modelSelection"];
    if m.is_object() {
        let mut selection = json!({});
        if let Some(s) = m["instanceId"].as_str() {
            selection["providerId"] = json!(s)
        }
        if let Some(s) = m["model"].as_str() {
            selection["modelId"] = json!(s)
        }
        for o in m["options"].as_array().into_iter().flatten() {
            let k = text(o, "id");
            if !["effort", "reasoningEffort", "permissionMode", "speedMode"].contains(&k)
                || !(o["value"].is_string() || o["value"].is_boolean() || o["value"].is_number())
            {
                return Err("shared_draft_options_unsupported".into());
            }
            selection[k] = o["value"].clone()
        }
        p["modelSelection"] = selection;
    }
    p["preferences"] = json!({"deliveryMode":if d["delivery"]=="queue"{"queue"}else{"send"},"interactionMode":d["interactionMode"].as_str().unwrap_or("default")});
    if let Some(mode) = d["runtimeMode"].as_str() {
        p["preferences"]["permissionMode"] = json!(mode);
    }
    if p.to_string().len() > 8192 || p["attachments"].as_array().unwrap().len() > 16 {
        return Err("draft_too_large".into());
    }
    Ok(p)
}
fn local(p: &Value, d: &Value) -> Value {
    let mut d = d.clone();
    d["text"] = p["text"].clone();
    d["attachments"]=json!(p["attachments"].as_array().into_iter().flatten().map(|a|d["attachments"].as_array().into_iter().flatten().find(|f|f["attachment"]["id"]==a["id"]&&f["attachment"]["name"]==a["name"]&&f["error"].is_null()).cloned().unwrap_or_else(||json!({"name":a["name"],"retained":true,"error":"shared_attachment_verification_required","attachment":{"id":a["id"],"name":a["name"]}}))).collect::<Vec<_>>());
    d["delivery"] = json!(if p["preferences"]["deliveryMode"] == "queue" {
        "queue"
    } else {
        "auto"
    });
    d["interactionMode"] = p["preferences"]["interactionMode"].clone();
    d["runtimeMode"] = p["preferences"]["permissionMode"].clone();
    if p["modelSelection"].is_object() {
        let m = &p["modelSelection"];
        d["modelSelection"] = json!({"instanceId":m["providerId"],"model":m["modelId"],"options":m.as_object().unwrap().iter().filter(|(k,_)|!["providerId","modelId"].contains(&k.as_str())).map(|(k,v)|json!({"id":k,"value":v})).collect::<Vec<_>>()});
    } else {
        d["modelSelection"] = Value::Null;
    }
    d
}

/// Called on every successful local save, without network I/O. Retain the old
/// base/pending command even when the editor advances during an uncertain send.
pub fn record(authority: &str, key: &str, d: &Value) -> Result<()> {
    if authority.is_empty() {
        return Ok(());
    }
    let _lock = locked()?;
    let file = path(authority, key)?;
    let mut s = read(&file)?;
    if s["draft"] != *d {
        s["generation"] = json!(s["generation"].as_u64().unwrap_or(0) + 1);
        s["draft"] = d.clone();
        s["key"] = json!(key);
        s["authority"] = json!(authority);
        save_json(&file, &s)?;
    }
    Ok(())
}
fn journal(authority: &str, action: &str, id: &str, cmd: &Value) -> Result<Value> {
    shared_drafts::execute(
        &json!({"authority":authority,"action":action,"journalId":id,"command":cmd}),
    )
}
fn revision(snapshot: &Value, scope: &Value) -> Value {
    snapshot["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|d| d["scope"] == *scope)
        .cloned()
        .unwrap_or(Value::Null)
}
fn stage(s: &mut Value, authority: &str, cmd: Value) -> Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    journal(authority, "stage", &id, &cmd)?;
    s["pending"] = json!({"id":id,"command":cmd,"generation":s["generation"],"payload":payload(&s["draft"],text(&cmd["scope"],"environment"))?});
    Ok(())
}
fn mutation(cursor: &Value) -> Result<String> {
    let c = cursor
        .as_str()
        .and_then(|s| s.rsplit_once(':'))
        .ok_or("shared_drafts_invalid_snapshot")?;
    Ok(format!("{}.{}", c.1, uuid::Uuid::new_v4()))
}

pub fn sync(authority: &str, key: &str, scope: &Value, resolution: &str) -> Result<Value> {
    // Network never holds the editor lock: autosave remains available offline.
    let file = path(authority, key)?;
    let mut s = {
        let _l = locked()?;
        read(&file)?
    };
    if !s["draft"].is_object() {
        return Err("draft_not_found".into());
    }
    if s["scope"].is_object() && s["scope"] != *scope {
        return Err("shared_drafts_scope_changed".into());
    }
    let generation = s["generation"].clone();
    let draft = s["draft"].clone();
    let mut fresh = shared_drafts::execute(&json!({"action":"snapshot","authority":authority}))?;
    if fresh.get("error").is_some() {
        return Ok(json!({"error":fresh["error"]}));
    }
    let mut remote = revision(&fresh, scope);
    let mut remote_payload = remote["payload"].clone();
    let p = payload(&draft, text(scope, "environment"))?;
    if !resolution.is_empty() {
        if !["local", "remote"].contains(&resolution) {
            return Err("invalid_shared_draft_action".into());
        }
        // Archive the original branch before explicit resolution, including its
        // pending identity. Never replace the original journal.
        save_json(
            &directory()?.join(format!("resolved-{}.json", uuid::Uuid::new_v4())),
            &s,
        )?;
        s["pending"] = Value::Null;
        s["conflict"] = json!(false);
        s["base"] = remote.clone();
        s["cursor"] = fresh["cursor"].clone();
        if resolution == "remote" {
            s["draft"] = if remote_payload.is_object() {
                local(&remote_payload, &draft)
            } else {
                local(&json!({"text":"","attachments":[]}), &draft)
            };
            s["synced"] = payload(&s["draft"], text(scope, "environment"))?;
        }
    }
    if s["pending"].is_object() && s["conflict"] != true {
        let out = journal(authority, "retry", text(&s["pending"], "id"), &Value::Null)?;
        if out["conflict"] == true || out["outcome"]["error"].is_string() {
            s["conflict"] = json!(true)
        } else if out["outcome"]["status"] == "applied" {
            s["base"] = json!({"revision":out["outcome"]["event"]["revision"]});
            s["synced"] = if s["pending"]["command"]["op"] == "delete" {
                s["pending"]["payload"].clone()
            } else {
                s["pending"]["command"]["payload"].clone()
            };
            s["pending"] = Value::Null;
            fresh = shared_drafts::execute(&json!({"action":"snapshot","authority":authority}))?;
            if fresh.get("error").is_some() {
                return Err("shared_drafts_unavailable".into());
            }
            remote = revision(&fresh, scope);
            remote_payload = remote["payload"].clone();
        }
    }
    if s["pending"].is_null() && s["conflict"] != true && resolution != "remote" {
        if s["cursor"].is_null() {
            // First import is create-if-absent. A live remote branch is a
            // conflict even if its wall clock looks older than the local file.
            if !remote.is_null()
                && remote_payload != p
                && remote_payload.is_object()
                && (text(&draft, "text").is_empty()
                    && draft["attachments"].as_array().is_none_or(Vec::is_empty))
            {
                s["draft"] = local(&remote_payload, &draft);
                s["synced"] = remote_payload.clone();
                s["base"] = remote.clone();
            } else if !remote.is_null() && remote_payload != p {
                s["conflict"] = json!(true)
            } else if remote_payload == p {
                s["synced"] = p.clone();
                s["base"] = remote.clone();
            }
            s["cursor"] = fresh["cursor"].clone();
        } else if remote["revision"] != s["base"]["revision"] {
            if s["synced"] == p
                && text(&draft, "text").is_empty()
                && draft["attachments"].as_array().is_none_or(Vec::is_empty)
            {
                s["draft"] = if remote_payload.is_null() {
                    draft.clone()
                } else {
                    local(&remote_payload, &draft)
                };
                s["synced"] = if remote_payload.is_null() {
                    p.clone()
                } else {
                    remote_payload.clone()
                };
                s["base"] = remote.clone();
            } else {
                s["conflict"] = json!(true)
            }
        }
        if s["conflict"] != true && s["synced"] != p && s["draft"] == draft {
            // Stage with the ORIGINAL completed cursor; compaction must block
            // replay, even when a formerly absent key is still absent.
            let cursor = s["cursor"].clone();
            let empty = text(&p, "text").is_empty()
                && p["attachments"].as_array().is_some_and(Vec::is_empty);
            let mut cmd = json!({"op":if empty&&!s["base"]["revision"].is_null(){"delete"}else{"put"},"scope":scope,"expected":s["base"]["revision"],"baseCursor":cursor,"mutationId":mutation(&cursor)?});
            if cmd["op"] == "put" {
                cmd["payload"] = p.clone();
            }
            if empty && s["base"]["revision"].is_null() {
                s["synced"] = p.clone();
            } else {
                stage(&mut s, authority, cmd)?;
            }
        }
    }
    if s["pending"].is_null() && s["conflict"] != true && s["synced"] == p {
        s["cursor"] = fresh["cursor"].clone();
    }
    s["remote"] = remote;
    s["scope"] = scope.clone();
    // Preserve edits made during network I/O; only bookkeeping belongs to the
    // earlier generation. A conflict never changes the active editor.
    let _l = locked()?;
    let current = read(&file)?;
    if current["generation"] != generation {
        if s["draft"] != draft {
            s["base"] = current["base"].clone();
            s["synced"] = current["synced"].clone();
            s["conflict"] = json!(true);
        }
        s["draft"] = current["draft"].clone();
        s["generation"] = current["generation"].clone();
    }
    if !key.starts_with("stash/") && s["draft"] != draft {
        drafts::update(key, |old| {
            if *old != draft {
                return Err("draft_changed".into());
            }
            Ok(s["draft"].clone())
        })?;
    }
    save_json(&file, &s)?;
    Ok(
        json!({"draft":s["draft"],"generation":s["generation"],"conflict":s["conflict"]==true,"remote":s["remote"],"pending":s["pending"].is_object(),"scope":scope}),
    )
}

/// Import one legacy row using its original identity; the old file is retained.
pub fn import(authority: &str, key: &str, scope: &Value, draft: &Value) -> Result<Value> {
    let exists = read(&path(authority, key)?)?["draft"].is_object();
    if !exists {
        record(authority, key, draft)?;
    }
    let first = sync(authority, key, scope, "")?;
    if first["pending"] == true {
        sync(authority, key, scope, "")
    } else {
        Ok(first)
    }
}
/// Atomic stash/restore/delete. Request UUID doubles as the durable journal ID,
/// so an uncertain transfer is retried without reconstructing its command.
pub fn project(authority: &str, cmd: &Value, environment: &str) -> Result<Value> {
    let project = &cmd["projectId"];
    let key = text(cmd, "key");
    let id = key.split_once('/').ok_or("invalid_draft_key")?.1;
    let scope = |kind: &str, id: &str| json!({"environment":environment,"project":project,"kind":kind,"id":id});
    let current = scope("thread", id);
    let request = text(cmd, "requestId");
    let transfer_file = path(authority, &format!("transfer/{request}"))?;
    let mut transfer = read(&transfer_file)?;
    let mut snapshot = shared_drafts::execute(&json!({"action":"snapshot","authority":authority}))?;
    if snapshot.get("error").is_some() {
        return Err("shared_drafts_unavailable".into());
    }
    if text(cmd, "action") != "list" {
        let old = journal(authority, "status", request, &Value::Null)?;
        if old["command"].is_null() {
            let source = match text(cmd, "action") {
                "stash" => current.clone(),
                "restore" | "delete" => {
                    if let Some(key) = text(cmd, "id").strip_prefix("thread:") {
                        scope("thread", key.split_once('/').ok_or("invalid_draft_key")?.1)
                    } else {
                        scope("stash", text(cmd, "id"))
                    }
                }
                _ => return Err("invalid_shared_draft_action".into()),
            };
            let source_doc = revision(&snapshot, &source);
            if source_doc.is_null() || source_doc["deleted"] == true {
                return Err("draft_not_found".into());
            }
            // Refuse moving a branch that hasn't reached the authority.
            if source == current
                && payload(&drafts::load(key)?, environment)? != source_doc["payload"]
            {
                return Err("draft_changed".into());
            }
            let cursor = &snapshot["cursor"];
            let mut command = json!({"op":if cmd["action"]=="delete"{"delete"}else{"transfer"},"scope":source,"expected":source_doc["revision"],"baseCursor":cursor,"mutationId":mutation(cursor)?});
            if command["op"] == "transfer" {
                let target = if cmd["action"] == "stash" {
                    scope("stash", request)
                } else {
                    current.clone()
                };
                let target_doc = revision(&snapshot, &target);
                command["target"] = target;
                command["targetExpected"] = target_doc["revision"].clone();
                if target_doc["deleted"] != true && !target_doc.is_null() {
                    command["backup"] = scope("stash", &uuid::Uuid::new_v4().to_string());
                }
            }
            // Persist local branch guards before dispatch too. A remote CAS
            // cannot protect edits in this host's offline cache.
            let alias = key.split_once('/').unwrap().0;
            let mut before = json!({});
            for sc in [&command["scope"], &command["target"]] {
                if sc["kind"] == "thread" {
                    let k = format!("{alias}/{}", text(sc, "id"));
                    let d = drafts::load(&k)?;
                    if d["attempt"].is_object() || d["queueEdit"].is_object() {
                        return Err("draft_locked".into());
                    }
                    let local_payload = payload(&d, environment)?;
                    let remote_doc = revision(&snapshot, sc);
                    if remote_doc["payload"].is_object() {
                        if local_payload != remote_doc["payload"] {
                            return Err("draft_changed".into());
                        }
                    } else if !text(&d, "text").is_empty()
                        || d["attachments"].as_array().is_some_and(|a| !a.is_empty())
                    {
                        return Err("draft_changed".into());
                    }
                    before[&k] = d;
                }
            }
            transfer = json!({"before":before});
            save_json(&transfer_file, &transfer)?;
            journal(authority, "stage", request, &command)?;
        }
        let result = journal(authority, "retry", request, &Value::Null)?;
        if result["conflict"] == true {
            return Err("shared_draft_conflict".into());
        }
        if result["outcome"]["status"] != "applied" {
            return Err("shared_drafts_unavailable".into());
        }
        snapshot = shared_drafts::execute(&json!({"action":"snapshot","authority":authority}))?;
    }
    let mut changes = json!({});
    let mut entries = vec![];
    for doc in snapshot["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["scope"]["environment"] == environment && d["scope"]["project"] == *project)
    {
        let sc = &doc["scope"];
        let live = doc["deleted"] != true;
        let p = &doc["payload"];
        if sc["kind"] == "stash" {
            if text(cmd, "action") != "list" {
                let journal = journal(authority, "status", request, &Value::Null)?;
                if journal["command"]["scope"] == *sc {
                    let alias = key.split_once('/').unwrap().0;
                    let cache_key = format!("stash/{alias}/{}", text(sc, "id"));
                    let _l = locked()?;
                    let cache_file = path(authority, &cache_key)?;
                    let mut cached = read(&cache_file)?;
                    if cached["draft"].is_object() {
                        cached["draft"]["text"] = json!("");
                        cached["draft"]["attachments"] = json!([]);
                        cached["base"] = doc.clone();
                        cached["cursor"] = snapshot["cursor"].clone();
                        cached["synced"] = payload(&cached["draft"], environment)?;
                        cached["pending"] = Value::Null;
                        save_json(&cache_file, &cached)?;
                    }
                }
            }
            if live {
                entries.push(json!({"id":sc["id"],"prompt":p["text"],"attachmentCount":p["attachments"].as_array().map_or(0,Vec::len),"locked":false}));
            }
            continue;
        }
        let alias = key.split_once('/').unwrap().0;
        let k = format!("{alias}/{}", text(sc, "id"));
        if live
            && (!text(p, "text").is_empty()
                || p["attachments"].as_array().is_some_and(|a| !a.is_empty()))
        {
            entries.push(json!({"id":format!("thread:{k}"),"sourceKey":k,"prompt":p["text"],"attachmentCount":p["attachments"].as_array().map_or(0,Vec::len),"locked":false}));
        }
        if text(cmd, "action") != "list" {
            let old = journal(authority, "status", request, &Value::Null)?;
            let command = &old["command"];
            if *sc == command["scope"] || *sc == command["target"] {
                let d = drafts::load(&k)?;
                let next = if live {
                    local(p, &d)
                } else {
                    let mut next = d.clone();
                    next["text"] = json!("");
                    next["attachments"] = json!([]);
                    next
                };
                let expected = &transfer["before"][&k];
                drafts::update(&k, |now| {
                    if now != expected && now != &next {
                        return Err("draft_changed".into());
                    }
                    Ok(next.clone())
                })?;
                let _l = locked()?;
                let file = path(authority, &k)?;
                let mut state = read(&file)?;
                state["draft"] = next.clone();
                state["base"] = doc.clone();
                state["cursor"] = snapshot["cursor"].clone();
                state["synced"] = if live {
                    p.clone()
                } else {
                    payload(&next, environment)?
                };
                state["pending"] = Value::Null;
                save_json(&file, &state)?;
                changes[&k] = next;
            }
        }
    }
    Ok(json!({"entries":entries,"changes":changes}))
}
