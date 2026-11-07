//! Explicit native Electron guest launch. The parent renderer retains its profile
//! lock and supervision while the Electron main process owns the guest socket.
use crate::{json, text, Result};
use serde_json::Value;
use std::{
    fs,
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(crate) fn run(payload: &Value, runtime: &Path, assets: &Path) -> Result<()> {
    let executable = checked_file(&runtime.join("electron"))?;
    let assets = assets
        .canonicalize()
        .map_err(|_| "browser_runtime_unavailable")?;
    for name in [
        "native-electron-main.cjs",
        "electron-human-input.cjs",
        "native-electron-services.cjs",
        "electron-private-fd.node",
        "qt-target-scope.js",
    ] {
        checked_file(&assets.join(name))?;
    }
    let original = payload["privateBrowserFd"]
        .as_i64()
        .and_then(|fd| i32::try_from(fd).ok())
        .ok_or("invalid_browser_private_descriptor")?;
    // A distinct high descriptor avoids every stdio/launcher slot. No dup2
    // destination can collide with its source; only this fork clears CLOEXEC.
    let source = crate::desktop_browser_ipc::duplicate_child_source(original)
        .map_err(|_| "invalid_browser_private_descriptor")?;
    let mut child_payload = payload.clone();
    child_payload["privateBrowserFd"] = json!(source.as_raw_fd());
    child_payload["privateBrowserPeerPid"] = json!(unsafe { libc::getppid() });
    let mut command = Command::new(executable);
    command
        .arg("--disable-setuid-sandbox")
        .arg(assets.join("native-electron-main.cjs"))
        .env("O3_NATIVE_BROWSER_DATA", child_payload.to_string())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    unsafe {
        command.pre_exec(move || {
            crate::desktop_browser_ipc::admit_child_source(source.as_raw_fd())?;
            // Node admits and marks this destination CLOEXEC before guests.
            Ok(())
        });
    }
    let status = command
        .status()
        .map_err(|_| "browser_runtime_unavailable")?;
    if !status.success() {
        return Err("browser_runtime_unavailable".into());
    }
    Ok(())
}
fn checked_file(path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "browser_runtime_unavailable")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("browser_runtime_unavailable".into());
    }
    path.canonicalize()
        .map_err(|_| "browser_runtime_unavailable".into())
}
/// Both switches are needed; an installed executable alone never enables this
/// unfinished renderer or advertises server-browser capability.
pub(crate) fn configured() -> Option<(PathBuf, PathBuf)> {
    if std::env::var("O3_BROWSER_ENGINE").as_deref() != Ok("native-electron") {
        return None;
    }
    let runtime = std::env::var_os("O3_NATIVE_ELECTRON_RUNTIME")?;
    let assets = std::env::var_os("O3_NATIVE_ELECTRON_ASSETS")?;
    Some((runtime.into(), assets.into()))
}
pub(crate) fn directory(payload: &Value) -> Result<PathBuf> {
    // Electron profile data must never share Qt's Cookies/cache layout.
    let base = if payload["persistent"] == true {
        text(payload, "profileDirectory")
    } else {
        text(payload, "frameDirectory")
    };
    let path = Path::new(base).join("native-electron");
    fs::create_dir(&path)
        .or_else(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(error)
            }
        })
        .map_err(|_| "browser_storage_unavailable")?;
    let metadata = fs::symlink_metadata(&path).map_err(|_| "browser_storage_unavailable")?;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err("browser_storage_unavailable".into());
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
        .map_err(|_| "browser_storage_unavailable")?;
    Ok(path)
}
