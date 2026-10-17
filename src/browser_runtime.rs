//! Desktop browser process bootstrap and native environment/profile partition
//! identity. Distinct from the sandboxed document renderer; no T3 credentials
//! enter the web page process. Native preview RPC ownership lives in Rust.
use crate::{json, text, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    os::{fd::FromRawFd, unix::fs::PermissionsExt},
    path::{Path, PathBuf},
};
pub struct ProfileLock {
    file: fs::File,
    path: PathBuf,
}
impl Drop for ProfileLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(std::os::fd::AsRawFd::as_raw_fd(&self.file), libc::LOCK_UN);
        }
    }
}
/// Outside the erasable partition, so cleanup and another worker cannot lock
/// two different inodes for the same profile while its directory is replaced.
pub fn lock_profile(storage: &Path, environment: &str, profile: &str) -> Result<ProfileLock> {
    use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
    let identity = partition(environment, profile)?;
    let locks = storage
        .parent()
        .ok_or("browser_storage_unavailable")?
        .join(".ot3-browser-profile-locks");
    private_directory(&locks)?;
    let name = text(&identity, "partition").trim_start_matches("persist:");
    let lock_path = locks.join(format!("{name}.lock"));
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&lock_path)
        .map_err(|_| "browser_storage_unavailable")?;
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err("browser_storage_unavailable".into());
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("browser_profile_busy".into());
    }
    Ok(ProfileLock {
        file,
        path: lock_path
            .canonicalize()
            .map_err(|_| "browser_storage_unavailable")?,
    })
}
/// The coordinator must hold the profile lease and OS lock, with its Qt
/// process stopped successfully. Only this derived Qt partition is removed.
pub fn clear_owned_storage(
    storage: &Path,
    environment: &str,
    profile: &str,
    lock: &ProfileLock,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let identity = partition(environment, profile)?;
    let name = text(&identity, "partition").trim_start_matches("persist:");
    let expected = storage
        .parent()
        .ok_or("browser_profile_storage_unsafe")?
        .join(".ot3-browser-profile-locks")
        .join(format!("{name}.lock"));
    let meta = fs::symlink_metadata(&expected).map_err(|_| "browser_profile_storage_unsafe")?;
    let held = lock
        .file
        .metadata()
        .map_err(|_| "browser_profile_storage_unsafe")?;
    if meta.file_type().is_symlink()
        || meta.dev() != held.dev()
        || meta.ino() != held.ino()
        || expected.canonicalize().ok().as_ref() != Some(&lock.path)
    {
        return Err("browser_profile_storage_unsafe".into());
    }
    if identity["persistent"] != true {
        return Ok(());
    }
    let path = storage.join(text(&identity, "partition").trim_start_matches("persist:"));
    for entry in [storage, &path] {
        match fs::symlink_metadata(entry) {
            Ok(m)
                if m.is_dir()
                    && !m.file_type().is_symlink()
                    && m.uid() == unsafe { libc::geteuid() } => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            _ => return Err("browser_profile_storage_unsafe".into()),
        }
    }
    fs::remove_dir_all(path).map_err(|_| "browser_profile_clear_failed".into())
}
pub fn partition(environment: &str, profile: &str) -> Result<Value> {
    if !crate::valid_id(environment) {
        return Err("invalid_browser_environment".into());
    }
    crate::browser_preferences::validate("browserDefaultProfileId", &json!(profile))?;
    // Exact native resolvePartitionScope + BrowserSession.getPartition for valid
    // Unicode strings: default keeps bare environment, others use JSON tuples and
    // a separate namespace. Incognito never persists.
    let scope = if profile == "default" {
        environment.to_owned()
    } else {
        json!([environment, profile]).to_string()
    };
    let digest = format!("{:x}", Sha256::digest(scope.as_bytes()));
    let persistent = profile != "incognito";
    let key = format!(
        "{}{}{}",
        if persistent {
            "persist:t3code-preview-"
        } else {
            "t3code-preview-ephemeral-"
        },
        if profile == "default" { "" } else { "profile-" },
        &digest[..20]
    );
    Ok(json!({"partition":key,"persistent":persistent}))
}
fn private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|_| "browser_storage_unavailable")?;
    let meta = path
        .symlink_metadata()
        .map_err(|_| "browser_storage_unavailable")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("browser_storage_unavailable".into());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| "browser_storage_unavailable".into())
}
pub fn prepare(mut payload: Value, storage: &Path, frames: &Path) -> Result<Value> {
    let identity = partition(text(&payload, "environmentId"), text(&payload, "profileId"))?;
    let width = payload.get("width").cloned().unwrap_or(json!(900));
    let height = payload.get("height").cloned().unwrap_or(json!(620));
    let size = crate::browser_preferences::validate(
        "browserDefaultViewport",
        &json!({"_tag":"freeform","width":width,"height":height}),
    )?;
    let zoom = crate::browser_preferences::validate(
        "browserDefaultZoomFactor",
        &payload.get("zoomFactor").cloned().unwrap_or(json!(1)),
    )?;
    payload = json!({"environmentId":payload["environmentId"],"profileId":payload["profileId"],"width":size["width"],"height":size["height"],"zoomFactor":zoom,"initializeCookieStore":payload["initializeCookieStore"]==true,"restoreSessionCookies":payload["restoreSessionCookies"]==true});
    private_directory(frames)?;
    payload["partition"] = identity["partition"].clone();
    payload["persistent"] = identity["persistent"].clone();
    payload["frameDirectory"] = json!(frames);
    if identity["persistent"] == true {
        private_directory(storage)?;
        let path = storage.join(text(&identity, "partition").trim_start_matches("persist:"));
        private_directory(&path)?;
        payload["profileDirectory"] = json!(path);
    } else {
        payload["profileDirectory"] = Value::Null;
    }
    Ok(payload)
}
pub fn renderer() -> Result<()> {
    // This entry point is an IPC guest, never a separate desktop window.
    std::env::set_var("QT_QPA_PLATFORM", "offscreen");
    std::env::set_var("QT_QUICK_BACKEND", "software");
    std::env::set_var("QT_SCALE_FACTOR", "1");
    std::env::set_var("QT_SCREEN_SCALE_FACTORS", "1");
    let fd = unsafe { libc::dup(libc::STDIN_FILENO) };
    if fd < 0 {
        return Err("invalid_browser_payload".into());
    }
    let mut input = unsafe { fs::File::from_raw_fd(fd) };
    let mut header = [0; 8];
    input
        .read_exact(&mut header)
        .map_err(|_| "invalid_browser_payload")?;
    let size = u64::from_be_bytes(header);
    if size > 64 * 1024 {
        return Err("invalid_browser_payload".into());
    }
    let mut bytes = vec![0; size as usize];
    input
        .read_exact(&mut bytes)
        .map_err(|_| "invalid_browser_payload")?;
    let payload: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid_browser_payload")?;
    let storage = storage_root();
    let _profile_lock = lock_profile(
        &storage,
        text(&payload, "environmentId"),
        text(&payload, "profileId"),
    )?;
    let cache = crate::xdg("XDG_CACHE_HOME", ".cache").join("browser-sessions");
    private_directory(&cache)?;
    let frames = cache.join(uuid::Uuid::new_v4().to_string());
    let payload = prepare(payload, &storage, &frames)?;
    let result = run(&payload);
    let _ = fs::remove_dir_all(frames);
    result
}
fn run(payload: &Value) -> Result<()> {
    let executable = std::env::current_exe().map_err(|_| "browser_runtime_unavailable")?;
    let root = executable.parent().ok_or("browser_runtime_unavailable")?;
    let library =
        std::ffi::CString::new(root.join("libt3-browser.so").to_string_lossy().as_bytes())
            .map_err(|_| "browser_runtime_unavailable")?;
    let qml = std::ffi::CString::new(
        root.join("../ui/browser/T3BrowserSurface.qml")
            .to_string_lossy()
            .as_bytes(),
    )
    .map_err(|_| "browser_runtime_unavailable")?;
    let payload =
        std::ffi::CString::new(payload.to_string()).map_err(|_| "invalid_browser_payload")?;
    unsafe {
        let handle = libc::dlopen(library.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if handle.is_null() {
            return Err("browser_runtime_unavailable".into());
        }
        let symbol = libc::dlsym(handle, c"t3_browser_main".as_ptr());
        if symbol.is_null() {
            return Err("browser_runtime_unavailable".into());
        }
        let entry: unsafe extern "C" fn(*const libc::c_char, *const libc::c_char) -> libc::c_int =
            std::mem::transmute(symbol);
        if entry(payload.as_ptr(), qml.as_ptr()) != 0 {
            return Err("browser_runtime_unavailable".into());
        }
    }
    Ok(())
}
pub fn storage_root() -> PathBuf {
    crate::t3_home().join("userdata/ot3-browser-profiles")
}
