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
//!
//! Deletion first renames the candidate to `.o3-cleanup-<uuid>` in the same parent.
//! Interrupted quarantines block subsequent cleanup and are reported for owner
//! inspection. Recovery is manual: stop builders, inspect the reported directory,
//! then restore its original build name only when that name is unoccupied. Never
//! recursively delete an unverified leftover as part of startup or a retry.
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
    (
        &[
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
        ],
        &["build", ".gradle"],
    ),
    (&["CMakeLists.txt"], &["build"]),
    (
        &["package.json"],
        &[".next", ".nuxt", ".svelte-kit", ".turbo", ".parcel-cache"],
    ),
    (&["build.zig"], &["zig-cache", ".zig-cache"]),
];

/// `buildCleanupWorktreeDays` / `buildCleanupCheckoutDays` from the UI state: days, or
/// null to switch that kind off.
pub fn days(ui_state: &Value, key: &str, default: u64) -> Option<u64> {
    match ui_state.get(key) {
        Some(Value::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .filter(|n| (1..=365).contains(n))
                .unwrap_or(default),
        ),
        None => Some(default),
    }
}
pub fn due(last_use_ms: i64, now_ms: i64, days: Option<u64>) -> bool {
    days.is_some_and(|d| now_ms.saturating_sub(last_use_ms) >= d as i64 * DAY_MS)
}

fn ignored(root: &Path, dir: &Path) -> bool {
    let Ok(rel) = dir.strip_prefix(root) else {
        return false;
    };
    let rel = format!("{}/", rel.display());
    crate::disk_space::git(root, &["check-ignore", "-q", "--", &rel])
        .is_some_and(|o| o.status.success())
}
fn real_dir(path: &Path) -> bool {
    path.symlink_metadata().is_ok_and(|m| m.is_dir())
}

/// Component-aware overlap, including canonical aliases when available.
pub(crate) fn paths_overlap(a: &Path, b: &Path) -> bool {
    let a = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
    let b = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
    a.starts_with(&b) || b.starts_with(&a)
}
fn protected_candidate(shell: &Value, owner: &Path, dir: &Path) -> bool {
    shell["projects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["workspaceRoot"].as_str())
        .chain(
            shell["threads"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|t| t["worktreePath"].as_str()),
        )
        .filter(|p| !p.is_empty())
        .map(Path::new)
        // The owning checkout necessarily contains its build output.
        .any(|p| p != owner && paths_overlap(dir, p))
}

// No pathname-based recursive deletion: every lookup is relative to an opened
// directory, and openat2 refuses bind mounts (even on the same device), symlinks,
// and escapes. Unsupported kernels/filesystems fail closed.
mod guarded {
    use super::*;
    use std::{
        ffi::CString,
        fs::File,
        io,
        os::fd::{AsRawFd, FromRawFd},
        os::unix::{ffi::OsStrExt, fs::MetadataExt},
    };
    const RESOLVE_NO_XDEV: u64 = 0x01;
    const RESOLVE_NO_SYMLINKS: u64 = 0x04;
    const RESOLVE_BENEATH: u64 = 0x08;
    #[repr(C)]
    struct OpenHow {
        flags: u64,
        mode: u64,
        resolve: u64,
    }
    fn open(parent: &File, name: &std::ffi::OsStr) -> io::Result<File> {
        let name = CString::new(name.as_bytes())?;
        let how = OpenHow {
            flags: (libc::O_PATH | libc::O_CLOEXEC) as u64,
            mode: 0,
            resolve: RESOLVE_NO_XDEV | RESOLVE_NO_SYMLINKS | RESOLVE_BENEATH,
        };
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                parent.as_raw_fd(),
                name.as_ptr(),
                &how,
                std::mem::size_of::<OpenHow>(),
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_fd(fd as i32) })
    }
    fn mount(file: &File) -> io::Result<u64> {
        let mut stat: libc::statx = unsafe { std::mem::zeroed() };
        let rc = unsafe {
            libc::statx(
                file.as_raw_fd(),
                c"".as_ptr(),
                libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
                libc::STATX_MNT_ID,
                &mut stat,
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        if stat.stx_mask & libc::STATX_MNT_ID == 0 {
            return Err(io::Error::other("mount identity unavailable"));
        }
        Ok(stat.stx_mnt_id)
    }
    fn same_mount(expected: u64, actual: u64) -> io::Result<()> {
        if expected != actual {
            Err(io::Error::other("mount crossing"))
        } else {
            Ok(())
        }
    }
    fn names(file: &File) -> io::Result<Vec<std::ffi::OsString>> {
        let mut names = std::fs::read_dir(format!("/proc/self/fd/{}", file.as_raw_fd()))?
            .take(10_001)
            .map(|e| e.map(|e| e.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        if names.len() > 10_000 {
            return Err(io::Error::other("report_limit"));
        }
        names.sort();
        if names.iter().any(|n| n == ".git") {
            return Err(io::Error::other("nested Git checkout"));
        }
        Ok(names)
    }
    struct Node {
        file: File,
        name: std::ffi::OsString,
        children: Vec<Node>,
    }
    pub(super) struct Tree {
        root: PathBuf,
        parent: File,
        node: Node,
        pub identity: String,
        pub bytes: u64,
        pub newest: i64,
    }
    #[allow(clippy::too_many_arguments)]
    fn walk(
        parent: &File,
        name: std::ffi::OsString,
        mount_id: u64,
        digest: &mut sha2::Sha256,
        bytes: &mut u64,
        newest: &mut i64,
        visited: &mut usize,
        start: std::time::Instant,
        depth: usize,
    ) -> io::Result<Node> {
        use sha2::Digest;
        *visited += 1;
        if depth > 128 || *visited > 10_000 || start.elapsed() > Duration::from_secs(2) {
            return Err(io::Error::other("report_limit"));
        }
        let file = open(parent, &name)?;
        same_mount(mount_id, mount(&file)?)?;
        let meta = file.metadata()?;
        if !meta.is_dir() && !meta.is_file() {
            return Err(io::Error::other("unsupported entry"));
        }
        digest.update(name.as_bytes());
        digest.update(format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}",
            mount_id,
            meta.dev(),
            meta.ino(),
            meta.len(),
            meta.mtime(),
            meta.mtime_nsec(),
            meta.mode(),
            meta.ctime(),
            meta.ctime_nsec()
        ));
        *bytes = bytes.saturating_add(meta.blocks().saturating_mul(512));
        *newest = (*newest).max(
            meta.mtime()
                .saturating_mul(1000)
                .saturating_add(meta.mtime_nsec() / 1_000_000),
        );
        let mut children = Vec::new();
        if meta.is_dir() {
            for child in names(&file)? {
                children.push(walk(
                    &file,
                    child,
                    mount_id,
                    digest,
                    bytes,
                    newest,
                    visited,
                    start,
                    depth + 1,
                )?);
            }
        }
        Ok(Node {
            file,
            name,
            children,
        })
    }
    pub(super) fn scan(root: &Path, dir: &Path) -> io::Result<Tree> {
        scan_bounded(root, dir, &mut 0, std::time::Instant::now())
    }
    pub(super) fn scan_bounded(
        root: &Path,
        dir: &Path,
        visited: &mut usize,
        start: std::time::Instant,
    ) -> io::Result<Tree> {
        use sha2::Digest;
        if dir
            .file_name()
            .is_some_and(|n| n.as_bytes().starts_with(b".o3-cleanup-"))
            || dir.parent() != Some(root)
        {
            return Err(io::Error::other("invalid candidate"));
        }
        let path = CString::new(root.as_os_str().as_bytes())?;
        let how = OpenHow {
            flags: (libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC) as u64,
            mode: 0,
            resolve: RESOLVE_NO_SYMLINKS,
        };
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                libc::AT_FDCWD,
                path.as_ptr(),
                &how,
                std::mem::size_of::<OpenHow>(),
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let parent = unsafe { File::from_raw_fd(fd as i32) };
        let mut digest = sha2::Sha256::new();
        let (mut bytes, mut newest) = (0, 0);
        let node = walk(
            &parent,
            dir.file_name()
                .ok_or_else(|| io::Error::other("invalid candidate"))?
                .to_owned(),
            mount(&parent)?,
            &mut digest,
            &mut bytes,
            &mut newest,
            visited,
            start,
            0,
        )?;
        Ok(Tree {
            root: root.to_path_buf(),
            parent,
            node,
            identity: format!("{:x}", digest.finalize()),
            bytes,
            newest,
        })
    }
    fn remove(parent: &File, node: &Node) -> io::Result<()> {
        let current = open(parent, &node.name)?;
        let a = current.metadata()?;
        let b = node.file.metadata()?;
        same_mount(mount(&node.file)?, mount(&current)?)?;
        if a.dev() != b.dev() || a.ino() != b.ino() {
            return Err(io::Error::other("replaced directory entry"));
        }
        if b.is_dir() {
            if names(&node.file)?
                != node
                    .children
                    .iter()
                    .map(|n| n.name.clone())
                    .collect::<Vec<_>>()
            {
                return Err(io::Error::other("changed directory"));
            }
            for child in &node.children {
                remove(&node.file, child)?;
            }
            if !names(&node.file)?.is_empty() {
                return Err(io::Error::other("changed directory"));
            }
        }
        let name = CString::new(node.name.as_bytes())?;
        let rc = unsafe {
            libc::unlinkat(
                parent.as_raw_fd(),
                name.as_ptr(),
                if b.is_dir() { libc::AT_REMOVEDIR } else { 0 },
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    const QUARANTINE_PREFIX: &str = ".o3-cleanup-";

    // Leftovers require an explicit owner decision; never adopt them as candidates.
    pub(super) fn leftovers(root: &Path) -> io::Result<Vec<PathBuf>> {
        std::fs::read_dir(root)?
            .filter_map(|entry| match entry {
                Ok(entry)
                    if entry
                        .file_name()
                        .as_bytes()
                        .starts_with(QUARANTINE_PREFIX.as_bytes()) =>
                {
                    Some(Ok(entry.path()))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }
    fn rename(parent: &File, from: &std::ffi::OsStr, to: &std::ffi::OsStr) -> io::Result<()> {
        let from = CString::new(from.as_bytes())?;
        let to = CString::new(to.as_bytes())?;
        let rc = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                parent.as_raw_fd(),
                from.as_ptr(),
                parent.as_raw_fd(),
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    fn same_entry(a: &File, b: &File) -> io::Result<()> {
        let a_meta = a.metadata()?;
        let b_meta = b.metadata()?;
        same_mount(mount(a)?, mount(b)?)?;
        if a_meta.dev() != b_meta.dev() || a_meta.ino() != b_meta.ino() {
            return Err(io::Error::other("replaced directory entry"));
        }
        Ok(())
    }
    // Validate the entire snapshot before the first unlink, including nested Git.
    fn validate(parent: &File, node: &Node) -> io::Result<()> {
        let current = open(parent, &node.name)?;
        same_entry(&current, &node.file)?;
        if current.metadata()?.is_dir() {
            if names(&current)?
                != node
                    .children
                    .iter()
                    .map(|n| n.name.clone())
                    .collect::<Vec<_>>()
            {
                return Err(io::Error::other("changed directory"));
            }
            for child in &node.children {
                validate(&current, child)?;
            }
        }
        Ok(())
    }
    impl Tree {
        #[cfg(test)]
        pub fn remove(self) -> io::Result<()> {
            self.remove_checked(|_| Ok(()))
        }
        pub fn remove_checked(self, check: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<()> {
            self.remove_hooked(|| {}, check)
        }
        pub(super) fn remove_hooked(
            mut self,
            before_rename: impl FnOnce(),
            check: impl FnOnce(&Path) -> io::Result<()>,
        ) -> io::Result<()> {
            before_rename();
            let original = self.node.name.clone();
            let quarantine =
                std::ffi::OsString::from(format!("{QUARANTINE_PREFIX}{}", uuid::Uuid::new_v4()));
            // Atomic ownership transfer relative to the validated parent. ENOSYS and
            // unsupported flags fail closed: there is no pathname deletion fallback.
            rename(&self.parent, &original, &quarantine).map_err(|error| {
                io::Error::other(format!("no deletion: quarantine rename failed: {error}"))
            })?;
            self.node.name = quarantine.clone();
            let moved = open(&self.parent, &quarantine);
            let verified: io::Result<()> = (|| {
                let reopened = moved
                    .as_ref()
                    .map_err(|e| io::Error::other(e.to_string()))?
                    .try_clone()?;
                same_entry(&self.node.file, &reopened)?;
                self.node.file = reopened;
                check(&self.root.join(&quarantine))?;
                validate(&self.parent, &self.node)?;
                Ok(())
            })();
            if let Err(error) = verified {
                // Do not overwrite a new build, or move an unrelated replacement.
                if open(&self.parent, &quarantine).is_ok_and(|f| {
                    moved
                        .as_ref()
                        .is_ok_and(|moved| same_entry(moved, &f).is_ok())
                }) {
                    let _ = rename(&self.parent, &quarantine, &original);
                }
                return Err(io::Error::other(format!(
                    "no deletion: {error}; inspect {}",
                    self.root.join(&quarantine).display()
                )));
            }
            // Children are resolved only beneath the reopened quarantine descriptor.
            // An interruption leaves a discoverable quarantine for manual recovery.
            remove(&self.parent, &self.node).map_err(|error| {
                io::Error::other(format!(
                    "cleanup_outcome_unknown: {error}; inspect {}",
                    self.root.join(&quarantine).display()
                ))
            })
        }
    }
    #[cfg(test)]
    #[test]
    fn same_device_different_mount_is_refused() {
        assert!(same_mount(100, 101).is_err());
        assert!(same_mount(100, 100).is_ok());
    }
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
            if *d == "build"
                && markers.contains(&"CMakeLists.txt")
                && !path.join("CMakeCache.txt").is_file()
            {
                continue;
            }
            found.push(path);
        }
    }
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if entry.file_name() != ".git"
            && !entry
                .file_name()
                .as_encoded_bytes()
                .starts_with(b".o3-cleanup-")
            && entry.path().join("CACHEDIR.TAG").is_file()
        {
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
    if let Some(out) = crate::disk_space::git(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )
    .filter(|o| o.status.success())
    {
        for name in out
            .stdout
            .split(|b| *b == 0)
            .filter(|n| !n.is_empty())
            .take(MAX_FILES)
        {
            newest = newest.max(mtime_ms(&root.join(String::from_utf8_lossy(name).as_ref())));
        }
    }
    if let Some(out) = crate::disk_space::git(root, &["rev-parse", "--absolute-git-dir"])
        .filter(|o| o.status.success())
    {
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
    [
        "updatedAt",
        "latestRunCompletedAt",
        "latestUserAuthoredMessageAt",
        "createdAt",
    ]
    .iter()
    .map(|k| epoch(&t[*k]))
    .max()
    .unwrap_or(0)
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
        let Some(root) = p["workspaceRoot"].as_str().filter(|r| !r.is_empty()) else {
            continue;
        };
        let root = PathBuf::from(root);
        if out.contains_key(&root) {
            continue;
        }
        let activity = threads
            .iter()
            .filter(|t| {
                t["projectId"] == p["id"] && t["worktreePath"].as_str().is_none_or(str::is_empty)
            })
            .map(|t| thread_activity(t, now))
            .max()
            .unwrap_or(0);
        out.insert(root, ("checkout", activity));
    }
    out
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// One cleanup pass; returns what was removed (or, with `dry_run`, what would be).
pub fn run(client: &Client, ui_state: &Value, dry_run: bool) -> Result<Value> {
    let shell = crate::disk_space::local_shell(client)?;
    let now = now_ms();
    let limits = [
        (
            "worktree",
            days(ui_state, "buildCleanupWorktreeDays", DEFAULT_WORKTREE_DAYS),
        ),
        (
            "checkout",
            days(ui_state, "buildCleanupCheckoutDays", DEFAULT_CHECKOUT_DAYS),
        ),
    ];
    let (mut entries, mut failed, mut freed) = (vec![], vec![], 0u64);
    let known = folders(&shell, now);
    for (root, (kind, activity)) in &known {
        let (kind, activity) = (*kind, *activity);
        let limit = limits
            .iter()
            .find(|(k, _)| *k == kind)
            .and_then(|(_, d)| *d);
        match guarded::leftovers(root) {
            Ok(paths) if !paths.is_empty() => {
                failed.push(
                    json!({"path":root,"error":"cleanup_recovery_required","quarantines":paths}),
                );
                continue;
            }
            Err(error) => {
                failed.push(json!({"path":root,"error":error.to_string()}));
                continue;
            }
            _ => {}
        }
        // Cheap checks first: thread activity, then whether there is anything to clean.
        if !due(activity, now, limit) || !real_dir(root) {
            continue;
        }
        let builds = build_dirs(root);
        if builds.is_empty() || !due(last_file_change(root, &builds), now, limit) {
            continue;
        }
        for dir in builds {
            if protected_candidate(&shell, root, &dir)
                || known.keys().any(|p| p != root && paths_overlap(&dir, p))
            {
                continue;
            }
            let Ok(tree) = guarded::scan(root, &dir) else {
                continue;
            };
            if !due(tree.newest, now, limit) {
                continue;
            }
            let bytes = tree.bytes;
            if dry_run {
                freed += bytes;
                entries.push(json!({"path":root,"dir":dir,"kind":kind,"bytes":bytes}));
                continue;
            }
            let fresh_shell = crate::disk_space::local_shell(client)?;
            if protected_candidate(&fresh_shell, root, &dir)
                || folders(&fresh_shell, now_ms())
                    .keys()
                    .any(|p| p != root && paths_overlap(&dir, p))
            {
                continue;
            }
            let fresh = guarded::scan(root, &dir).map_err(|_| "stale_report")?;
            if fresh.identity != tree.identity {
                continue;
            }
            match fresh.remove_checked(|quarantine| {
                if protected_candidate(&fresh_shell, root, &dir)
                    || protected_candidate(&fresh_shell, root, quarantine)
                    || folders(&fresh_shell, now_ms()).keys().any(|p| {
                        p != root && (paths_overlap(&dir, p) || paths_overlap(quarantine, p))
                    })
                {
                    return Err(std::io::Error::other("protected root overlap"));
                }
                Ok(())
            }) {
                Ok(()) => {
                    eprintln!(
                        "ot3: removed idle build output {} ({bytes} bytes)",
                        dir.display()
                    );
                    freed += bytes;
                    entries.push(json!({"path":root,"dir":dir,"kind":kind,"bytes":bytes}));
                }
                Err(error) => failed
                    .push(json!({"path":root,"dir":dir,"kind":kind,"error":error.to_string()})),
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
        let root =
            PathBuf::from("/var/tmp").join(format!("t3-build-cleanup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        root
    }
    #[test]
    fn days_default_and_off() {
        assert_eq!(days(&json!({}), "buildCleanupWorktreeDays", 1), Some(1));
        assert_eq!(
            days(
                &json!({"buildCleanupCheckoutDays":3}),
                "buildCleanupCheckoutDays",
                7
            ),
            Some(3)
        );
        assert_eq!(
            days(
                &json!({"buildCleanupCheckoutDays":0}),
                "buildCleanupCheckoutDays",
                7
            ),
            Some(7)
        );
        assert_eq!(
            days(
                &json!({"buildCleanupWorktreeDays":null}),
                "buildCleanupWorktreeDays",
                1
            ),
            None
        );
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
        std::fs::write(
            root.join("cache/CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55",
        )
        .unwrap();
        // …but only when git ignores it.
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets/CACHEDIR.TAG"), "x").unwrap();
        // Symlinks are never followed.
        std::os::unix::fs::symlink(root.join("target"), root.join("linked")).unwrap();
        assert_eq!(
            build_dirs(&root),
            vec![root.join("cache"), root.join("target")]
        );
        std::fs::write(root.join("build/CMakeCache.txt"), "").unwrap();
        assert_eq!(
            build_dirs(&root),
            vec![root.join("build"), root.join("cache"), root.join("target")]
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn building_or_editing_counts_as_use() {
        let root = repo();
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        let builds = build_dirs(&root);
        assert!(
            now_ms() - last_file_change(&root, &builds) < 60_000,
            "fresh files are recent use"
        );
        let old = std::fs::FileTimes::new()
            .set_modified(SystemTime::now() - Duration::from_secs(3 * 86400));
        for p in ["Cargo.toml", ".gitignore", "target", "target/debug"] {
            std::fs::File::open(root.join(p))
                .unwrap()
                .set_times(old)
                .unwrap();
        }
        let git_dir = root.join(".git");
        for f in ["HEAD"] {
            std::fs::File::open(git_dir.join(f))
                .unwrap()
                .set_times(old)
                .unwrap();
        }
        assert!(
            due(last_file_change(&root, &builds), now_ms(), Some(2)),
            "untouched for 3 days"
        );
        std::fs::write(root.join("target/debug/new"), "").unwrap();
        std::fs::File::open(root.join("target"))
            .unwrap()
            .set_times(old)
            .unwrap();
        assert!(
            !due(last_file_change(&root, &builds), now_ms(), Some(2)),
            "a fresh build is use"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn thread_folders_carry_their_latest_activity() {
        let shell = json!({"projects":[{"id":"p","workspaceRoot":"/w/main"}],"threads":[
            {"id":"a","projectId":"p","worktreePath":"/w/tree","updatedAt":"2026-10-01T00:00:00Z"},
            {"id":"b","projectId":"p","worktreePath":null,"updatedAt":"2026-10-02T00:00:00Z"},
            {"id":"c","projectId":"p","worktreePath":"/w/busy","activeRunId":"r","updatedAt":"2026-01-01T00:00:00Z"}]});
        let f = folders(&shell, 42);
        assert_eq!(
            f[&PathBuf::from("/w/tree")],
            ("worktree", epoch(&json!("2026-10-01T00:00:00Z")))
        );
        assert_eq!(
            f[&PathBuf::from("/w/main")],
            ("checkout", epoch(&json!("2026-10-02T00:00:00Z")))
        );
        assert_eq!(
            f[&PathBuf::from("/w/busy")].1,
            42,
            "a running thread is in use now"
        );
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
    let start = std::time::Instant::now();
    let mut entries = Vec::new();
    let mut visited = 0;
    let roots = settings["remoteMaintenanceRoots"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if roots.len() > 16 {
        return Err("report_limit".into());
    }
    let projects = shell["projects"].as_array().ok_or("invalid_shell")?;
    let threads = shell["threads"].as_array().ok_or("invalid_shell")?;
    for value in roots {
        if start.elapsed() > Duration::from_secs(2) {
            return Err("report_limit".into());
        }
        let root = PathBuf::from(value.as_str().ok_or("invalid_root")?);
        if root.canonicalize().ok().as_ref() != Some(&root) || !real_dir(&root) {
            return Err("invalid_root".into());
        }
        let quarantines = guarded::leftovers(&root).map_err(|_| "invalid_root")?;
        if !quarantines.is_empty() {
            return Err(format!(
                "cleanup_recovery_required: {}",
                quarantines
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
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
            if protected_candidate(shell, &root, &dir) {
                continue;
            }
            let tree = guarded::scan_bounded(&root, &dir, &mut visited, start).map_err(|e| {
                if e.to_string() == "report_limit" {
                    "report_limit"
                } else {
                    "unsafe_build_tree"
                }
            })?;
            let (newest, bytes, identity) = (tree.newest, tree.bytes, tree.identity);
            if !due(newest, now_ms(), limit) {
                continue;
            }
            entries.push(json!({"root":root,"dir":dir,"kind":kind,"activity":at,"identity":identity,"bytes":bytes}));
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
        let root = Path::new(text(entry, "root"));
        let dir = Path::new(text(entry, "dir"));
        if protected_candidate(shell, root, dir) {
            return Err("stale_report".into());
        }
        let tree = guarded::scan(root, dir).map_err(|_| "stale_report")?;
        if tree.identity != text(entry, "identity") {
            return Err("stale_report".into());
        }
        tree.remove_checked(|quarantine| {
            if protected_candidate(shell, root, dir) || protected_candidate(shell, root, quarantine)
            {
                return Err(std::io::Error::other("protected root overlap"));
            }
            Ok(())
        })
        .map_err(|error| format!("cleanup_stopped: {error}"))?;
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
    fn nested_project_and_active_thread_are_protected_without_git_marker() {
        let (root, mut shell, settings) = fixture();
        let report = remote_report(&shell, &settings).unwrap();
        shell["threads"] = json!([{"id":"nested","projectId":"p","worktreePath":root.join("target/debug"),"activeRunId":"run"}]);
        assert_eq!(
            remote_report(&shell, &settings).unwrap()["entries"],
            json!([])
        );
        assert!(remote_run(&shell, &settings, &report).is_err());
        shell["threads"] = json!([]);
        shell["projects"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"nested","workspaceRoot":root.join("target")}));
        assert_eq!(
            remote_report(&shell, &settings).unwrap()["entries"],
            json!([])
        );
        assert!(protected_candidate(&shell, &root, &root.join("target")));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn nested_git_directory_and_worktree_file_refuse_scan_and_delete() {
        for directory in [true, false] {
            let (root, shell, settings) = fixture();
            let report = remote_report(&shell, &settings).unwrap();
            let tree = guarded::scan(&root, &root.join("target")).unwrap();
            let marker = root.join("target/debug/deep/.git");
            if directory {
                std::fs::create_dir(&marker).unwrap();
            } else {
                std::fs::write(&marker, "gitdir: /not-followed").unwrap();
            }
            assert!(remote_report(&shell, &settings).is_err());
            assert!(remote_run(&shell, &settings, &report).is_err());
            assert!(tree.remove().is_err());
            assert!(root.join("target/debug/deep/artifact").exists());
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn symlink_inside_output_is_refused() {
        let (root, shell, settings) = fixture();
        let report = remote_report(&shell, &settings).unwrap();
        std::os::unix::fs::symlink(root.join("Cargo.toml"), root.join("target/debug/link"))
            .unwrap();
        assert!(guarded::scan(&root, &root.join("target")).is_err());
        assert!(remote_run(&shell, &settings, &report).is_err());
        assert!(root.join("Cargo.toml").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn replaced_directory_between_scan_and_delete_preserves_replacement() {
        let (root, shell, settings) = fixture();
        let report = remote_report(&shell, &settings).unwrap();
        let tree = guarded::scan(&root, &root.join("target")).unwrap();
        std::fs::rename(root.join("target"), root.join("saved-output")).unwrap();
        std::fs::create_dir(root.join("target")).unwrap();
        std::fs::write(root.join("target/source.rs"), "precious").unwrap();
        assert!(tree.remove().is_err());
        assert!(remote_run(&shell, &settings, &report).is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("target/source.rs")).unwrap(),
            "precious"
        );
        assert!(root.join("saved-output/debug/deep/artifact").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn replaced_child_directory_is_not_traversed_during_deletion() {
        let (root, _, _) = fixture();
        let tree = guarded::scan(&root, &root.join("target")).unwrap();
        std::fs::rename(root.join("target/debug"), root.join("saved-output")).unwrap();
        std::fs::create_dir(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/debug/source.rs"), "precious").unwrap();
        assert!(tree.remove().is_err());
        assert!(root.join("target/debug/source.rs").exists());
        assert!(root.join("saved-output/deep/artifact").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn symlink_replacement_after_scan_is_not_followed_during_deletion() {
        let (root, _, _) = fixture();
        let tree = guarded::scan(&root, &root.join("target")).unwrap();
        std::fs::rename(root.join("target"), root.join("saved-output")).unwrap();
        std::os::unix::fs::symlink(root.join("saved-output"), root.join("target")).unwrap();
        assert!(tree.remove().is_err());
        assert!(root.join("saved-output/debug/deep/artifact").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rename_after_validation_never_traverses_moved_tree() {
        for replacement in [false, true] {
            let (root, _, _) = fixture();
            let outside = root.with_extension("outside");
            std::fs::create_dir(&outside).unwrap();
            let saved = outside.join("saved-output");
            let tree = guarded::scan(&root, &root.join("target")).unwrap();
            let result = tree.remove_hooked(
                || {
                    std::fs::rename(root.join("target"), &saved).unwrap();
                    if replacement {
                        std::fs::create_dir(root.join("target")).unwrap();
                        std::fs::write(root.join("target/precious"), "keep").unwrap();
                    }
                },
                |_| Ok(()),
            );
            assert!(result.is_err());
            assert_eq!(
                std::fs::read_to_string(saved.join("debug/deep/artifact")).unwrap(),
                "old"
            );
            if replacement {
                assert_eq!(
                    std::fs::read_to_string(root.join("target/precious")).unwrap(),
                    "keep"
                );
            }
            assert!(guarded::leftovers(&root).unwrap().is_empty());
            std::fs::remove_dir_all(root).unwrap();
            std::fs::remove_dir_all(outside).unwrap();
        }
    }
    #[test]
    fn interrupted_quarantine_is_reported_and_manually_recoverable() {
        let (root, shell, settings) = fixture();
        std::fs::write(root.join(".gitignore"), "target/\n.o3-cleanup-*/\n").unwrap();
        std::fs::write(root.join("target/CACHEDIR.TAG"), "cache").unwrap();
        let tree = guarded::scan(&root, &root.join("target")).unwrap();
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tree.remove_checked(|_| panic!("simulated interruption"))
        }));
        assert!(interrupted.is_err());
        let leftovers = guarded::leftovers(&root).unwrap();
        assert_eq!(leftovers.len(), 1);
        assert!(leftovers[0].join("debug/deep/artifact").exists());
        assert!(remote_report(&shell, &settings)
            .unwrap_err()
            .contains("cleanup_recovery_required"));
        assert!(remote_report(&shell, &settings)
            .unwrap_err()
            .contains(leftovers[0].to_str().unwrap()));
        assert!(guarded::scan(&root, &leftovers[0]).is_err());
        assert!(build_dirs(&root).is_empty());
        // Recovery is an explicit owner action, never an automatic retry.
        std::fs::rename(&leftovers[0], root.join("target")).unwrap();
        assert!(guarded::leftovers(&root).unwrap().is_empty());
        assert_eq!(
            std::fs::read_to_string(root.join("target/debug/deep/artifact")).unwrap(),
            "old"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn quarantine_git_and_overlap_rechecks_restore_without_deletion() {
        for nested_git in [false, true] {
            let (root, mut shell, _) = fixture();
            let tree = guarded::scan(&root, &root.join("target")).unwrap();
            let result = tree.remove_checked(|quarantine| {
                if nested_git {
                    std::fs::write(quarantine.join("debug/.git"), "gitdir: /protected").unwrap();
                } else {
                    shell["projects"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"workspaceRoot":quarantine.join("debug")}));
                    if protected_candidate(&shell, &root, quarantine) {
                        return Err(std::io::Error::other("protected root overlap"));
                    }
                }
                Ok(())
            });
            assert!(result.is_err());
            assert!(root.join("target/debug/deep/artifact").exists());
            assert!(guarded::leftovers(&root).unwrap().is_empty());
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn failed_verification_never_overwrites_new_original_name() {
        let (root, _, _) = fixture();
        let tree = guarded::scan(&root, &root.join("target")).unwrap();
        assert!(tree
            .remove_checked(|_| {
                std::fs::create_dir(root.join("target")).unwrap();
                std::fs::write(root.join("target/precious"), "keep").unwrap();
                Err(std::io::Error::other("stop before deletion"))
            })
            .is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("target/precious")).unwrap(),
            "keep"
        );
        let leftovers = guarded::leftovers(&root).unwrap();
        assert_eq!(leftovers.len(), 1);
        assert!(leftovers[0].join("debug/deep/artifact").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bind_mount_same_filesystem_is_refused_when_permitted() {
        use std::os::unix::ffi::OsStrExt;
        let (root, shell, settings) = fixture();
        let source = root.join("mount-source");
        let dest = root.join("target/debug/deep/mount");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&dest).unwrap();
        std::fs::write(source.join("source.rs"), "precious").unwrap();
        let src = std::ffi::CString::new(source.as_os_str().as_bytes()).unwrap();
        let dst = std::ffi::CString::new(dest.as_os_str().as_bytes()).unwrap();
        let tree = guarded::scan(&root, &root.join("target")).unwrap();
        let rc = unsafe {
            libc::mount(
                src.as_ptr(),
                dst.as_ptr(),
                std::ptr::null(),
                libc::MS_BIND,
                std::ptr::null(),
            )
        };
        if rc != 0 {
            let error = std::io::Error::last_os_error();
            assert!(
                matches!(
                    error.raw_os_error(),
                    Some(libc::EPERM | libc::EACCES | libc::ENOSYS)
                ),
                "{error}"
            );
            eprintln!(
                "bind mount unavailable: {error}; simulated mount-ID regression covers refusal"
            );
        } else {
            // Always unmount before assertions or fixture teardown.
            let denied = guarded::scan(&root, &root.join("target")).is_err();
            let report_denied = remote_report(&shell, &settings).is_err();
            let deletion_denied = tree.remove().is_err();
            assert_eq!(unsafe { libc::umount2(dst.as_ptr(), 0) }, 0);
            assert!(denied && report_denied && deletion_denied);
            assert!(source.join("source.rs").exists());
        }
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
