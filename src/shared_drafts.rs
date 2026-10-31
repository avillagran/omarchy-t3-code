//! Durable command journal for OmaBridge shared drafts. This adapter is opt-in
//! and deliberately independent of local send attempts, outbox and recovery UI.
use crate::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};

#[derive(Default, Serialize, Deserialize)]
struct Journal {
    authority: String,
    pending: Option<Value>,
    outcome: Option<Value>,
}

fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}
fn scope(v: &Value) -> bool {
    v.as_object().is_some_and(|v| v.len() == 4)
        && ["environment", "project", "id"]
            .iter()
            .all(|k| identifier(text(v, k)))
        && matches!(text(v, "kind"), "thread" | "stash")
        && uuid::Uuid::parse_str(text(v, "environment")).is_ok()
        && uuid::Uuid::parse_str(text(v, "project")).is_ok()
}
fn command(authority: &str, v: &Value) -> Result<()> {
    let keys: &[&str] = match text(v, "op") {
        "put" => &[
            "op",
            "scope",
            "expected",
            "baseCursor",
            "mutationId",
            "payload",
        ],
        "delete" => &["op", "scope", "expected", "baseCursor", "mutationId"],
        "transfer" => &[
            "op",
            "scope",
            "expected",
            "target",
            "targetExpected",
            "backup",
            "payload",
            "baseCursor",
            "mutationId",
        ],
        _ => return Err("invalid_shared_draft_command".into()),
    };
    let object = v.as_object().ok_or("invalid_shared_draft_command")?;
    if object.keys().any(|k| !keys.contains(&k.as_str()))
        || !scope(&v["scope"])
        || !v
            .get("expected")
            .is_some_and(|r| r.is_null() || r.is_string())
        || v.to_string().len() > 32 * 1024
    {
        return Err("invalid_shared_draft_command".into());
    }
    if v["op"] == "put" && !v["payload"].is_object() {
        return Err("invalid_shared_draft_command".into());
    }
    if v["op"] == "transfer"
        && (!scope(&v["target"])
            || !v
                .get("targetExpected")
                .is_some_and(|r| r.is_null() || r.is_string())
            || v.get("backup")
                .is_some_and(|b| !scope(b) || b["kind"] != "stash"))
    {
        return Err("invalid_shared_draft_command".into());
    }
    let cursor = text(v, "baseCursor");
    let sequence = cursor
        .strip_prefix(&format!("{authority}:"))
        .filter(|s| s.parse::<u64>().is_ok_and(|n| n.to_string() == *s))
        .ok_or("invalid_shared_draft_cursor")?;
    let id = text(v, "mutationId")
        .strip_prefix(&format!("{sequence}."))
        .ok_or("invalid_shared_draft_mutation")?;
    uuid::Uuid::parse_str(id).map_err(|_| "invalid_shared_draft_mutation")?;
    Ok(())
}
fn request(authority: &str, command: &Value) -> Value {
    let mutation = matches!(text(command, "op"), "put" | "delete" | "transfer");
    json!({"action":"drafts", "peer":authority, "confirmed":mutation, "command":command})
}
fn call(authority: &str, command: &Value) -> Result<Value> {
    let (out, status) = omabridge_device::manage(&request(authority, command), 30);
    // Protocol errors are results too: preserve resync/conflict information.
    if status != 0
        && !out["error"]
            .as_str()
            .is_some_and(|e| e.starts_with("draft_"))
    {
        return Err("shared_drafts_unavailable".into());
    }
    Ok(out)
}
fn private(meta: &fs::Metadata, directory: bool) -> bool {
    meta.uid() == unsafe { libc::geteuid() }
        && meta.permissions().mode() & 0o077 == 0
        && if directory {
            meta.is_dir()
        } else {
            meta.is_file()
        }
}
struct Locked {
    _file: fs::File,
    directory: PathBuf,
}
impl Locked {
    fn open() -> Result<Self> {
        let parent = xdg("XDG_STATE_HOME", ".local/state");
        fs::create_dir_all(&parent).map_err(|_| "shared_drafts_local_error")?;
        let directory = parent.join("shared-drafts");
        match fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(_) => return Err("shared_drafts_local_error".into()),
        }
        if !fs::symlink_metadata(&directory).is_ok_and(|m| private(&m, true)) {
            return Err("shared_drafts_private_state_required".into());
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(directory.join("journal.lock"))
            .map_err(|_| "shared_drafts_private_state_required")?;
        if !file.metadata().is_ok_and(|m| private(&m, false))
            || unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0
        {
            return Err("shared_drafts_busy".into());
        }
        Ok(Self {
            _file: file,
            directory,
        })
    }
    fn load(&self, path: &Path) -> Result<Journal> {
        let mut file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Journal::default()),
            Err(_) => return Err("shared_drafts_private_state_required".into()),
        };
        if !file
            .metadata()
            .is_ok_and(|m| private(&m, false) && m.len() <= 128 * 1024)
        {
            return Err("shared_drafts_private_state_required".into());
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|_| "shared_drafts_local_error")?;
        serde_json::from_slice(&bytes).map_err(|_| "shared_drafts_journal_invalid".into())
    }
}
use std::os::fd::AsRawFd;
fn store(path: &Path, journal: &Journal) -> Result<()> {
    save_json(
        path,
        &serde_json::to_value(journal).map_err(|_| "shared_drafts_local_error")?,
    )
}
fn blocked(out: &Value) -> bool {
    out["status"] == "conflict"
        || matches!(
            text(out, "error"),
            "draft_resync_required" | "draft_snapshot_required" | "draft_mutation_reused"
        )
}
fn snapshot(authority: &str) -> Result<Value> {
    let mut items = Vec::new();
    let mut cursor = Value::Null;
    let mut offset = None;
    let mut bytes = 0usize;
    // A corrupt peer must not turn bounded pagination into an infinite loop.
    for _ in 0..4096 {
        let mut command = json!({"op":"list"});
        if let Some(offset) = offset {
            command["offset"] = json!(offset);
            command["snapshot"] = cursor.clone();
        }
        let page = call(authority, &command)?;
        if page.get("error").is_some() {
            return Ok(page);
        }
        if page["owner"] != authority
            || !page["cursor"].is_string()
            || (!cursor.is_null() && page["cursor"] != cursor)
        {
            return Err("shared_drafts_invalid_snapshot".into());
        }
        cursor = page["cursor"].clone();
        let rows = page["items"]
            .as_array()
            .ok_or("shared_drafts_invalid_snapshot")?;
        if rows.len() > 2 {
            return Err("shared_drafts_invalid_snapshot".into());
        }
        bytes += serde_json::to_vec(rows)
            .map_err(|_| "shared_drafts_invalid_snapshot")?
            .len();
        if bytes > 16 * 1024 * 1024 {
            return Err("shared_drafts_snapshot_limit".into());
        }
        items.extend(rows.iter().cloned());
        match page.get("nextOffset").filter(|v| !v.is_null()) {
            None => return Ok(json!({"owner":authority,"items":items,"cursor":cursor})),
            Some(v) => {
                let next = v.as_u64().ok_or("shared_drafts_invalid_snapshot")?;
                if next <= offset.unwrap_or(0) {
                    return Err("shared_drafts_invalid_snapshot".into());
                }
                offset = Some(next);
            }
        }
    }
    Err("shared_drafts_snapshot_limit".into())
}
/// Local-only foundation API. Explicit authority + journal ID are required.
/// `stage` fsyncs the exact command without network I/O; `retry` sends only that
/// command. Conflicts require an explicit new journal, preserving the old branch.
/// This is not a remote host endpoint and does not enable composer sharing.
pub fn execute(input: &Value) -> Result<Value> {
    let authority = text(input, "authority");
    if !omabridge_route::valid_peer(authority) {
        return Err("invalid_shared_draft_authority".into());
    }
    match text(input, "action") {
        "snapshot" => return snapshot(authority),
        "get" if scope(&input["scope"]) => {
            return call(authority, &json!({"op":"get","scope":input["scope"]}))
        }
        "stage" | "retry" | "status" => (),
        _ => return Err("invalid_shared_draft_action".into()),
    }
    let id = text(input, "journalId");
    uuid::Uuid::parse_str(id).map_err(|_| "invalid_shared_draft_journal")?;
    let locked = Locked::open()?;
    let path = locked.directory.join(format!(
        "{:x}.json",
        Sha256::digest(format!("{authority}/{id}"))
    ));
    let mut journal = locked.load(&path)?;
    if !journal.authority.is_empty() && journal.authority != authority {
        return Err("shared_drafts_authority_changed".into());
    }
    if input["action"] == "stage" {
        command(authority, &input["command"])?;
        if journal
            .pending
            .as_ref()
            .is_some_and(|old| old != &input["command"])
        {
            return Err("shared_drafts_pending_preserved".into());
        }
        journal.authority = authority.into();
        journal.pending = Some(input["command"].clone());
        store(&path, &journal)?;
    } else if input["action"] == "retry" {
        let pending = journal.pending.as_ref().ok_or("shared_drafts_no_pending")?;
        command(authority, pending)?;
        if !journal
            .outcome
            .as_ref()
            .is_some_and(|o| blocked(o) || o["status"] == "applied")
        {
            // Persisted before every send, including after a process interruption.
            store(&path, &journal)?;
            let outcome = call(authority, pending)?;
            if matches!(text(&outcome, "status"), "applied" | "conflict")
                && outcome["mutationId"] != pending["mutationId"]
            {
                return Err("shared_drafts_invalid_receipt".into());
            }
            journal.outcome = Some(outcome);
            store(&path, &journal)?;
        }
    }
    Ok(
        json!({"authority":authority,"journalId":id,"command":journal.pending,"outcome":journal.outcome,
        "conflict":journal.outcome.as_ref().is_some_and(blocked)}),
    )
}
