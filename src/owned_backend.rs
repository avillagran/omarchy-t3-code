//! Optional direct-child supervisor. This leaf is not selected by service mode.
//! Credentials travel only through inherited bootstrap IPC and process memory.
use crate::*;
use std::io::{Read, Write};
use std::os::{
    fd::AsRawFd,
    unix::{
        ffi::OsStrExt,
        fs::{OpenOptionsExt, PermissionsExt},
        net::UnixStream,
        process::CommandExt,
    },
};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;
mod auth;
use std::sync::Mutex;

pub struct Config {
    pub executable: PathBuf,
    /// Dedicated application state root; never the current shared server profile.
    pub state_root: PathBuf,
}
impl Config {
    pub fn native(executable: PathBuf) -> Self {
        Self {
            executable,
            state_root: xdg("XDG_STATE_HOME", ".local/state"),
        }
    }
    pub fn profile(&self) -> PathBuf {
        self.state_root.join("owned-backends/o3")
    }
}
/// A successful launch has passed child identity, OAuth and authenticated RPC
/// checks. No bearer or bootstrap seed is written to disk or returned in status.
pub struct Owner {
    child: Mutex<Child>,
    origin: String,
    authorization: auth::Auth,
    profile: PathBuf,
    _lock: Mutex<Option<fs::File>>,
}
impl Owner {
    pub fn launch(config: &Config) -> Result<Self> {
        if !config.executable.is_absolute()
            || !config.executable.is_file()
            || !config.state_root.is_absolute()
        {
            return Err("owned_backend_configuration_invalid".into());
        }
        let profile = config.profile();
        // Linux pathname AF_UNIX addresses reserve one byte for the terminator.
        // Keep bridge identity persistent; never hide this limit by moving it.
        if profile
            .join("omabridge/control.sock")
            .as_os_str()
            .as_bytes()
            .len()
            >= 108
        {
            return Err("owned_backend_control_socket_path_too_long".into());
        }
        fs::create_dir_all(&config.state_root).map_err(|_| "owned_backend_profile_unavailable")?;
        for directory in [
            config.state_root.clone(),
            config.state_root.join("owned-backends"),
            profile.clone(),
        ] {
            match fs::symlink_metadata(&directory) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err("owned_backend_profile_unavailable".into())
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&directory).map_err(|_| "owned_backend_profile_unavailable")?;
                }
                Err(_) => return Err("owned_backend_profile_unavailable".into()),
            }
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .map_err(|_| "owned_backend_profile_unavailable")?;
        }
        if let Ok(metadata) = fs::symlink_metadata(profile.join("userdata")) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("owned_backend_profile_unavailable".into());
            }
        }
        // The process holding this open description owns this private profile.
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(profile.join("desktop-owner.lock"))
            .map_err(|_| "owned_backend_profile_unavailable")?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("owned_backend_already_running".into());
        }
        let (mut writer, reader) =
            UnixStream::pair().map_err(|_| "owned_backend_ipc_unavailable")?;
        let bootstrap_source =
            crate::desktop_browser_ipc::duplicate_child_source(reader.as_raw_fd())
                .map_err(|_| "owned_backend_ipc_unavailable")?;
        let fd = bootstrap_source.as_raw_fd();
        let secret = Zeroizing::new(format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ));
        let seed = auth::token(&secret, Utc::now().timestamp_millis())?;
        // Borrow credential strings while serializing directly into private IPC;
        // never create a secret-bearing serde_json::Value or disk payload.
        let envelope = Envelope {
            mode: "desktop",
            no_browser: true,
            host: "127.0.0.1",
            port: 3773,
            desktop_bootstrap_token: &seed,
            desktop_bootstrap_secret: &secret,
            tailscale_serve_enabled: false,
            tailscale_serve_port: 3773,
            t3_home: &profile,
        };
        let mut command = Command::new(&config.executable);
        command
            .args(["serve", "--mode", "desktop", "--bootstrap-fd"])
            .arg(fd.to_string())
            .arg("--base-dir")
            .arg(&profile)
            .args(["--host", "127.0.0.1", "--port", "0"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &config.state_root)
            .env("XDG_CONFIG_HOME", config.state_root.join("config"))
            .env("XDG_STATE_HOME", config.state_root.join("state"))
            .env("XDG_DATA_HOME", config.state_root.join("data"))
            .env("XDG_CACHE_HOME", config.state_root.join("cache"))
            .current_dir(&profile)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let parent = unsafe { libc::getpid() };
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent
                {
                    return Err(std::io::Error::other("owned_backend_parent_gone"));
                }
                crate::desktop_browser_ipc::admit_child_source(bootstrap_source.as_raw_fd())?;
                Ok(())
            });
        }
        let child = command.spawn().map_err(|_| "owned_backend_spawn_failed")?;
        drop(reader);
        let mut pending = Pending {
            child: Some(child),
            lock: Some(lock),
        };
        writer
            .set_write_timeout(Some(Duration::from_secs(1)))
            .map_err(|_| "owned_backend_ipc_unavailable")?;
        serde_json::to_writer(&mut writer, &envelope)
            .map_err(|_| "owned_backend_ipc_unavailable")?;
        writer
            .flush()
            .map_err(|_| "owned_backend_ipc_unavailable")?;
        writer
            .shutdown(std::net::Shutdown::Write)
            .map_err(|_| "owned_backend_ipc_unavailable")?;
        drop(writer);
        let deadline = Instant::now() + Duration::from_secs(15);
        let http = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|_| "owned_backend_readiness_failed")?;
        loop {
            let child = pending.child.as_mut().ok_or("owned_backend_start_failed")?;
            if child
                .try_wait()
                .map_err(|_| "owned_backend_start_failed")?
                .is_some()
            {
                return Err("owned_backend_exited_before_ready".into());
            }
            if let Ok(origin) = runtime_origin(&profile, child.id()) {
                if let Ok(grant) = auth::exchange(&http, &origin, &seed) {
                    let child = pending.child.take().ok_or("owned_backend_start_failed")?;
                    let lock = pending.lock.take().ok_or("owned_backend_start_failed")?;
                    return Ok(Self {
                        child: Mutex::new(child),
                        authorization: auth::Auth::new(secret, grant, http, origin.clone()),
                        origin,
                        profile,
                        _lock: Mutex::new(Some(lock)),
                    });
                }
            }
            if Instant::now() >= deadline {
                return Err("owned_backend_readiness_failed".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn profile(&self) -> &Path {
        &self.profile
    }
    pub fn connection(&self) -> protocol::Connection {
        self.authorization.snapshot()
    }
    pub fn fresh_connection(&self) -> Result<protocol::Connection> {
        if !self.alive()? {
            return Err("owned_backend_stopped".into());
        }
        let result = self.authorization.fresh();
        if !self.alive()? {
            return Err("owned_backend_stopped".into());
        }
        result
    }
    pub fn refresh_authorization(&self) -> Result<protocol::Connection> {
        if !self.alive()? {
            return Err("owned_backend_stopped".into());
        }
        let result = self.authorization.refresh();
        if !self.alive()? {
            return Err("owned_backend_stopped".into());
        }
        result
    }
    pub fn stop(&self) -> Result<()> {
        self.authorization.begin_close();
        let result = {
            let mut child = self
                .child
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            stop_child(&mut child);
            child
                .try_wait()
                .map_err(|_| "owned_backend_state_unavailable".to_owned())
                .and_then(|status| {
                    if status.is_some() {
                        Ok(())
                    } else {
                        Err("owned_backend_stop_failed".into())
                    }
                })
        };
        self.authorization.finish_close();
        if result.is_ok() {
            self._lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
        }
        result
    }
    pub fn is_running(&mut self) -> Result<bool> {
        self.alive()
    }
    fn alive(&self) -> Result<bool> {
        let alive = self
            .child
            .lock()
            .map_err(|_| "owned_backend_state_unavailable")?
            .try_wait()
            .map_err(|_| "owned_backend_state_unavailable")?
            .is_none();
        if !alive {
            self.authorization.begin_close();
            self.authorization.finish_close();
        }
        Ok(alive)
    }
}
// Hold the profile lock for the owner's entire lifetime, including startup.
struct Pending {
    child: Option<Child>,
    lock: Option<fs::File>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            stop_child(child);
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn stop_child(child: &mut Child) {
    match child.try_wait() {
        Ok(None) => {}
        Ok(Some(_)) | Err(_) => return,
    }
    let group = -(child.id() as i32);
    unsafe {
        libc::kill(group, libc::SIGTERM);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(None) => {}
            Ok(Some(_)) | Err(_) => return,
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    unsafe {
        libc::kill(group, libc::SIGKILL);
    }
    let _ = child.wait();
}
fn runtime_origin(profile: &Path, pid: u32) -> Result<String> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(profile.join("userdata/server-runtime.json"))
        .map_err(|_| "owned_backend_not_ready")?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "owned_backend_not_ready")?;
    if bytes.len() > 16384 {
        return Err("owned_backend_runtime_invalid".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "owned_backend_runtime_invalid")?;
    if value["version"] != 1 || value["pid"].as_u64() != Some(pid.into()) {
        return Err("owned_backend_runtime_invalid".into());
    }
    let origin = text(&value, "origin");
    validate_base(origin)?;
    let url = Url::parse(origin).map_err(|_| "owned_backend_runtime_invalid")?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none_or(|p| p == 0)
    {
        return Err("owned_backend_runtime_invalid".into());
    }
    Ok(origin.into())
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Envelope<'a> {
    mode: &'static str,
    no_browser: bool,
    host: &'static str,
    port: u16,
    desktop_bootstrap_token: &'a str,
    desktop_bootstrap_secret: &'a str,
    tailscale_serve_enabled: bool,
    tailscale_serve_port: u16,
    t3_home: &'a Path,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("owned-backend-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn runtime_requires_child_identity_version_and_exact_loopback_origin() {
        let root = fixture();
        fs::create_dir(root.join("userdata")).unwrap();
        let path = root.join("userdata/server-runtime.json");
        for value in [
            json!({"version":1,"pid":12,"origin":"http://127.0.0.1:1234"}),
            json!({"version":2,"pid":11,"origin":"http://127.0.0.1:1234"}),
            json!({"version":1,"pid":11,"origin":"http://example.invalid:1234"}),
            json!({"version":1,"pid":11,"origin":"https://127.0.0.1:1234"}),
            json!({"version":1,"pid":11,"origin":"http://127.0.0.1:1234/private"}),
        ] {
            fs::write(&path, value.to_string()).unwrap();
            assert!(runtime_origin(&root, 11).is_err());
        }
        fs::write(
            &path,
            json!({"version":1,"pid":11,"origin":"http://127.0.0.1:1234"}).to_string(),
        )
        .unwrap();
        assert_eq!(runtime_origin(&root, 11).unwrap(), "http://127.0.0.1:1234");
        fs::write(&path, vec![b' '; 16385]).unwrap();
        assert!(runtime_origin(&root, 11).is_err());
    }
    #[test]
    fn runtime_file_link_is_refused_without_following_owned_fixture_target() {
        let root = fixture();
        fs::create_dir(root.join("userdata")).unwrap();
        let target = root.join("retained-runtime");
        let bytes = json!({"version":1,"pid":11,"origin":"http://127.0.0.1:1234"}).to_string();
        fs::write(&target, &bytes).unwrap();
        std::os::unix::fs::symlink(&target, root.join("userdata/server-runtime.json")).unwrap();
        assert!(runtime_origin(&root, 11).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), bytes);
    }
    #[test]
    fn linked_private_userdata_is_refused_before_child_launch() {
        let root = fixture();
        let config = Config {
            executable: PathBuf::from("/usr/bin/true"),
            state_root: root.join("state"),
        };
        fs::create_dir_all(config.profile()).unwrap();
        let target = root.join("retained-userdata");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("retained"), b"fixture").unwrap();
        std::os::unix::fs::symlink(&target, config.profile().join("userdata")).unwrap();
        assert!(
            matches!(Owner::launch(&config),Err(error) if error=="owned_backend_profile_unavailable")
        );
        assert_eq!(fs::read(target.join("retained")).unwrap(), b"fixture");
    }
    #[test]
    fn long_control_socket_path_is_refused_before_profile_creation() {
        let root = fixture();
        let config = Config {
            executable: PathBuf::from("/usr/bin/true"),
            state_root: root.join("x".repeat(108)),
        };
        assert!(
            matches!(Owner::launch(&config),Err(error) if error=="owned_backend_control_socket_path_too_long")
        );
        assert!(!config.state_root.exists());
    }
    #[test]
    fn actual_previous_rotating_bucket_authenticates_without_exporting_secret() {
        let root = std::env::temp_dir().join(format!(
            "op-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        ));
        fs::create_dir(&root).unwrap();
        let config = Config {
            executable: PathBuf::from("/fixture/o3-server"),
            state_root: root.join("state"),
        };
        let owner = Owner::launch(&config).unwrap();
        let credential = owner
            .authorization
            .credential(Utc::now().timestamp_millis() - auth::WINDOW_MS)
            .unwrap();
        let http = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let grant = auth::exchange(&http, owner.origin(), &credential).unwrap();
        assert!(!grant.connection.same_session_admission(&owner.connection()));
    }
}
