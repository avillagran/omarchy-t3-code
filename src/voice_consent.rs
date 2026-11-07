//! A local consent owner. Opaque guards always consult the original producer
//! over its peer-admitted private socket; snapshots never authorize audio.
use crate::voice_capture::{
    resources::{Assets, Runtime},
    Cleanup, StartupFailure,
};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    net::Shutdown,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
            net::UnixStream,
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
const MAX_PACKET: usize = 32 * 1024 * 1024;
fn failed() -> io::Error {
    io::Error::other("Private native consent unavailable")
}
struct Namespace {
    path: PathBuf,
    file: File,
    dev: u64,
    ino: u64,
}
impl Namespace {
    fn admit(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() || path.canonicalize()? != path {
            return Err(failed());
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        let metadata = file.metadata()?;
        let namespace = Self {
            path: path.into(),
            file,
            dev: metadata.dev(),
            ino: metadata.ino(),
        };
        namespace.verify()?;
        Ok(namespace)
    }
    fn verify(&self) -> io::Result<()> {
        if self.path.canonicalize()? != self.path {
            return Err(failed());
        }
        for metadata in [self.file.metadata()?, fs::symlink_metadata(&self.path)?] {
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.mode() & 0o7777 != 0o700
                || metadata.dev() != self.dev
                || metadata.ino() != self.ino
            {
                return Err(failed());
            }
        }
        Ok(())
    }
}
struct Pending(Option<Child>);
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            if let Ok(group) = i32::try_from(child.id()) {
                unsafe {
                    libc::kill(-group, libc::SIGKILL);
                }
            } else {
                let _ = child.kill();
            }
            let _ = std::thread::Builder::new()
                .name("consent-start-reap".into())
                .spawn(move || {
                    let _ = child.wait();
                });
        }
    }
}
/// Inner JSON remains a string so arbitrary native UTF16 grant/EnvironmentId
/// code units survive Rust's UTF8 JSON boundary. Qt parses this inner document.
pub struct Snapshot {
    pub choice_json: String,
    pub cleanup_json: String,
}
#[derive(Clone)]
pub struct ConsentOwner(Arc<Mutex<Worker>>);
pub struct AudioGrantGuard {
    owner: Weak<Mutex<Worker>>,
    binding: String,
    destination_json: String,
}
pub struct CloudAcknowledgement {
    owner: Weak<Mutex<Worker>>,
    acknowledgement: String,
}
struct Worker {
    runtime: Arc<Runtime>,
    assets: Arc<Assets>,
    namespace: Namespace,
    child: Option<Child>,
    pidfd: OwnedFd,
    group: i32,
    socket: UnixStream,
    buffer: Vec<u8>,
    sequence: u64,
    changes: u64,
    broken: bool,
    cleanup: Option<Cleanup>,
}
impl ConsentOwner {
    /// Native factory inputs only. No request supplies runtime, helper or store
    /// paths. The namespace must already be a canonical, owned private directory.
    pub fn launch(
        runtime: Arc<Runtime>,
        assets: Arc<Assets>,
        process_directory: &Path,
        namespace: &Path,
    ) -> io::Result<Self> {
        runtime.verify()?;
        assets.verify()?;
        if assets.entry().file_name().and_then(|name| name.to_str()) != Some("consent-helper.cjs") {
            return Err(failed());
        }
        let namespace = Namespace::admit(namespace)?;
        let parent = Namespace::admit(process_directory.parent().ok_or_else(failed)?)?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(process_directory)?;
        parent.verify()?;
        let (socket, child_socket) = UnixStream::pair()?;
        socket.set_nonblocking(true)?;
        let source = crate::desktop_browser_ipc::duplicate_child_source(child_socket.as_raw_fd())?;
        let fd = source.as_raw_fd();
        let parent_pid = unsafe { libc::getpid() };
        let mut command = Command::new(runtime.executable());
        command
            .arg(assets.entry())
            .arg(fd.to_string())
            .arg(parent_pid.to_string())
            .arg(&namespace.path)
            .arg(assets.descriptor())
            .env_clear()
            .env("ELECTRON_RUN_AS_NODE", "1")
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("HOME", process_directory)
            .env("TMPDIR", process_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0
                    || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent_pid
                {
                    return Err(failed());
                }
                crate::desktop_browser_ipc::admit_child_source(fd)
            });
        }
        let mut pending = Pending(Some(command.spawn()?));
        drop(child_socket);
        let group = i32::try_from(pending.0.as_ref().unwrap().id()).map_err(|_| {
            io::Error::other(StartupFailure {
                cleanup: Cleanup::Unknown,
            })
        })?;
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, group, 0) };
        if raw < 0 {
            return Err(io::Error::other(StartupFailure {
                cleanup: Cleanup::Unknown,
            }));
        }
        let mut worker = Worker {
            runtime,
            assets,
            namespace,
            child: pending.0.take(),
            pidfd: unsafe { OwnedFd::from_raw_fd(raw as i32) },
            group,
            socket,
            buffer: Vec::new(),
            sequence: 0,
            changes: 0,
            broken: false,
            cleanup: None,
        };
        let ready = worker.receive(Instant::now() + Duration::from_secs(5));
        if ready.is_err()
            || ready.as_ref().is_ok_and(|value| {
                value != &json!({"type":"ready","protocol":"o3-native-consent-v1"})
            })
            || worker.verify().is_err()
        {
            let cleanup = worker.close();
            return Err(io::Error::other(StartupFailure { cleanup }));
        }
        Ok(Self(Arc::new(Mutex::new(worker))))
    }
    fn call(&self, packet: Value) -> io::Result<Value> {
        self.0.lock().map_err(|_| failed())?.call(packet)
    }
    pub fn snapshot(&self) -> io::Result<Snapshot> {
        snapshot(self.call(json!({"op":"read"}))?)
    }
    /// Call only for an explicit current UI choice, never preference replay.
    pub fn choose_audio(&self, destination_json: &str) -> io::Result<Snapshot> {
        snapshot(self.call(json!({"op":"choose-audio","destinationJson":destination_json}))?)
    }
    pub fn choose_cleanup(&self, enabled: bool) -> io::Result<Snapshot> {
        snapshot(self.call(json!({"op":"choose-cleanup","enabled":enabled}))?)
    }
    pub fn pending_cloud(&self, destination_json: &str) -> io::Result<CloudAcknowledgement> {
        let result = self.call(json!({"op":"pending-cloud","destinationJson":destination_json}))?;
        Ok(CloudAcknowledgement {
            owner: Arc::downgrade(&self.0),
            acknowledgement: opaque(&result, "acknowledgement")?,
        })
    }
    pub fn bind_audio(&self, grant_id_json: &str) -> io::Result<AudioGrantGuard> {
        let result = self.call(json!({"op":"bind","grantIdJson":grant_id_json}))?;
        Ok(AudioGrantGuard {
            owner: Arc::downgrade(&self.0),
            binding: opaque(&result, "binding")?,
            destination_json: result["destinationJson"]
                .as_str()
                .ok_or_else(failed)?
                .into(),
        })
    }
    /// A monotonic notification hint. Authority always comes from a fresh check.
    pub fn change_generation(&self) -> io::Result<u64> {
        self.snapshot()?;
        Ok(self.0.lock().map_err(|_| failed())?.changes)
    }
    pub fn process_id(&self) -> io::Result<u32> {
        Ok(self.0.lock().map_err(|_| failed())?.group as u32)
    }
    pub fn close(&self) -> Cleanup {
        self.0
            .lock()
            .map(|mut worker| worker.close())
            .unwrap_or(Cleanup::Unknown)
    }
}
impl AudioGrantGuard {
    /// Destination supplied by the same consent realm as this binding, never
    /// copied independently from ordinary preferences or an old UI snapshot.
    /// A null destination is unavailable and cannot select a transport.
    pub fn destination_json(&self) -> &str {
        &self.destination_json
    }
    /// Invoke immediately before each SDK audio operation, together with current
    /// server authority. A dead helper, replaced namespace or read failure denies.
    pub fn is_current(&self) -> bool {
        self.owner
            .upgrade()
            .and_then(|owner| {
                owner.lock().ok().map(|mut worker| {
                    worker
                        .call(json!({"op":"check","binding":self.binding}))
                        .is_ok_and(|value| value["allowed"] == true)
                })
            })
            .unwrap_or(false)
    }
}
impl Drop for AudioGrantGuard {
    fn drop(&mut self) {
        // Releasing metadata is best effort; a dead owner never revives a guard.
        if let Some(owner) = self.owner.upgrade() {
            if let Ok(mut worker) = owner.lock() {
                let _ = worker.call(json!({"op":"release","binding":self.binding}));
            }
        }
    }
}
impl CloudAcknowledgement {
    /// Consumes the original closure; late ACKs can acknowledge only their own
    /// pending choice. It cannot be moved to another worker or supplied by JSON.
    pub fn acknowledge(self) -> io::Result<Snapshot> {
        let owner = self.owner.upgrade().ok_or_else(failed)?;
        let result = owner
            .lock()
            .map_err(|_| failed())?
            .call(json!({"op":"acknowledge","acknowledgement":self.acknowledgement}))?;
        snapshot(result)
    }
}
impl Drop for CloudAcknowledgement {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            if let Ok(mut worker) = owner.lock() {
                let _ =
                    worker.call(json!({"op":"forget-ack","acknowledgement":self.acknowledgement}));
            }
        }
    }
}
fn snapshot(value: Value) -> io::Result<Snapshot> {
    Ok(Snapshot {
        choice_json: value["choiceJson"].as_str().ok_or_else(failed)?.into(),
        cleanup_json: value["cleanupJson"].as_str().ok_or_else(failed)?.into(),
    })
}
fn opaque(value: &Value, key: &str) -> io::Result<String> {
    value[key]
        .as_str()
        .filter(|value| value.len() == 48 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_owned)
        .ok_or_else(failed)
}
impl Worker {
    fn verify(&self) -> io::Result<()> {
        self.runtime.verify()?;
        self.assets.verify()?;
        self.namespace.verify()
    }
    fn call(&mut self, packet: Value) -> io::Result<Value> {
        if self.broken || self.cleanup.is_some() {
            return Err(failed());
        }
        let result = self
            .verify()
            .and_then(|_| self.exchange(packet))
            .and_then(|value| {
                self.verify()?;
                Ok(value)
            });
        if result.is_err() {
            self.broken = true;
        }
        result
    }
    fn exchange(&mut self, packet: Value) -> io::Result<Value> {
        self.exchange_until(packet, Instant::now() + Duration::from_secs(5))
    }
    fn exchange_until(&mut self, mut packet: Value, deadline: Instant) -> io::Result<Value> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .filter(|id| *id < 9_007_199_254_740_991)
            .ok_or_else(failed)?;
        packet["id"] = json!(self.sequence);
        let mut bytes = serde_json::to_vec(&packet).map_err(|_| failed())?;
        bytes.push(b'\n');
        if bytes.len() > MAX_PACKET {
            return Err(failed());
        }
        let mut offset = 0;
        while offset < bytes.len() {
            if !poll(self.socket.as_raw_fd(), libc::POLLOUT, deadline)? {
                return Err(failed());
            }
            match self.socket.write(&bytes[offset..]) {
                Ok(0) => return Err(failed()),
                Ok(count) => offset += count,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                    ) => {}
                Err(_) => return Err(failed()),
            }
        }
        loop {
            let value = self.receive(deadline)?;
            if value == json!({"type":"changed"}) {
                self.changes = self.changes.saturating_add(1);
                continue;
            }
            if value["type"] != "result" || value["id"] != self.sequence || value["ok"] != true {
                return Err(failed());
            }
            return Ok(value);
        }
    }
    fn receive(&mut self, deadline: Instant) -> io::Result<Value> {
        loop {
            if Instant::now() >= deadline {
                return Err(failed());
            }
            if let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let value = serde_json::from_slice(&self.buffer[..end]).map_err(|_| failed())?;
                self.buffer.drain(..=end);
                return Ok(value);
            }
            if !poll(self.socket.as_raw_fd(), libc::POLLIN, deadline)? {
                return Err(failed());
            }
            let mut bytes = [0_u8; 65536];
            match self.socket.read(&mut bytes) {
                Ok(0) => return Err(failed()),
                Ok(count) => {
                    if self.buffer.len() + count > MAX_PACKET {
                        return Err(failed());
                    }
                    self.buffer.extend_from_slice(&bytes[..count]);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                    ) => {}
                Err(_) => return Err(failed()),
            }
        }
    }
    fn close(&mut self) -> Cleanup {
        if let Some(cleanup) = self.cleanup {
            return cleanup;
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let ack = self.exchange_until(json!({"op":"close"}), deadline).is_ok();
        let exited = poll(self.pidfd.as_raw_fd(), libc::POLLIN, deadline).unwrap_or(false);
        // This terminal group fence always precedes the only operation that can
        // reap the child. Forced or uncertain retirement remains Unknown.
        let fenced = unsafe { libc::kill(-self.group, libc::SIGKILL) } == 0
            || io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        let _ = self.socket.shutdown(Shutdown::Both);
        let mut cleanup = Cleanup::Unknown;
        if let Some(mut child) = self.child.take() {
            if exited {
                if child.wait().is_ok_and(|status| status.success())
                    && ack
                    && fenced
                    && !self.broken
                {
                    cleanup = Cleanup::Confirmed;
                }
            } else {
                let _ = std::thread::Builder::new()
                    .name("consent-reap".into())
                    .spawn(move || {
                        let _ = child.wait();
                    });
            }
        }
        self.cleanup = Some(cleanup);
        cleanup
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
fn poll(fd: i32, events: i16, deadline: Instant) -> io::Result<bool> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        let mut descriptor = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let result = unsafe {
            libc::poll(
                &mut descriptor,
                1,
                remaining.as_millis().max(1).min(i32::MAX as u128) as i32,
            )
        };
        if result > 0 {
            return Ok(true);
        }
        if result == 0 {
            return Ok(false);
        }
        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Err(failed());
        }
    }
}
