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

#[path = "browser_stack_diagnostics.rs"]
mod stack_diagnostics;

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
    let identity = payload
        .get("privateClipboardIdentityFd")
        .map(|value| {
            let descriptor = value
                .as_i64()
                .and_then(|value| i32::try_from(value).ok())
                .ok_or("invalid_browser_private_descriptor")?;
            crate::desktop_browser_ipc::duplicate_child_source(descriptor)
                .map_err(|_| "invalid_browser_private_descriptor")
        })
        .transpose()?;
    let mut child_payload = payload.clone();
    child_payload["privateBrowserFd"] = json!(source.as_raw_fd());
    if let Some(identity) = &identity {
        child_payload["privateClipboardIdentityFd"] = json!(identity.as_raw_fd());
    }
    child_payload["privateBrowserPeerPid"] = json!(unsafe { libc::getppid() });
    let mut command = Command::new(executable);
    command
        .arg("--disable-setuid-sandbox")
        .arg(assets.join("native-electron-main.cjs"))
        .env("O3_NATIVE_BROWSER_DATA", child_payload.to_string())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    // Only Config::isolated supplies this path. Ordinary native factories do
    // not forward the diagnostic variable or enable native stack printing.
    let diagnostics = isolated_diagnostics()?;
    if diagnostics.is_some() {
        command
            .env("ELECTRON_ENABLE_STACK_DUMPING", "1")
            .stderr(Stdio::piped());
    }
    unsafe {
        command.pre_exec(move || {
            crate::desktop_browser_ipc::admit_child_source(source.as_raw_fd())?;
            if let Some(identity) = &identity {
                crate::desktop_browser_ipc::admit_child_source(identity.as_raw_fd())?;
            }
            // Node admits and marks this destination CLOEXEC before guests.
            Ok(())
        });
    }
    let mut child = command.spawn().map_err(|_| "browser_runtime_unavailable")?;
    if payload["privateEngineSpawn"] == true {
        let result = payload["privateClipboardIdentityFd"]
            .as_i64()
            .and_then(|fd| i32::try_from(fd).ok())
            .ok_or_else(|| "browser_engine_identity_unavailable".to_string())
            .and_then(|fd| crate::browser_engine_identity::report_spawn(fd, child.id()));
        if let Err(error) = result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    }
    if let Some(target) = diagnostics {
        let source = child.stderr.take().ok_or("browser_runtime_unavailable")?;
        if std::thread::Builder::new()
            .name("native-stack-addresses".into())
            .spawn(move || stack_diagnostics::drain(source, target))
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("browser_runtime_unavailable".into());
        }
    }
    let status = child.wait().map_err(|_| "browser_runtime_unavailable")?;
    if !status.success() {
        return Err("browser_runtime_unavailable".into());
    }
    Ok(())
}

fn isolated_diagnostics() -> Result<Option<fs::File>> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    let Some(path) = std::env::var_os("O3_NATIVE_ELECTRON_DIAGNOSTICS") else {
        return Ok(None);
    };
    let path = PathBuf::from(path);
    let home = std::env::var_os("HOME").ok_or("browser_diagnostics_unavailable")?;
    let home = Path::new(&home);
    if !home.is_absolute()
        || path != home.join("native-electron.log")
        || home
            .canonicalize()
            .map_err(|_| "browser_diagnostics_unavailable")?
            != home
    {
        return Err("browser_diagnostics_unavailable".into());
    }
    let parent = fs::symlink_metadata(home).map_err(|_| "browser_diagnostics_unavailable")?;
    if !parent.is_dir()
        || parent.uid() != unsafe { libc::geteuid() }
        || parent.permissions().mode() & 0o077 != 0
    {
        return Err("browser_diagnostics_unavailable".into());
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "browser_diagnostics_unavailable")?;
    let metadata = file
        .metadata()
        .map_err(|_| "browser_diagnostics_unavailable")?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("browser_diagnostics_unavailable".into());
    }
    Ok(Some(file))
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
