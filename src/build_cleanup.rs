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

// Remote scans use bounded subprocess output and deadlines rather than the
// desktop's broad discovery/report helpers.
pub(crate) fn remote_git(root: &Path, args: &[&str], limit: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut child = std::process::Command::new("timeout")
        .arg("1s")
        .arg("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "report_unavailable")?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or("report_unavailable")?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > limit {
        let _ = child.kill();
        let _ = child.wait();
        return Err("report_limit".into());
    }
    if !child.wait().map_err(|_| "report_unavailable")?.success() {
        return Err("report_unavailable".into());
    }
    Ok(bytes)
}
fn remote_source_change(root: &Path) -> Result<i64> {
    use std::os::unix::ffi::OsStrExt;
    let names = remote_git(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        65536,
    )?;
    let mut newest = 0;
    for name in names.split(|b| *b == 0).filter(|n| !n.is_empty()) {
        newest = newest.max(mtime_ms(&root.join(std::ffi::OsStr::from_bytes(name))));
    }
    let git_dir = remote_git(root, &["rev-parse", "--absolute-git-dir"], 4096)?;
    let dir = PathBuf::from(
        std::str::from_utf8(&git_dir)
            .map_err(|_| "invalid_root")?
            .trim(),
    );
    for file in ["index", "HEAD", "logs/HEAD"] {
        newest = newest.max(mtime_ms(&dir.join(file)));
    }
    Ok(newest)
}
/// Remote manual cleanup is restricted to owner-listed canonical roots. It never
/// expands discovery to other checkouts or accepts paths from the HTTP request.
pub fn remote_report(shell: &Value, settings: &Value) -> Result<Value> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::MetadataExt;
    let start = std::time::Instant::now();
    let mut entries = Vec::new();
    let roots = settings["remoteMaintenanceRoots"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if roots.len() > 16 {
        return Err("report_limit".into());
    }
    let projects = shell["projects"].as_array().ok_or("invalid_shell")?;
    let threads = shell["threads"].as_array().ok_or("invalid_shell")?;
    let mut visited = 0;
    for value in roots {
        if start.elapsed() > Duration::from_secs(2) {
            return Err("report_limit".into());
        }
        let root = PathBuf::from(value.as_str().ok_or("invalid_root")?);
        if root.canonicalize().ok().as_ref() != Some(&root) || !real_dir(&root) {
            return Err("invalid_root".into());
        }
        let associated = threads
            .iter()
            .filter(|t| t["worktreePath"].as_str() == root.to_str())
            .collect::<Vec<_>>();
        let (kind, at) = if !associated.is_empty() {
            (
                "worktree",
                associated
                    .iter()
                    .map(|t| thread_activity(t, now_ms()))
                    .max()
                    .unwrap_or(0),
            )
        } else if let Some(project) = projects
            .iter()
            .find(|p| p["workspaceRoot"].as_str() == root.to_str())
        {
            (
                "checkout",
                threads
                    .iter()
                    .filter(|t| {
                        t["projectId"] == project["id"]
                            && t["worktreePath"].as_str().is_none_or(str::is_empty)
                    })
                    .map(|t| thread_activity(t, now_ms()))
                    .max()
                    .unwrap_or(0),
            )
        } else {
            continue;
        };
        let limit = days(
            settings,
            if kind == "worktree" {
                "buildCleanupWorktreeDays"
            } else {
                "buildCleanupCheckoutDays"
            },
            if kind == "worktree" { 1 } else { 7 },
        );
        if !due(at, now_ms(), limit) {
            continue;
        }
        let mut builds = Vec::new();
        for (markers, names) in RULES {
            if !markers.iter().any(|m| root.join(m).is_file()) {
                continue;
            }
            for name in *names {
                if start.elapsed() > Duration::from_secs(2) {
                    return Err("report_limit".into());
                }
                let dir = root.join(name);
                if !real_dir(&dir) {
                    continue;
                }
                if *name == "build"
                    && markers.contains(&"CMakeLists.txt")
                    && !dir.join("CMakeCache.txt").is_file()
                {
                    continue;
                }
                // check-ignore has no content output; a nonzero result denies the candidate.
                if remote_git(&root, &["check-ignore", "-q", "--", &format!("{name}/")], 0).is_ok()
                {
                    builds.push(dir)
                }
            }
        }
        builds.sort();
        builds.dedup();
        if !due(remote_source_change(&root)?, now_ms(), limit) {
            continue;
        }
        for dir in builds {
            // Remote API deliberately excludes custom CACHEDIR.TAG directories.
            if !RULES
                .iter()
                .any(|(_, ds)| ds.iter().any(|d| dir == root.join(d)))
            {
                continue;
            }
            if entries.len() >= 32 {
                return Err("report_limit".into());
            }
            let mut stack = vec![dir.clone()];
            let mut digest = Sha256::new();
            let mut bytes = 0u64;
            let mut newest = 0i64;
            while let Some(path) = stack.pop() {
                visited += 1;
                if visited > 10_000 || start.elapsed() > Duration::from_secs(2) {
                    return Err("report_limit".into());
                }
                let meta = path.symlink_metadata().map_err(|_| "report_unavailable")?;
                newest = newest.max(
                    meta.mtime()
                        .saturating_mul(1000)
                        .saturating_add(meta.mtime_nsec() / 1_000_000),
                );
                digest.update(path.as_os_str().as_encoded_bytes());
                digest.update(format!(
                    "{}:{}:{}:{}:{}:{}:{}:{}",
                    meta.dev(),
                    meta.ino(),
                    meta.len(),
                    meta.mtime(),
                    meta.mtime_nsec(),
                    meta.mode(),
                    meta.ctime(),
                    meta.ctime_nsec()
                ));
                bytes = bytes.saturating_add(meta.blocks().saturating_mul(512));
                if meta.is_dir() {
                    let mut children = Vec::new();
                    for child in std::fs::read_dir(&path).map_err(|_| "report_unavailable")? {
                        if children.len() + visited > 10_000 {
                            return Err("report_limit".into());
                        }
                        children.push(child.map_err(|_| "report_unavailable")?.path());
                    }
                    children.sort();
                    stack.extend(children);
                }
            }
            if !due(newest, now_ms(), limit) {
                continue;
            }
            entries.push(json!({"root":root,"dir":dir,"kind":kind,"activity":at,"identity":format!("{:x}",digest.finalize()),"bytes":bytes}));
        }
    }
    Ok(
        json!({"entries":entries,"settings":{"worktreeDays":settings["buildCleanupWorktreeDays"],"checkoutDays":settings["buildCleanupCheckoutDays"]},"roots":settings["remoteMaintenanceRoots"]}),
    )
}
/// Caller re-reads native activity with its own bearer immediately before here.
pub fn remote_run(shell: &Value, settings: &Value, expected: &Value) -> Result<Value> {
    let report = remote_report(shell, settings)?;
    if &report != expected {
        return Err("stale_report".into());
    }
    let mut removed = 0;
    for entry in report["entries"].as_array().ok_or("invalid_report")? {
        // Revalidate each tree's identity before removal as earlier removals may take time.
        let fresh = remote_report(shell, settings)?;
        if !fresh["entries"]
            .as_array()
            .is_some_and(|rows| rows.contains(entry))
        {
            return Err("stale_report".into());
        }
        std::fs::remove_dir_all(text(entry, "dir")).map_err(|_| "cleanup_outcome_unknown")?;
        removed += 1;
    }
    Ok(json!({"removed":removed}))
}

#[cfg(test)]
mod remote_tests {
    use super::*;
    fn fixture() -> (PathBuf, Value, Value) {
        let root = PathBuf::from("/var/tmp").join(format!("remote-build-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("target/debug/deep")).unwrap();
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(root.join(".gitignore"), "target/\ntarget-orphan/\n").unwrap();
        std::fs::write(root.join("target/debug/deep/artifact"), "old").unwrap();
        std::fs::create_dir(root.join("target-orphan")).unwrap();
        std::fs::write(root.join("target-orphan/keep"), "untouched").unwrap();
        old(&root);
        let shell = json!({"projects":[{"id":"p","workspaceRoot":root}],"threads":[]});
        let settings = json!({"remoteMaintenanceRoots":[root],"buildCleanupCheckoutDays":1});
        (root, shell, settings)
    }
    fn old(root: &Path) {
        let time = std::fs::FileTimes::new()
            .set_modified(SystemTime::now() - Duration::from_secs(10 * 86400));
        for p in [
            "Cargo.toml",
            ".gitignore",
            ".git/HEAD",
            "target",
            "target/debug",
            "target/debug/deep",
            "target/debug/deep/artifact",
        ] {
            std::fs::File::open(root.join(p))
                .unwrap()
                .set_times(time)
                .unwrap();
        }
    }
    #[test]
    fn remote_cleanup_is_allowlisted_stale_checked_and_preserves_orphan_output() {
        let (root, shell, settings) = fixture();
        assert_eq!(
            remote_report(&shell, &json!({})).unwrap()["entries"],
            json!([])
        );
        let report = remote_report(&shell, &settings).unwrap();
        assert_eq!(report["entries"].as_array().unwrap().len(), 1);
        let artifact = root.join("target/debug/deep/artifact");
        let previous = artifact.metadata().unwrap().modified().unwrap();
        std::fs::write(&artifact, "new").unwrap();
        std::fs::File::open(&artifact)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(previous))
            .unwrap();
        assert_eq!(
            remote_run(&shell, &settings, &report),
            Err("stale_report".into()),
            "same length and restored mtime still change identity"
        );
        std::fs::write(root.join("target/debug/deep/artifact"), "new active build").unwrap();
        assert_eq!(
            remote_run(&shell, &settings, &report),
            Err("stale_report".into())
        );
        assert!(root.join("target").exists());
        old(&root);
        let report = remote_report(&shell, &settings).unwrap();
        assert_eq!(
            remote_run(&shell, &settings, &report).unwrap()["removed"],
            1
        );
        assert!(!root.join("target").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("target-orphan/keep")).unwrap(),
            "untouched"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn active_thread_and_changed_settings_prevent_remote_cleanup() {
        let (root, mut shell, mut settings) = fixture();
        let report = remote_report(&shell, &settings).unwrap();
        shell["threads"] = json!([{"id":"t","projectId":"p","activeRunId":"r"}]);
        assert_eq!(
            remote_report(&shell, &settings).unwrap()["entries"],
            json!([])
        );
        assert!(remote_run(&shell, &settings, &report).is_err());
        shell["threads"] = json!([]);
        settings["buildCleanupCheckoutDays"] = Value::Null;
        assert!(remote_run(&shell, &settings, &report).is_err());
        assert!(root.join("target").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn remote_walk_is_bounded_and_symlink_roots_are_denied() {
        let (root, shell, settings) = fixture();
        for n in 0..10001 {
            std::fs::write(root.join(format!("target/debug/deep/{n}")), "x").unwrap();
        }
        old(&root);
        assert_eq!(remote_report(&shell, &settings), Err("report_limit".into()));
        let link = root.with_extension("link");
        std::os::unix::fs::symlink(&root, &link).unwrap();
        assert!(remote_report(&shell, &json!({"remoteMaintenanceRoots":[link]})).is_err());
        std::fs::remove_file(link).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
