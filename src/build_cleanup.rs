//! Automatic cleanup of build output on this computer: regenerable, git-ignored
//! build folders (Cargo `target/`, Gradle/CMake `build/`, `.next/`, `zig-cache/`,
//! any folder marked with `CACHEDIR.TAG`, ...) in worktrees and project checkouts
//! that have been idle for a user-chosen number of days.
//!
//! Idle means both: no activity of the folder's threads in T3, and no change to
//! its files (tracked or untracked, its git index/HEAD, or the build folder itself,
//! so building by hand counts as use). Sources, branches and uncommitted changes are
//! never touched, nor is `node_modules` (reinstalling needs the network). Without a
//! shell to read thread activity from, nothing is cleaned.
use omarchy_t3_client::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const DEFAULT_WORKTREE_DAYS: u64 = 1;
pub const DEFAULT_CHECKOUT_DAYS: u64 = 7;
pub const CHECK_INTERVAL: Duration = Duration::from_secs(3600);
/// The first check waits for the login rush to pass.
pub const FIRST_CHECK: Duration = Duration::from_secs(120);
const DAY_MS: i64 = 24 * 3600 * 1000;
const MAX_FILES: usize = 200_000;

/// Project markers and the build folders their tools regenerate.
const RULES: &[(&[&str], &[&str])] = &[
    (&["Cargo.toml"], &["target"]),
    (&["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"], &["build", ".gradle"]),
    (&["CMakeLists.txt"], &["build"]),
    (&["package.json"], &[".next", ".nuxt", ".svelte-kit", ".turbo", ".parcel-cache"]),
    (&["build.zig"], &["zig-cache", ".zig-cache"]),
];

/// `buildCleanupWorktreeDays` / `buildCleanupCheckoutDays` from the UI state: days, or
/// null to switch that kind off.
pub fn days(ui_state: &Value, key: &str, default: u64) -> Option<u64> {
    match ui_state.get(key) {
        Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().filter(|n| (1..=365).contains(n)).unwrap_or(default)),
        None => Some(default),
    }
}
pub fn due(last_use_ms: i64, now_ms: i64, days: Option<u64>) -> bool {
    days.is_some_and(|d| now_ms.saturating_sub(last_use_ms) >= d as i64 * DAY_MS)
}

fn ignored(root: &Path, dir: &Path) -> bool {
    let Ok(rel) = dir.strip_prefix(root) else { return false };
    let rel = format!("{}/", rel.display());
    crate::disk_space::git(root, &["check-ignore", "-q", "--", &rel]).is_some_and(|o| o.status.success())
}
fn real_dir(path: &Path) -> bool {
    path.symlink_metadata().is_ok_and(|m| m.is_dir())
}

/// Build folders directly under `root` that are safe to delete.
pub fn build_dirs(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = vec![];
    for (markers, dirs) in RULES {
        if !markers.iter().any(|m| root.join(m).is_file()) {
            continue;
        }
        for d in *dirs {
            let path = root.join(d);
            // A CMake `build/` is only build output once CMake configured it.
            if *d == "build" && markers.contains(&"CMakeLists.txt") && !path.join("CMakeCache.txt").is_file() {
                continue;
            }
            found.push(path);
        }
    }
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if entry.file_name() != ".git" && entry.path().join("CACHEDIR.TAG").is_file() {
            found.push(entry.path());
        }
    }
    found.sort();
    found.dedup();
    found.retain(|p| real_dir(p) && ignored(root, p));
    found
}

fn mtime_ms(path: &Path) -> i64 {
    path.symlink_metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
/// Newest change to the folder's files, git state or build output.
pub fn last_file_change(root: &Path, builds: &[PathBuf]) -> i64 {
    let mut newest = 0;
    if let Some(out) = crate::disk_space::git(root, &["ls-files", "-z", "--cached", "--others", "--exclude-standard"]).filter(|o| o.status.success()) {
        for name in out.stdout.split(|b| *b == 0).filter(|n| !n.is_empty()).take(MAX_FILES) {
            newest = newest.max(mtime_ms(&root.join(String::from_utf8_lossy(name).as_ref())));
        }
    }
    if let Some(out) = crate::disk_space::git(root, &["rev-parse", "--absolute-git-dir"]).filter(|o| o.status.success()) {
        let git_dir = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        for f in ["index", "HEAD", "logs/HEAD"] {
            newest = newest.max(mtime_ms(&git_dir.join(f)));
        }
    }
    for build in builds {
        newest = newest.max(mtime_ms(build));
        for entry in std::fs::read_dir(build).into_iter().flatten().flatten() {
            newest = newest.max(mtime_ms(&entry.path()));
        }
    }
    newest
}

fn thread_activity(t: &Value, now: i64) -> i64 {
    if !t["activeRunId"].is_null() {
        return now;
    }
    ["updatedAt", "latestRunCompletedAt", "latestUserAuthoredMessageAt", "createdAt"].iter().map(|k| epoch(&t[*k])).max().unwrap_or(0)
}

/// Worktrees and checkouts on this computer, with the latest activity of their threads.
fn folders(shell: &Value, now: i64) -> BTreeMap<PathBuf, (&'static str, i64)> {
    let mut out: BTreeMap<PathBuf, (&'static str, i64)> = BTreeMap::new();
    let threads = shell["threads"].as_array().cloned().unwrap_or_default();
    for t in &threads {
        if let Some(path) = t["worktreePath"].as_str().filter(|p| !p.is_empty()) {
            let slot = out.entry(PathBuf::from(path)).or_insert(("worktree", 0));
            slot.1 = slot.1.max(thread_activity(t, now));
        }
    }
    for tree in crate::disk_space::worktree_folders() {
        out.entry(tree).or_insert(("worktree", 0));
    }
    for p in shell["projects"].as_array().into_iter().flatten() {
        let Some(root) = p["workspaceRoot"].as_str().filter(|r| !r.is_empty()) else { continue };
        let root = PathBuf::from(root);
        if out.contains_key(&root) {
            continue;
        }
        let activity = threads
            .iter()
            .filter(|t| t["projectId"] == p["id"] && t["worktreePath"].as_str().is_none_or(str::is_empty))
            .map(|t| thread_activity(t, now))
            .max()
            .unwrap_or(0);
        out.insert(root, ("checkout", activity));
    }
    out
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// One cleanup pass; returns what was removed (or, with `dry_run`, what would be).
pub fn run(client: &Client, ui_state: &Value, dry_run: bool) -> Result<Value> {
    let shell = crate::disk_space::local_shell(client)?;
    let now = now_ms();
    let limits = [
        ("worktree", days(ui_state, "buildCleanupWorktreeDays", DEFAULT_WORKTREE_DAYS)),
        ("checkout", days(ui_state, "buildCleanupCheckoutDays", DEFAULT_CHECKOUT_DAYS)),
    ];
    let (mut entries, mut failed, mut freed) = (vec![], vec![], 0u64);
    for (root, (kind, activity)) in folders(&shell, now) {
        let limit = limits.iter().find(|(k, _)| *k == kind).and_then(|(_, d)| *d);
        // Cheap checks first: thread activity, then whether there is anything to clean.
        if !due(activity, now, limit) || !real_dir(&root) {
            continue;
        }
        let builds = build_dirs(&root);
        if builds.is_empty() || !due(last_file_change(&root, &builds), now, limit) {
            continue;
        }
        for dir in builds {
            let bytes = crate::disk_space::disk_usage(&dir);
            if dry_run {
                freed += bytes;
                entries.push(json!({"path":root,"dir":dir,"kind":kind,"bytes":bytes}));
                continue;
            }
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => {
                    eprintln!("ot3: removed idle build output {} ({bytes} bytes)", dir.display());
                    freed += bytes;
                    entries.push(json!({"path":root,"dir":dir,"kind":kind,"bytes":bytes}));
                }
                Err(error) => failed.push(json!({"path":root,"dir":dir,"kind":kind,"error":error.kind().to_string()})),
            }
        }
    }
    Ok(json!({"at":now,"dryRun":dry_run,"entries":entries,"failed":failed,"freedBytes":freed}))
}

static BUSY: AtomicBool = AtomicBool::new(false);
/// Runs a pass in the background, one at a time. `done` gets the result.
pub fn spawn(client: Client, done: impl FnOnce(Result<Value>) + Send + 'static) -> bool {
    if BUSY.swap(true, Ordering::SeqCst) {
        return false;
    }
    std::thread::spawn(move || {
        let result = run(&client, &crate::ui_state::load(), false);
        BUSY.store(false, Ordering::SeqCst);
        done(result);
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    fn repo() -> PathBuf {
        let root = std::env::temp_dir().join(format!("t3-build-cleanup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(Command::new("git").arg("-C").arg(&root).args(["init", "-q"]).status().unwrap().success());
        root
    }
    #[test]
    fn days_default_and_off() {
        assert_eq!(days(&json!({}), "buildCleanupWorktreeDays", 1), Some(1));
        assert_eq!(days(&json!({"buildCleanupCheckoutDays":3}), "buildCleanupCheckoutDays", 7), Some(3));
        assert_eq!(days(&json!({"buildCleanupCheckoutDays":0}), "buildCleanupCheckoutDays", 7), Some(7));
        assert_eq!(days(&json!({"buildCleanupWorktreeDays":null}), "buildCleanupWorktreeDays", 1), None);
        assert!(due(0, DAY_MS, Some(1)));
        assert!(!due(1, DAY_MS, Some(1)));
        assert!(!due(0, 100 * DAY_MS, None));
    }
    #[test]
    fn only_ignored_regenerable_folders_qualify() {
        let root = repo();
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(root.join("CMakeLists.txt"), "").unwrap();
        std::fs::write(root.join(".gitignore"), "target/\nbuild/\ncache/\nlinked\n").unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        // CMake build/ without CMakeCache.txt is not known to be output.
        std::fs::create_dir_all(root.join("build")).unwrap();
        // A tagged cache folder qualifies whatever the project kind…
        std::fs::create_dir_all(root.join("cache")).unwrap();
        std::fs::write(root.join("cache/CACHEDIR.TAG"), "Signature: 8a477f597d28d172789f06886806bc55").unwrap();
        // …but only when git ignores it.
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets/CACHEDIR.TAG"), "x").unwrap();
        // Symlinks are never followed.
        std::os::unix::fs::symlink(root.join("target"), root.join("linked")).unwrap();
        assert_eq!(build_dirs(&root), vec![root.join("cache"), root.join("target")]);
        std::fs::write(root.join("build/CMakeCache.txt"), "").unwrap();
        assert_eq!(build_dirs(&root), vec![root.join("build"), root.join("cache"), root.join("target")]);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn building_or_editing_counts_as_use() {
        let root = repo();
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        let builds = build_dirs(&root);
        assert!(now_ms() - last_file_change(&root, &builds) < 60_000, "fresh files are recent use");
        let old = std::fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(3 * 86400));
        for p in ["Cargo.toml", ".gitignore", "target", "target/debug"] {
            std::fs::File::open(root.join(p)).unwrap().set_times(old).unwrap();
        }
        let git_dir = root.join(".git");
        for f in ["HEAD"] {
            std::fs::File::open(git_dir.join(f)).unwrap().set_times(old).unwrap();
        }
        assert!(due(last_file_change(&root, &builds), now_ms(), Some(2)), "untouched for 3 days");
        std::fs::write(root.join("target/debug/new"), "").unwrap();
        std::fs::File::open(root.join("target")).unwrap().set_times(old).unwrap();
        assert!(!due(last_file_change(&root, &builds), now_ms(), Some(2)), "a fresh build is use");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn thread_folders_carry_their_latest_activity() {
        let shell = json!({"projects":[{"id":"p","workspaceRoot":"/w/main"}],"threads":[
            {"id":"a","projectId":"p","worktreePath":"/w/tree","updatedAt":"2026-10-01T00:00:00Z"},
            {"id":"b","projectId":"p","worktreePath":null,"updatedAt":"2026-10-02T00:00:00Z"},
            {"id":"c","projectId":"p","worktreePath":"/w/busy","activeRunId":"r","updatedAt":"2026-01-01T00:00:00Z"}]});
        let f = folders(&shell, 42);
        assert_eq!(f[&PathBuf::from("/w/tree")], ("worktree", epoch(&json!("2026-10-01T00:00:00Z"))));
        assert_eq!(f[&PathBuf::from("/w/main")], ("checkout", epoch(&json!("2026-10-02T00:00:00Z"))));
        assert_eq!(f[&PathBuf::from("/w/busy")].1, 42, "a running thread is in use now");
    }
}
