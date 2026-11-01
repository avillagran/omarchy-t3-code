//! Local maintenance adapter. Never uses Client::shared or saved Desktop credentials.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{fs, io::Read, os::unix::fs::OpenOptionsExt, path::Path};
static BASE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
pub fn configure(base: &str) {
    let _ = BASE.set(base.trim_end_matches('/').into());
}
fn bounded_file(path: &Path) -> Result<Value> {
    let f = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(_) => return Err("state_unavailable".into()),
    };
    if !f.metadata().is_ok_and(|m| m.is_file() && m.len() <= 65536) {
        return Err("state_limit".into());
    }
    let mut b = Vec::new();
    f.take(65537)
        .read_to_end(&mut b)
        .map_err(|_| "state_unavailable")?;
    if b.len() > 65536 {
        return Err("state_limit".into());
    }
    serde_json::from_slice(&b).map_err(|_| "state_unavailable".into())
}
fn statuses(path: &Path) -> Result<Value> {
    let value = bounded_file(path)?;
    let rows = value.as_object().ok_or("invalid_state")?;
    if rows.len() > 1024 {
        return Err("state_limit".into());
    }
    let mut counts = serde_json::Map::new();
    for row in rows.values() {
        let status = match text(row, "status") {
            "claimed" | "uncertain" => "unknown",
            "released" => "released",
            "completed" => "completed",
            status @ ("scheduled" | "accepted" | "cancelled" | "disabled" | "busy"
            | "unavailable" | "unsupported" | "server") => status,
            "failed" | "error" => "failed",
            _ => "other",
        };
        let n = counts.get(status).and_then(Value::as_u64).unwrap_or(0) + 1;
        counts.insert(status.into(), json!(n));
    }
    Ok(json!({"counts":counts,"total":rows.len(),"source":"persisted","live":false}))
}
fn sync_projection(raw: &Value) -> Result<Value> {
    let rows = raw
        .as_object()
        .filter(|o| o.len() <= 64)
        .ok_or("state_limit")?;
    let hosts=rows.iter().filter(|(id,_)|valid_id(id)).map(|(id,row)| {
        let items=row["items"].as_array().into_iter().flatten().take(32).map(|item| {
            let status=match text(item,"status") {
                s @ ("done"|"partial"|"failed"|"unchanged"|"cloned"|"pulled"|"unavailable"|"auth"|"unreachable"|"occupied"|"conflict"|"nogit")=>s,
                _=>"unknown",
            };
            let mut counts=json!({});
            for key in ["added","updated","unchanged","kept","changed","failed","left"] {if let Some(n)=item["counts"][key].as_u64(){counts[key]=json!(n)}}
            json!({"id":text(item,"id").chars().take(64).collect::<String>(),"status":status,"counts":counts})
        }).collect::<Vec<_>>();
        let at=row["at"].as_i64().or_else(||row["at"].as_str().and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).map(|at|at.timestamp_millis()));
        json!({"environment":id,"at":at,"items":items})
    }).collect::<Vec<_>>();
    Ok(json!({"hosts":hosts,"source":"persisted","live":false}))
}
fn policy(s: &Value, surface: &str) -> Value {
    let s = session_memory::host_settings(s, "local");
    let keys = if surface == "compaction" {
        &[
            "autoCompactEnabled",
            "autoCompactFallbackMinutes",
            "autoCompactMinTokens",
            "autoCompactProviderMinutes",
            "autoCompactExcludedThreads",
        ][..]
    } else {
        &[
            "idleReleaseEnabled",
            "idleReleaseMinutes",
            "idleReleaseProviderMinutes",
        ][..]
    };
    let mut result = json!({});
    for k in keys {
        result[k] = s[k].clone()
    }
    result
}
fn native_read(path: &str, bearer: &str) -> Result<Value> {
    let base = BASE.get().ok_or("auth_base_unavailable")?;
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|_| "read_unavailable")?;
    let r = client
        .get(format!("{base}{path}"))
        .bearer_auth(bearer)
        .send()
        .map_err(|_| "read_unavailable")?;
    if !r.status().is_success() {
        return Err("read_denied".into());
    }
    let limit = if path == "/api/auth/session" {
        16384
    } else {
        1024 * 1024
    };
    let mut b = Vec::new();
    r.take(limit + 1)
        .read_to_end(&mut b)
        .map_err(|_| "read_unavailable")?;
    if b.len() as u64 > limit {
        return Err("report_limit".into());
    }
    serde_json::from_slice(&b).map_err(|_| "read_unavailable".into())
}
fn idle_projection(p: &Value) -> bool {
    if !p["thread"]
        .as_object()
        .is_some_and(|t| t.contains_key("activeRunId"))
        || !p["thread"]["activeRunId"].is_null()
    {
        return false;
    }
    [
        "runs",
        "runtimeRequests",
        "subagents",
        "nodes",
        "plans",
        "turnItems",
    ]
    .iter()
    .all(|key| {
        p[key].as_array().is_some_and(|rows| {
            rows.iter().all(|r| {
                !matches!(
                    text(r, "status"),
                    "preparing"
                        | "starting"
                        | "running"
                        | "waiting"
                        | "pending"
                        | "queued"
                        | "proposed"
                )
            })
        })
    })
}
fn cleanup_report(settings: &Value, bearer: &str) -> Result<(Value, Value)> {
    let start = std::time::Instant::now();
    let roots = settings["remoteMaintenanceRoots"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if roots.is_empty() {
        let shell = json!({"projects":[],"threads":[]});
        let mut report = crate::build_cleanup::remote_report(&shell, settings)?;
        report["idleClaims"] = json!([]);
        return Ok((shell, report));
    }
    let shell = native_read("/api/orchestration/shell", bearer)?;
    let projects = shell["projects"].as_array().ok_or("invalid_shell")?;
    let threads = shell["threads"].as_array().ok_or("invalid_shell")?;
    let mut claims = Vec::new();
    for t in threads {
        let root = t["worktreePath"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                projects
                    .iter()
                    .find(|p| p["id"] == t["projectId"])
                    .and_then(|p| p["workspaceRoot"].as_str())
            });
        if !root.is_some_and(|r| {
            roots
                .iter()
                .filter_map(Value::as_str)
                .any(|v| crate::build_cleanup::paths_overlap(Path::new(v), Path::new(r)))
        }) {
            continue;
        }
        if claims.len() >= 16 || start.elapsed() > std::time::Duration::from_secs(2) {
            return Err("report_limit".into());
        }
        let id = text(t, "id");
        let envelope = native_read(&protocol::bounded_snapshot::path(id)?, bearer)?;
        let restored = protocol::bounded_snapshot::restore(envelope)?;
        if restored["projection"]["thread"]["id"] != id || !idle_projection(&restored["projection"])
        {
            return Err("idle_not_proven".into());
        }
        use sha2::{Digest, Sha256};
        claims.push(json!({"threadId":id,"fingerprint":format!("{:x}",Sha256::digest(restored["projection"].to_string().as_bytes()))}));
    }
    let mut report = crate::build_cleanup::remote_report(&shell, settings)?;
    report["idleClaims"] = json!(claims);
    Ok((shell, report))
}
fn vault_config(paths: &journal_vault::Paths) -> Result<Value> {
    let saved = bounded_file(&paths.config)?;
    let mut config = journal_vault::default_config(paths);
    for key in ["enabled", "path", "remote", "agentInstructions"] {
        if let Some(v) = saved.get(key) {
            config[key] = v.clone()
        }
    }
    let path = text(&config, "path");
    if !path.starts_with('/') || path.len() > 2048 || path.chars().any(char::is_control) {
        return Err("invalid_target".into());
    }
    Ok(config)
}
fn vault_status(vault: &Path, config: &Value) -> Result<Value> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::MetadataExt;
    if !vault.exists() {
        return Ok(json!({"state":"missing"}));
    }
    if !vault.is_dir() || vault.canonicalize().ok().as_deref() != Some(vault) {
        return Err("invalid_target".into());
    }
    let meta = vault.symlink_metadata().map_err(|_| "read_unavailable")?;
    if !vault.join(".git").exists() {
        return Ok(json!({"state":"not-git","identity":[meta.dev(),meta.ino()]}));
    }
    let remote = crate::build_cleanup::remote_git(vault, &["remote", "get-url", "origin"], 4096)?;
    let url = std::str::from_utf8(&remote)
        .map_err(|_| "read_unavailable")?
        .trim();
    let matches = config["remote"]["url"].as_str().is_none_or(|expected| {
        journal_vault::canonical_remote(expected) == journal_vault::canonical_remote(url)
    });
    let changes = crate::build_cleanup::remote_git(vault, &["status", "--porcelain"], 65536)?;
    let revision =
        crate::build_cleanup::remote_git(vault, &["rev-parse", "--verify", "HEAD"], 4096).ok();
    Ok(
        json!({"state":if matches{"clone"}else{"other-remote"},"identity":[meta.dev(),meta.ino()],"origin":format!("{:x}",Sha256::digest(remote)),"head":revision.map(|r|String::from_utf8_lossy(&r).trim().to_string()),"changes":changes.split(|b|*b==b'\n').filter(|l|!l.is_empty()).count()}),
    )
}
pub fn execute(input: &Value, settings: &Value, bearer: &str) -> Result<Value> {
    let surface = text(input, "surface");
    let paths = journal_vault::Paths::current();
    if input["op"] == "report" {
        return match surface {
            "memory" => Ok(
                json!({"idleRelease":policy(settings,"idle-release"),"compaction":policy(settings,"compaction"),"scope":"local-host"}),
            ),
            "compaction" | "idle-release" => Ok(
                json!({"settings":policy(settings,surface),"hostOverrides":settings["remoteHostMemorySettings"],"status":statuses(&xdg("XDG_STATE_HOME",".local/state").join(if surface=="compaction" {"compaction/ledger.json"} else {"idle-release/claims.json"}))?}),
            ),
            "host-sync" => sync_projection(&bounded_file(
                &xdg("XDG_STATE_HOME", ".local/state").join("host-sync.json"),
            )?),
            "vault" => {
                let config = vault_config(&paths)?;
                let vault = std::path::PathBuf::from(text(&config, "path"));
                let state = vault_status(&vault, &config)?;
                let mut files = Vec::new();
                for relative in journal_vault::HOST_FILES {
                    let path = paths.home.join(relative);
                    if let Some(parent) = path.parent().filter(|p| p.exists()) {
                        let resolved = parent.canonicalize().map_err(|_| "invalid_target")?;
                        let home = paths.home.canonicalize().map_err(|_| "invalid_target")?;
                        if !resolved.starts_with(home) {
                            return Err("invalid_target".into());
                        }
                    }

                    let bytes = match fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                        .open(&path)
                    {
                        Ok(f) => {
                            if !f.metadata().is_ok_and(|m| m.is_file() && m.len() <= 65536) {
                                return Err("report_limit".into());
                            }
                            let mut b = Vec::new();
                            f.take(65537)
                                .read_to_end(&mut b)
                                .map_err(|_| "read_unavailable")?;
                            if b.len() > 65536 {
                                return Err("report_limit".into());
                            }
                            b
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                        _ => return Err("read_unavailable".into()),
                    };
                    use sha2::{Digest, Sha256};
                    files.push(json!({"file":relative,"state":journal_vault::block_state(std::str::from_utf8(&bytes).map_err(|_|"read_unavailable")?,text(&config,"path")),"fingerprint":format!("{:x}",Sha256::digest(bytes))}));
                }
                Ok(
                    json!({"configured":config["enabled"]==true && state["state"]=="clone","steps":{"clone":state["state"]=="clone","instructions":files.iter().all(|f|f["state"]=="current")},"localState":state["state"],"git":state,"enabled":config["enabled"],"instructions":config["agentInstructions"],"vaultIdentity":vault,"configuration":({use sha2::{Digest,Sha256};format!("{:x}",Sha256::digest(config.to_string().as_bytes()))}),"files":files,"scope":"local-host"}),
                )
            }
            "build-cleanup" => cleanup_report(settings, bearer).map(|(_, report)| report),
            _ => Err("not_found".into()),
        };
    }
    if input["op"] != "apply" {
        return Err("invalid_command".into());
    }
    // The broker already authenticated and consumed a durable claim. Recheck local
    // consent and the exact report again at the adapter boundary before a side effect.
    let current = crate::ui_state::load();
    if remote_maintenance::epoch(&current) != remote_maintenance::epoch(settings)
        || current["remoteDocumentsConsent"] != settings["remoteDocumentsConsent"]
        || current["remoteDocumentsEnabled"] != true
    {
        return Err("consent_required".into());
    }
    let fresh = execute(&json!({"op":"report","surface":surface}), &current, bearer)?;
    if fresh != input["report"] {
        return Err("stale_report".into());
    }
    let authorization = native_read("/api/auth/session", bearer)?;
    if !remote_maintenance::operate_allowed(&authorization) {
        return Err("read_denied".into());
    }
    if crate::ui_state::load() != current {
        return Err("stale_report".into());
    }
    match (surface, text(input, "action")) {
        ("compaction" | "idle-release", "settings")
            if remote_maintenance::valid_patch(surface, &input["settings"]) =>
        {
            crate::ui_state::save_memory_policy(surface, &input["settings"])?;
            Ok(json!({"saved":true,"takesEffect":"next-worker-poll","scope":"local-host"}))
        }
        ("vault", "rollout") => {
            let config = vault_config(&paths)?;
            if config["enabled"] != true || config["agentInstructions"] != true {
                return Err("vault_not_enabled".into());
            }
            let vault = std::path::PathBuf::from(text(&config, "path"));
            if !vault.is_absolute()
                || vault.canonicalize().ok().as_ref() != Some(&vault)
                || vault_status(&vault, &config)?["state"] != "clone"
            {
                return Err("vault_not_configured".into());
            }
            let files = journal_vault::install_instructions(&paths.home, &vault)?;
            Ok(json!({"installed":files.len(),"scope":"local-host"}))
        }
        ("build-cleanup", "run") => {
            let (shell, mut report) = cleanup_report(&current, bearer)?;
            if report != input["report"] {
                return Err("stale_report".into());
            }
            report
                .as_object_mut()
                .ok_or("invalid_report")?
                .remove("idleClaims");
            crate::build_cleanup::remote_run(&shell, &current, &report)
        }
        _ => Err("invalid_command".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_requires_complete_idle_claims_and_rejects_pending_work() {
        let mut p = json!({"thread":{"activeRunId":null},"runs":[],"runtimeRequests":[],"subagents":[],"nodes":[],"plans":[],"turnItems":[]});
        assert!(idle_projection(&p));
        for key in [
            "runs",
            "runtimeRequests",
            "subagents",
            "nodes",
            "plans",
            "turnItems",
        ] {
            let before = p[key].clone();
            p[key] = json!([{"status":"pending"}]);
            assert!(!idle_projection(&p));
            p[key] = Value::Null;
            assert!(!idle_projection(&p));
            p[key] = before;
        }
        p["thread"]["activeRunId"] = json!("running");
        assert!(!idle_projection(&p));
        assert!(!idle_projection(&json!({})));
    }
    #[test]
    fn ledger_projection_is_bounded_redacted_and_claimed_means_unknown() {
        let dir = std::path::PathBuf::from("/var/tmp")
            .join(format!("remote-ledger-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        fs::write(
            &path,
            r#"{"private-thread":{"status":"claimed","token":"PRIVATE"}}"#,
        )
        .unwrap();
        let v = statuses(&path).unwrap();
        assert_eq!(v["counts"]["unknown"], 1);
        assert!(!v.to_string().contains("PRIVATE"));
        assert!(!v.to_string().contains("private-thread"));
        fs::write(&path, "x".repeat(65537)).unwrap();
        assert!(statuses(&path).is_err());
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink("/dev/zero", &path).unwrap();
        assert!(statuses(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn host_sync_status_preserves_native_outcomes_time_and_counts_without_backups() {
        let raw = json!({"local":{"at":"2026-10-31T00:00:00Z","backup":"PRIVATE","items":[{"id":"skills","status":"partial","counts":{"updated":2,"failed":1,"secret":"PRIVATE"}},{"id":"vault","status":"cloned"}]}});
        let report = sync_projection(&raw).unwrap();
        assert_eq!(report["hosts"][0]["items"][0]["status"], "partial");
        assert_eq!(report["hosts"][0]["items"][1]["status"], "cloned");
        assert_eq!(report["hosts"][0]["items"][0]["counts"]["updated"], 2);
        assert!(report["hosts"][0]["at"].as_i64().is_some());
        assert!(!report.to_string().contains("PRIVATE"));
    }
}
