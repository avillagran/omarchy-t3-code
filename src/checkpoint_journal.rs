//! Shared rewind outbox. Native rollback deduplication uses the saved command ID.
//! The per-thread admission lock is also used by message sends.
use crate::*;
use std::{
    fs::{File, OpenOptions},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
};
pub struct ThreadLock(File);
impl Drop for ThreadLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
fn thread_path(environment: &str, thread: &str) -> Result<PathBuf> {
    if !valid_id(environment) || !valid_id(thread) {
        return Err("revert_scope_mismatch".into());
    }
    Ok(xdg("XDG_STATE_HOME", ".local/state")
        .join("outbox/rewind-threads")
        .join(format!(
            "{:x}.json",
            Sha256::digest(format!("{environment}/{thread}"))
        )))
}
fn action_path(id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id).map_err(|_| "invalid_request_id")?;
    Ok(xdg("XDG_STATE_HOME", ".local/state")
        .join("outbox/rewind-actions")
        .join(format!("{id}.json")))
}
pub fn lock(environment: &str, thread: &str) -> Result<ThreadLock> {
    let p = thread_path(environment, thread)?.with_extension("lock");
    fs::create_dir_all(p.parent().unwrap()).map_err(|_| "local_error")?;
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(p)
        .map_err(|_| "local_error")?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("revert_already_pending".into());
    }
    Ok(ThreadLock(file))
}
fn bounded_read(p: &Path) -> Result<Value> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(p)
        .map_err(|_| "local_error")?;
    let mut bytes = vec![];
    f.take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "local_error")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("revert_limit".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "malformed".into())
}
pub fn read(environment: &str, thread: &str) -> Result<Option<Value>> {
    let p = thread_path(environment, thread)?;
    if !p.exists() {
        return Ok(None);
    }
    let r = bounded_read(&p)?;
    if r["request"]["environment"] != environment || r["request"]["thread"] != thread {
        return Err("revert_scope_mismatch".into());
    }
    Ok(Some(r))
}
pub fn action(id: &str) -> Result<Option<Value>> {
    let p = action_path(id)?;
    if p.exists() {
        Ok(Some(bounded_read(&p)?))
    } else {
        Ok(None)
    }
}
pub fn pending(record: &Value) -> bool {
    matches!(
        record["state"].as_str(),
        Some("prepared" | "accepted" | "uncertain" | "pending" | "superseded")
    ) || record["state"] == "complete" && record["recovered"] != true
}
pub fn ensure_available(environment: &str, thread: &str) -> Result<()> {
    if read(environment, thread)?.as_ref().is_some_and(pending) {
        Err("revert_already_pending".into())
    } else {
        Ok(())
    }
}
fn validated_paths(record: &Value) -> Result<(PathBuf, PathBuf)> {
    let r = &record["request"];
    let expected = checkpoint_revert::confirmed_command(r)?;
    if record["envelope"]["environment"] != r["environment"]
        || record["envelope"]["command"] != expected
    {
        return Err("revert_scope_mismatch".into());
    }
    if record.to_string().len() > 2 * 1024 * 1024 {
        return Err("revert_limit".into());
    }
    Ok((
        action_path(text(r, "requestId"))?,
        thread_path(text(r, "environment"), text(r, "thread"))?,
    ))
}
pub fn insert(record: &Value) -> Result<()> {
    let (archive, index) = validated_paths(record)?;
    let r = &record["request"];
    ensure_available(text(r, "environment"), text(r, "thread"))?;
    if archive.exists() {
        return Err("revert_request_exists".into());
    }
    save_json(&archive, record)?;
    save_json(&index, record)
}
/// A late recovery/result for an older action updates its archive only; it must
/// never replace a newer thread's active guard.
pub fn save(record: &Value) -> Result<()> {
    let (archive, index) = validated_paths(record)?;
    save_json(&archive, record)?;
    let r = &record["request"];
    let current = read(text(r, "environment"), text(r, "thread"))?;
    if current
        .as_ref()
        .is_none_or(|v| v["request"]["requestId"] == r["requestId"])
    {
        save_json(&index, record)?
    }
    Ok(())
}
pub fn receipt(record: &Value) -> Value {
    let mut result = record["request"].clone();
    result["durable"] = json!(true);
    result["state"] = record["state"].clone();
    result["error"] = record["error"].clone();
    result["recovered"] = json!(record["recovered"] == true);
    result["startedAt"] = record["startedAt"].clone();
    result
}
pub fn list() -> Result<Vec<Value>> {
    let p = xdg("XDG_STATE_HOME", ".local/state").join("outbox/rewind-threads");
    if !p.exists() {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for entry in fs::read_dir(p).map_err(|_| "local_error")? {
        let entry = entry.map_err(|_| "local_error")?;
        if entry.path().extension().is_some_and(|s| s == "json") {
            let r = bounded_read(&entry.path())?;
            if pending(&r) {
                out.push(receipt(&r));
                if out.len() > 512 {
                    return Err("revert_limit".into());
                }
            }
        }
    }
    Ok(out)
}
