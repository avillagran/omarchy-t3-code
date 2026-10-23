//! Free space on the disk that holds T3's data and worktrees, on this computer only.
//! T3 servers do not report their free space (neither 0.0.46 nor 0.1.2), so remote
//! environments are not checked.
//!
//! The periodic check is one `statvfs` call. The worktree report (size of each
//! worktree's `target/`, merged branch, local changes) walks directories and runs
//! git, so it only runs when the user opens it.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub const DEFAULT_THRESHOLD_GB: u64 = 15;
pub const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);
const GB: u64 = 1024 * 1024 * 1024;
const MAX_WORKTREES: usize = 64;

/// `lowDiskThresholdGb` from the UI state: a number of GB, or null to switch the warning off.
pub fn threshold_bytes(ui_state: &Value) -> Option<u64> {
    match ui_state.get("lowDiskThresholdGb") {
        Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().filter(|n| (1..=1000).contains(n)).unwrap_or(DEFAULT_THRESHOLD_GB) * GB),
        None => Some(DEFAULT_THRESHOLD_GB * GB),
    }
}
pub fn low(free: u64, threshold: Option<u64>) -> bool {
    threshold.is_some_and(|t| free < t)
}

pub(crate) fn worktrees_root() -> PathBuf {
    t3_home().join("worktrees")
}
fn watched_path() -> PathBuf {
    let worktrees = worktrees_root();
    if worktrees.is_dir() { worktrees } else { t3_home() }
}
fn statvfs(path: &Path) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let unit = s.f_frsize as u64;
    Some((s.f_bavail as u64 * unit, s.f_blocks as u64 * unit))
}

pub fn status(ui_state: &Value) -> Value {
    let path = watched_path();
    let threshold = threshold_bytes(ui_state);
    match statvfs(&path) {
        Some((free, total)) => json!({"path":path,"freeBytes":free,"totalBytes":total,"thresholdBytes":threshold,"low":low(free,threshold)}),
        None => json!({"path":path,"error":"disk_space_unavailable","thresholdBytes":threshold,"low":false}),
    }
}

/// Disk usage of a directory tree (allocated blocks, symlinks not followed).
pub(crate) fn disk_usage(root: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else { continue };
            total += meta.blocks() * 512;
            if meta.is_dir() {
                stack.push(entry.path());
            }
        }
    }
    total
}

pub(crate) fn git(dir: &Path, args: &[&str]) -> Option<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
}
/// Uncommitted or untracked files (ignored ones such as target/ do not count).
fn has_changes(dir: &Path) -> Option<bool> {
    let out = git(dir, &["status", "--porcelain"]).filter(|o| o.status.success())?;
    Some(!out.stdout.is_empty())
}
/// HEAD is contained in the default branch (merged, or no commits of its own).
fn merged_into_default(dir: &Path) -> Option<bool> {
    let base = git(dir, &["rev-parse", "--abbrev-ref", "origin/HEAD"])
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            ["main", "master"].into_iter().find(|b| git(dir, &["rev-parse", "--verify", "--quiet", b]).is_some_and(|o| o.status.success())).map(String::from)
        })?;
    Some(git(dir, &["merge-base", "--is-ancestor", "HEAD", &base])?.status.success())
}

/// The local environment's shell: the live stream's copy, or one HTTP read.
pub(crate) fn local_shell(client: &Client) -> Result<Value> {
    shell_state::live("local").map(Ok).unwrap_or_else(|| {
        environments()?.into_iter().find(|e| e.id == "local").ok_or("unknown_environment".to_string()).and_then(|e| client.get(&e, "/api/orchestration/shell", true))
    })
}
/// Checkouts under T3's worktree folder (`<project>/<worktree>` with a `.git` entry).
pub(crate) fn worktree_folders() -> Vec<PathBuf> {
    let mut out = vec![];
    for project in std::fs::read_dir(worktrees_root()).into_iter().flatten().flatten() {
        for tree in std::fs::read_dir(project.path()).into_iter().flatten().flatten() {
            if tree.path().join(".git").exists() {
                out.push(tree.path());
            }
        }
    }
    out
}

/// Worktrees of local threads, plus any other checkout under T3's worktree folder.
fn candidates(client: &Client) -> BTreeMap<PathBuf, Value> {
    let mut found = BTreeMap::new();
    let shell = local_shell(client);
    for t in shell.as_ref().ok().and_then(|s| s["threads"].as_array()).into_iter().flatten() {
        let Some(path) = t["worktreePath"].as_str().filter(|p| !p.is_empty()) else { continue };
        let merged_pr = t["pullRequests"].as_array().into_iter().flatten().any(|pr| pr["snapshot"]["state"] == "merged");
        found.entry(PathBuf::from(path)).or_insert_with(|| json!({"threadId":t["id"],"title":t["title"],"branch":t["branch"],"pullRequestMerged":merged_pr}));
    }
    for tree in worktree_folders() {
        found.entry(tree).or_insert_with(|| json!({}));
    }
    found
}

pub fn report(client: &Client, ui_state: &Value) -> Value {
    let mut worktrees: Vec<Value> = candidates(client)
        .into_iter()
        .filter(|(path, _)| path.is_dir())
        .take(MAX_WORKTREES)
        .map(|(path, mut info)| {
            let target = path.join("target");
            info["path"] = json!(path);
            info["targetBytes"] = json!(if target.is_dir() { disk_usage(&target) } else { 0 });
            info["merged"] = json!(info["pullRequestMerged"] == true || merged_into_default(&path) == Some(true));
            info["hasChanges"] = json!(has_changes(&path));
            info
        })
        .collect();
    worktrees.sort_by(|a, b| b["targetBytes"].as_u64().cmp(&a["targetBytes"].as_u64()));
    let mut out = status(ui_state);
    out["worktrees"] = json!(worktrees);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn threshold_defaults_to_15_gb_and_null_turns_it_off() {
        assert_eq!(threshold_bytes(&json!({})), Some(15 * GB));
        assert_eq!(threshold_bytes(&json!({"lowDiskThresholdGb":40})), Some(40 * GB));
        assert_eq!(threshold_bytes(&json!({"lowDiskThresholdGb":0})), Some(15 * GB), "out of range falls back");
        assert_eq!(threshold_bytes(&json!({"lowDiskThresholdGb":null})), None);
        assert!(low(14 * GB, Some(15 * GB)));
        assert!(!low(15 * GB, Some(15 * GB)));
        assert!(!low(0, None));
    }
    #[test]
    fn usage_counts_nested_files_but_not_symlink_targets() {
        let root = std::env::temp_dir().join(format!("t3-disk-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("target/debug/deps")).unwrap();
        std::fs::write(root.join("target/debug/deps/a.rlib"), vec![1u8; 64 * 1024]).unwrap();
        std::fs::write(root.join("big"), vec![1u8; 1024 * 1024]).unwrap();
        std::os::unix::fs::symlink(root.join("big"), root.join("target/link")).unwrap();
        let used = disk_usage(&root.join("target"));
        assert!(used >= 64 * 1024 && used < 1024 * 1024, "{used}");
        assert!(status(&json!({}))["freeBytes"].as_u64().is_some());
        std::fs::remove_dir_all(root).unwrap();
    }
}
