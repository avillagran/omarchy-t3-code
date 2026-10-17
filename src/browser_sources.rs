//! Linux port of T3 BrowserImport/Sources.ts at 5a4e998. Discovery only:
//! no cookies or keys are returned, changed, or imported by this module.
use crate::{json, text, Result};
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};
#[derive(Clone, Copy)]
struct Definition {
    id: &'static str,
    name: &'static str,
    root: Option<&'static str>,
    firefox: bool,
}
const SOURCES: [Definition; 9] = [
    Definition {
        id: "chrome",
        name: "Chrome",
        root: Some(".config/google-chrome"),
        firefox: false,
    },
    Definition {
        id: "edge",
        name: "Microsoft Edge",
        root: Some(".config/microsoft-edge"),
        firefox: false,
    },
    Definition {
        id: "brave",
        name: "Brave",
        root: Some(".config/BraveSoftware/Brave-Browser"),
        firefox: false,
    },
    Definition {
        id: "vivaldi",
        name: "Vivaldi",
        root: Some(".config/vivaldi"),
        firefox: false,
    },
    Definition {
        id: "opera",
        name: "Opera",
        root: Some(".config/opera"),
        firefox: false,
    },
    Definition {
        id: "arc",
        name: "Arc",
        root: None,
        firefox: false,
    },
    Definition {
        id: "helium",
        name: "Helium",
        root: Some(".config/net.imput.helium"),
        firefox: false,
    },
    Definition {
        id: "safari",
        name: "Safari",
        root: None,
        firefox: false,
    },
    Definition {
        id: "firefox",
        name: "Firefox",
        root: Some(".mozilla/firefox"),
        firefox: true,
    },
];
fn bounded_text(path: &Path) -> Option<String> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = vec![];
    file.take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 2 * 1024 * 1024 {
        return None;
    }
    String::from_utf8(bytes).ok()
}
fn segment(s: &str) -> bool {
    !s.is_empty() && !matches!(s, "." | "..") && !s.contains(['/', '\\', '\0'])
}
fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                if !out.pop() && !path.is_absolute() {
                    return None;
                }
            }
            Component::CurDir => {}
            c => out.push(c.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".")
    }
    Some(out)
}
/// Original ProfileN blocks only; explicit absolute Firefox roots are valid.
fn trim(s: &str) -> &str {
    s.trim_matches(|c|matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}'))
}
pub fn parse_firefox_profiles(ini: &str) -> Vec<Value> {
    fn flush(current: &mut Option<Value>, out: &mut Vec<Value>) {
        if let Some(row) = current.take() {
            let raw = text(&row, "path");
            let relative = row.get("isrelative").and_then(Value::as_str).unwrap_or("1");
            if raw.is_empty() || raw.contains('\0') || !matches!(relative, "0" | "1") {
                return;
            }
            let path = Path::new(raw);
            if (relative == "1") == path.is_absolute() {
                return;
            }
            if let Some(directory) = normalize(path) {
                let directory = directory.to_string_lossy();
                let name = trim(text(&row, "name"));
                out.push(json!({"directory":directory,"name":if name.is_empty(){directory.as_ref()}else{name}}))
            }
        }
    }
    let mut out = vec![];
    let mut current = None;
    for raw in ini.lines() {
        let line = trim(raw);
        if line.starts_with('[') {
            flush(&mut current, &mut out);
            let lower = line.to_ascii_lowercase();
            if let Some(n) = lower
                .strip_prefix("[profile")
                .and_then(|s| s.strip_suffix(']'))
            {
                if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) {
                    current = Some(json!({}))
                }
            }
            continue;
        }
        if let (Some(row), Some((key, value))) = (current.as_mut(), line.split_once('=')) {
            let key = trim(key).to_ascii_lowercase();
            if matches!(key.as_str(), "name" | "path" | "isrelative") {
                row[key] = json!(trim(value))
            }
        }
    }
    flush(&mut current, &mut out);
    out
}
fn database(root: &Path, directory: &str, firefox: bool) -> Option<PathBuf> {
    let profile = root.join(directory);
    let candidates = if firefox {
        vec![profile.join("cookies.sqlite")]
    } else {
        vec![profile.join("Network/Cookies"), profile.join("Cookies")]
    };
    candidates.into_iter().find(|p| p.is_file())
}
fn entries(root: &Path) -> Vec<String> {
    let mut out = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect::<Vec<_>>();
    out.sort();
    out.truncate(4096);
    out
}
fn chromium_order(raw: &str) -> Vec<String> {
    use serde::{
        de::{MapAccess, Visitor},
        Deserialize, Deserializer,
    };
    #[derive(Deserialize)]
    struct Local {
        profile: Option<Profiles>,
    }
    #[derive(Deserialize)]
    struct Profiles {
        #[serde(default, deserialize_with = "ordered")]
        info_cache: Vec<(String, Value)>,
    }
    fn ordered<'de, D: Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Vec<(String, Value)>, D::Error> {
        struct Entries;
        impl<'de> Visitor<'de> for Entries {
            type Value = Vec<(String, Value)>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("profile records")
            }
            fn visit_map<M: MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Self::Value, M::Error> {
                let mut out = Vec::<(String, Value)>::new();
                while let Some((key, value)) = map.next_entry::<String, Value>()? {
                    if let Some(prior) = out.iter_mut().find(|(k, _)| k == &key) {
                        prior.1 = value
                    } else {
                        out.push((key, value))
                    }
                }
                Ok(out)
            }
        }
        d.deserialize_map(Entries)
    }
    let mut keys = serde_json::from_str::<Local>(raw)
        .ok()
        .and_then(|s| s.profile)
        .map(|p| p.info_cache.into_iter().map(|(k, _)| k).collect::<Vec<_>>())
        .unwrap_or_default();
    // Object.entries enumerates canonical array indices first, then insertion order.
    fn index(s: &str) -> Option<u32> {
        let n = s.parse::<u32>().ok()?;
        (n < u32::MAX && n.to_string() == s).then_some(n)
    }
    keys.sort_by(|a, b| match (index(a), index(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    });
    keys
}
fn directory_profiles(root: &Path, firefox: bool) -> Vec<Value> {
    let local_state = if firefox {
        None
    } else {
        bounded_text(&root.join("Local State"))
    };
    let declared = if firefox {
        bounded_text(&root.join("profiles.ini"))
            .map(|s| parse_firefox_profiles(&s))
            .unwrap_or_default()
            .into_iter()
            .filter(|p| database(root, text(p, "directory"), true).is_some())
            .collect::<Vec<_>>()
    } else {
        local_state
            .as_ref()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .and_then(|s| {
                // Match the original schema: one malformed record invalidates metadata and triggers disk fallback.
                let Some(profile) = s.get("profile") else {
                    return Some(vec![]);
                };
                let profile = profile.as_object()?;
                let Some(cache) = profile.get("info_cache") else {
                    return Some(vec![]);
                };
                let cache = cache.as_object()?;
                if cache
                    .values()
                    .any(|v| !v.is_object() || v.get("name").is_some_and(|n| !n.is_string()))
                {
                    return None;
                }
                Some(
                    chromium_order(local_state.as_ref().unwrap())
                        .into_iter()
                        .filter(|dir| segment(dir))
                        .map(|dir| {
                            let info = &cache[&dir];
                            let name = trim(text(info, "name"));
                            json!({"directory":dir,"name":if name.is_empty(){dir.as_str()}else{name}})
                        })
                        .collect(),
                )
            })
            .unwrap_or_default()
    };
    let mut profiles = if declared.is_empty() {
        entries(root)
            .into_iter()
            .filter(|dir| (firefox || segment(dir)) && database(root, dir, firefox).is_some())
            .map(|dir| json!({"directory":dir,"name":dir}))
            .collect()
    } else {
        declared
    };
    for profile in &mut profiles {
        if let Some(path) = database(root, text(profile, "directory"), firefox) {
            if let Some(count) = cookie_count(&path, firefox) {
                profile["cookieCount"] = json!(count)
            }
        }
    }
    profiles
}
fn profiles(home: &Path, d: Definition) -> Vec<Value> {
    let Some(relative) = d.root else {
        return vec![];
    };
    let root = home.join(relative);
    if !d.firefox {
        return directory_profiles(&root, false);
    }
    let mut out = vec![];
    let mut seen = HashSet::new();
    for source in [&root, &home.join("snap/firefox/common/.mozilla/firefox")] {
        for mut p in directory_profiles(source, true) {
            let Some(absolute) = normalize(&source.join(text(&p, "directory"))) else {
                continue;
            };
            if seen.insert(absolute.clone()) {
                if source != &root {
                    p["directory"] = json!(absolute)
                }
                out.push(p)
            }
        }
    }
    out
}
fn alive(pid: &str) -> bool {
    let Ok(pid) = pid.parse::<i32>() else {
        return true;
    };
    if pid <= 0 {
        return true;
    }
    unsafe {
        libc::kill(pid, 0) == 0
            || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
}
pub fn chromium_lock_held(target: &str, hostname: &str) -> bool {
    let Some((host, pid)) = target.rsplit_once('-') else {
        return true;
    };
    host.is_empty() || host != hostname || !pid.bytes().all(|b| b.is_ascii_digit()) || alive(pid)
}
pub fn firefox_lock_held(target: &str, addresses: &HashSet<String>) -> bool {
    let Some((host, pid)) = target.rsplit_once(':') else {
        return true;
    };
    let pid = pid.strip_prefix('+').unwrap_or(pid);
    !addresses.contains(host) || !pid.bytes().all(|b| b.is_ascii_digit()) || alive(pid)
}
fn symlink_held(path: &Path, check: impl FnOnce(&str) -> bool) -> bool {
    match fs::read_link(path) {
        Ok(target) => target.to_str().is_none_or(check),
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}
fn parent_lock_held(path: &Path) -> bool {
    use std::os::fd::AsRawFd;
    let Ok(file) = fs::OpenOptions::new().read(true).open(path) else {
        return false;
    };
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return false;
    }
    // F_GETLK asks the kernel without acquiring or releasing any source lock.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    unsafe {
        libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut lock) != -1
            && lock.l_type != libc::F_UNLCK as libc::c_short
    }
}
fn local_identity() -> (String, HashSet<String>) {
    let mut hostname = [0u8; 256];
    unsafe { libc::gethostname(hostname.as_mut_ptr().cast(), hostname.len()) };
    let hostname = String::from_utf8_lossy(
        &hostname[..hostname
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(hostname.len())],
    )
    .into_owned();
    let mut addresses = HashSet::from(["127.0.0.1".into(), "::1".into()]);
    unsafe {
        let mut head = std::ptr::null_mut();
        if libc::getifaddrs(&mut head) == 0 {
            let mut item = head;
            while !item.is_null() {
                let addr = (*item).ifa_addr;
                if !addr.is_null() {
                    match (*addr).sa_family as i32 {
                        libc::AF_INET => {
                            let s = &*(addr as *const libc::sockaddr_in);
                            addresses.insert(
                                std::net::Ipv4Addr::from(s.sin_addr.s_addr.to_ne_bytes())
                                    .to_string(),
                            );
                        }
                        libc::AF_INET6 => {
                            let s = &*(addr as *const libc::sockaddr_in6);
                            addresses
                                .insert(std::net::Ipv6Addr::from(s.sin6_addr.s6_addr).to_string());
                        }
                        _ => {}
                    }
                }
                item = (*item).ifa_next
            }
            libc::freeifaddrs(head)
        }
    }
    use std::net::ToSocketAddrs;
    if let Ok(resolved) = (hostname.as_str(), 0).to_socket_addrs() {
        addresses.extend(resolved.map(|a| a.ip().to_string()));
    }
    (hostname, addresses)
}
pub fn list_at(home: &Path) -> Result<Value> {
    if !home.is_absolute() {
        return Err("invalid_browser_home".into());
    }
    let (hostname, addresses) = local_identity();
    let mut out = vec![];
    for d in SOURCES {
        let Some(relative) = d.root else {
            out.push(
                json!({"id":d.id,"name":d.name,"profiles":[],"unavailable":"unsupportedPlatform"}),
            );
            continue;
        };
        let root = home.join(relative);
        let found = profiles(home, d);
        let installed = found
            .iter()
            .any(|p| database(&root, text(p, "directory"), d.firefox).is_some());
        let running = installed
            && if d.firefox {
                found.iter().any(|p| {
                    let dir = root.join(text(p, "directory"));
                    symlink_held(&dir.join("lock"), |t| firefox_lock_held(t, &addresses))
                        || parent_lock_held(&dir.join(".parentlock"))
                })
            } else {
                symlink_held(&root.join("SingletonLock"), |t| {
                    chromium_lock_held(t, &hostname)
                })
            };
        let mut row = json!({"id":d.id,"name":d.name,"profiles":if running{vec![]}else{found}});
        if !installed {
            row["profiles"] = json!([]);
            row["unavailable"] = json!("notInstalled")
        } else if running {
            row["unavailable"] = json!("browserRunning")
        }
        out.push(row)
    }
    Ok(json!(out))
}
// Read-only SQLite C API: listing exports counts only, never cookie values.
#[link(name = "sqlite3")]
unsafe extern "C" {
    fn sqlite3_open_v2(
        path: *const libc::c_char,
        db: *mut *mut libc::c_void,
        flags: libc::c_int,
        vfs: *const libc::c_char,
    ) -> libc::c_int;
    fn sqlite3_close(db: *mut libc::c_void) -> libc::c_int;
    fn sqlite3_prepare_v2(
        db: *mut libc::c_void,
        sql: *const libc::c_char,
        n: libc::c_int,
        stmt: *mut *mut libc::c_void,
        tail: *mut *const libc::c_char,
    ) -> libc::c_int;
    fn sqlite3_step(stmt: *mut libc::c_void) -> libc::c_int;
    fn sqlite3_column_int64(stmt: *mut libc::c_void, column: libc::c_int) -> i64;
    fn sqlite3_finalize(stmt: *mut libc::c_void) -> libc::c_int;
    fn sqlite3_progress_handler(
        db: *mut libc::c_void,
        n: libc::c_int,
        callback: Option<unsafe extern "C" fn(*mut libc::c_void) -> libc::c_int>,
        state: *mut libc::c_void,
    );
}
fn cookie_count(path: &Path, firefox: bool) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    struct Db(*mut libc::c_void);
    impl Drop for Db {
        fn drop(&mut self) {
            unsafe {
                sqlite3_close(self.0);
            }
        }
    }
    struct Statement(*mut libc::c_void);
    impl Drop for Statement {
        fn drop(&mut self) {
            unsafe {
                sqlite3_finalize(self.0);
            }
        }
    }
    unsafe extern "C" fn deadline(state: *mut libc::c_void) -> libc::c_int {
        let start = &*(state as *const std::time::Instant);
        i32::from(start.elapsed() > std::time::Duration::from_millis(100))
    }
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    unsafe {
        let mut db = std::ptr::null_mut();
        let status = sqlite3_open_v2(path.as_ptr(), &mut db, 1, std::ptr::null());
        if db.is_null() {
            return None;
        }
        let db = Db(db);
        if status != 0 {
            return None;
        }
        let start = std::time::Instant::now();
        sqlite3_progress_handler(
            db.0,
            1000,
            Some(deadline),
            (&start as *const std::time::Instant).cast_mut().cast(),
        );
        let sql = if firefox {
            c"SELECT count(*) FROM moz_cookies WHERE originAttributes = ''"
        } else {
            c"SELECT count(*) FROM cookies"
        };
        let mut stmt = std::ptr::null_mut();
        let status = sqlite3_prepare_v2(db.0, sql.as_ptr(), -1, &mut stmt, std::ptr::null_mut());
        if stmt.is_null() {
            return None;
        }
        let stmt = Statement(stmt);
        if status != 0 || sqlite3_step(stmt.0) != 100 {
            return None;
        }
        u64::try_from(sqlite3_column_int64(stmt.0, 0)).ok()
    }
}

pub(crate) struct SelectedSource {
    pub database: PathBuf,
    pub application: Option<&'static str>,
}
/// Never honor an arbitrary database path received from the UI. Re-list the
/// original source and require exact membership in its current profiles.
pub(crate) fn resolve_import_at(home: &Path, id: &str, directory: &str) -> Result<SelectedSource> {
    let definition = SOURCES.iter().find(|d| d.id == id).ok_or("unknownSource")?;
    let listed = list_at(home)?;
    let row = listed
        .as_array()
        .and_then(|rows| rows.iter().find(|s| s["id"] == id))
        .ok_or("unknownSource")?;
    if let Some(reason) = row["unavailable"].as_str() {
        return Err(reason.into());
    }
    if !row["profiles"]
        .as_array()
        .is_some_and(|profiles| profiles.iter().any(|p| p["directory"] == directory))
    {
        return Err("unknownSourceProfile".into());
    }
    let root = home.join(definition.root.ok_or("unsupportedPlatform")?);
    let path = database(&root, directory, definition.firefox).ok_or("readFailed")?;
    let application = match id {
        "chrome" => Some("chrome"),
        "edge" => Some("msedge"),
        "brave" => Some("brave"),
        "vivaldi" => Some("vivaldi"),
        "opera" => Some("opera"),
        "helium" => Some("chromium"),
        _ => None,
    };
    Ok(SelectedSource {
        database: path,
        application,
    })
}
