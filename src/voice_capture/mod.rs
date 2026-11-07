//! One private native capture process. Audio never enters JSON or a status log.
pub mod resources;
use resources::{Assets, Runtime};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{DirBuilderExt, MetadataExt},
            net::UnixStream,
            process::CommandExt,
        },
    },
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
const CLOSE_ID: u64 = 9_007_199_254_740_991;
const MAX_PCM: usize = 19_200_000;
fn failed() -> io::Error {
    io::Error::other("Private native capture unavailable")
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Streaming,
    Buffered,
}
struct LaunchControl<'a> {
    synthetic: bool,
    startup: Option<&'a StartupCancellation>,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Streaming => "streaming",
            Self::Buffered => "buffered",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cleanup {
    Confirmed,
    Unknown,
}
#[derive(Debug)]
pub struct StartupFailure {
    pub cleanup: Cleanup,
}
impl std::fmt::Display for StartupFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Native capture startup did not complete")
    }
}
impl std::error::Error for StartupFailure {}
struct PendingChild(Option<Child>);
impl Drop for PendingChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            // No wait can release this identity before its single group fence.
            if let Ok(group) = i32::try_from(child.id()) {
                unsafe {
                    libc::kill(-group, libc::SIGKILL);
                }
            } else {
                let _ = child.kill();
            }
            let _ = std::thread::Builder::new()
                .name("capture-start-reap".into())
                .spawn(move || {
                    let _ = child.wait();
                });
        }
    }
}
/// PCM deliberately implements neither Serialize nor Debug. The consumer must
/// revalidate its current audio grant and authority before every SDK feed.
pub enum Event {
    Permission {
        request_id: String,
    },
    Level(f64),
    Pcm(Vec<u8>),
    Result {
        id: u64,
        ok: bool,
        error: Option<String>,
    },
    DurationLimit,
    CaptureError,
}
struct Gate {
    closed: AtomicBool,
    broken: AtomicBool,
    writer: Mutex<UnixStream>,
    capture: String,
}
#[derive(Clone)]
pub struct Cancellation(Arc<Gate>);
impl Cancellation {
    /// Fixed urgent close bypasses a pending permission. It cannot introduce a
    /// new command, capture identity, destination or ordinary request number.
    pub fn cancel(&self) {
        if self.0.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        let packet = json!({"type":"close","id":CLOSE_ID,"captureId":self.0.capture});
        let result = self
            .0
            .writer
            .try_lock()
            .map_err(|_| failed())
            .and_then(|mut writer| write(&mut writer, &packet, Duration::from_millis(100)));
        if result.is_err() {
            self.0.broken.store(true, Ordering::Release);
            if let Ok(writer) = self.0.writer.try_lock() {
                let _ = writer.shutdown(Shutdown::Both);
            }
        }
    }
}
/// One capture launch, including the period before its renderer is ready.
/// Cancellation is permanent and can be requested before a child exists.
#[derive(Clone, Default)]
pub struct StartupCancellation(Arc<StartupGate>);
#[derive(Default)]
struct StartupGate {
    cancelled: AtomicBool,
    started: AtomicBool,
    child_started: AtomicBool,
    capture: Mutex<Option<Cancellation>>,
}
impl StartupCancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        if let Ok(capture) = self.0.capture.lock() {
            if let Some(capture) = capture.as_ref() {
                capture.cancel();
            }
        }
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    pub(crate) fn child_started(&self) -> bool {
        self.0.child_started.load(Ordering::Acquire)
    }
    fn claim(&self) -> io::Result<()> {
        if self.0.started.swap(true, Ordering::AcqRel) || self.is_cancelled() {
            return Err(failed());
        }
        Ok(())
    }
    fn attach(&self, capture: Cancellation) -> io::Result<()> {
        let mut slot = self.0.capture.lock().map_err(|_| failed())?;
        if self.is_cancelled() {
            capture.cancel();
        }
        *slot = Some(capture);
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Preparing,
    Prepared,
    Recording,
    Stopping,
    Stopped,
    Failed,
}
#[derive(Clone, Copy)]
enum Operation {
    Prepare,
    Record,
    Stop,
    Permission,
}
pub struct Capture {
    runtime: Arc<Runtime>,
    assets: Arc<Assets>,
    _display: Arc<crate::browser_native_display::Display>,
    child: Option<Child>,
    pidfd: OwnedFd,
    group: i32,
    reader: UnixStream,
    gate: Cancellation,
    buffer: Vec<u8>,
    partial_since: Option<Instant>,
    pending: BTreeMap<u64, Operation>,
    sequence: u64,
    phase: Phase,
    mode: Mode,
    permission: Option<String>,
    pcm_bytes: usize,
    close_ack: bool,
    cleanup: Option<Cleanup>,
}
impl Capture {
    /// Only the native factory supplies admitted application artifacts and an
    /// owned authenticated display. No public request can choose an executable.
    pub fn launch(
        runtime: Arc<Runtime>,
        assets: Arc<Assets>,
        directory: &Path,
        display: Arc<crate::browser_native_display::Display>,
        mode: Mode,
        microphone_id: &str,
    ) -> io::Result<Self> {
        Self::launch_inner(
            runtime,
            assets,
            directory,
            display,
            mode,
            microphone_id,
            LaunchControl {
                synthetic: false,
                startup: None,
            },
        )
    }
    /// Explicit synthetic-device acceptance. This is never called by native
    /// configuration or selected through a preference/environment switch.
    pub fn synthetic(
        runtime: Arc<Runtime>,
        assets: Arc<Assets>,
        directory: &Path,
        display: Arc<crate::browser_native_display::Display>,
        mode: Mode,
        microphone_id: &str,
    ) -> io::Result<Self> {
        if !Path::new("/run/.containerenv").exists() {
            return Err(failed());
        }
        Self::launch_inner(
            runtime,
            assets,
            directory,
            display,
            mode,
            microphone_id,
            LaunchControl {
                synthetic: true,
                startup: None,
            },
        )
    }
    /// The trusted factory owns startup cancellation; no request supplies an
    /// executable, helper graph or display. Synthetic media is container-only.
    pub fn launch_cancellable(
        runtime: Arc<Runtime>,
        assets: Arc<Assets>,
        directory: &Path,
        display: Arc<crate::browser_native_display::Display>,
        mode: Mode,
        microphone_id: &str,
        startup: &StartupCancellation,
    ) -> io::Result<Self> {
        Self::launch_inner(
            runtime,
            assets,
            directory,
            display,
            mode,
            microphone_id,
            LaunchControl {
                synthetic: false,
                startup: Some(startup),
            },
        )
    }
    pub fn synthetic_cancellable(
        runtime: Arc<Runtime>,
        assets: Arc<Assets>,
        directory: &Path,
        display: Arc<crate::browser_native_display::Display>,
        mode: Mode,
        microphone_id: &str,
        startup: &StartupCancellation,
    ) -> io::Result<Self> {
        if !Path::new("/run/.containerenv").exists() {
            return Err(failed());
        }
        Self::launch_inner(
            runtime,
            assets,
            directory,
            display,
            mode,
            microphone_id,
            LaunchControl {
                synthetic: true,
                startup: Some(startup),
            },
        )
    }
    fn launch_inner(
        runtime: Arc<Runtime>,
        assets: Arc<Assets>,
        directory: &Path,
        display: Arc<crate::browser_native_display::Display>,
        mode: Mode,
        microphone_id: &str,
        control: LaunchControl<'_>,
    ) -> io::Result<Self> {
        let LaunchControl { synthetic, startup } = control;
        if let Some(startup) = startup {
            startup.claim()?;
        }
        runtime.verify()?;
        assets.verify()?;
        let parent = directory.parent().ok_or_else(failed)?;
        let metadata = fs::symlink_metadata(parent)?;
        if parent.canonicalize()? != parent
            || !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
            || microphone_id.len() > 60_000
        {
            return Err(failed());
        }
        fs::DirBuilder::new().mode(0o700).create(directory)?;
        let capture = random()?;
        let (reader, child_endpoint) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        let child_source =
            crate::desktop_browser_ipc::duplicate_child_source(child_endpoint.as_raw_fd())?;
        let descriptor = child_source.as_raw_fd();
        let parent_pid = unsafe { libc::getpid() };
        let payload =
            json!({"fd":descriptor,"peerPid":parent_pid,"captureId":capture,"directory":directory});
        let mut command = Command::new(runtime.executable());
        command.arg("--disable-setuid-sandbox");
        if synthetic {
            command.arg("--use-fake-device-for-media-stream");
        }
        command
            .arg(assets.entry())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("HOME", directory)
            .env("XDG_CONFIG_HOME", directory)
            .env("XDG_CACHE_HOME", directory)
            .env("XDG_DATA_HOME", directory)
            .env("DISPLAY", display.name())
            .env("XAUTHORITY", display.authorization_path())
            .env("O3_NATIVE_VOICE_DATA", payload.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for key in [
            "XDG_RUNTIME_DIR",
            "DBUS_SESSION_BUS_ADDRESS",
            "PULSE_SERVER",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        if synthetic {
            command.env("LIBGL_ALWAYS_SOFTWARE", "1").env(
                "__EGL_VENDOR_LIBRARY_FILENAMES",
                "/usr/share/glvnd/egl_vendor.d/50_mesa.json",
            );
        }
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0
                    || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent_pid
                {
                    return Err(failed());
                }
                crate::desktop_browser_ipc::admit_child_source(descriptor)?;
                Ok(())
            });
        }
        let writer = reader.try_clone()?;
        let mut pending_child = PendingChild(Some(command.spawn()?));
        if let Some(startup) = startup {
            startup.0.child_started.store(true, Ordering::Release);
        }
        drop(child_endpoint);
        let group = i32::try_from(pending_child.0.as_ref().unwrap().id()).map_err(|_| {
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
        let pidfd = unsafe { OwnedFd::from_raw_fd(raw as i32) };
        let gate = Cancellation(Arc::new(Gate {
            closed: AtomicBool::new(false),
            broken: AtomicBool::new(false),
            writer: Mutex::new(writer),
            capture,
        }));
        if let Some(startup) = startup {
            startup.attach(gate.clone())?;
        }
        let mut owner = Self {
            runtime,
            assets,
            _display: display,
            child: pending_child.0.take(),
            pidfd,
            group,
            reader,
            gate,
            buffer: Vec::new(),
            partial_since: None,
            pending: BTreeMap::new(),
            sequence: 0,
            phase: Phase::Preparing,
            mode,
            permission: None,
            pcm_bytes: 0,
            close_ack: false,
            cleanup: None,
        };
        let prepared = (|| {
            let ready = owner
                .packet(Instant::now() + Duration::from_secs(10))?
                .ok_or_else(failed)?;
            let value = owner.control(ready)?;
            if value["type"] != "ready" || value["runtime"] != "44.10.1" {
                return Err(failed());
            }
            owner.runtime.verify()?;
            owner.assets.verify()?;
            owner.send(
                Operation::Prepare,
                json!({"type":"prepare","mode":mode.name(),"microphoneId":microphone_id}),
            )?;
            Ok::<_, io::Error>(())
        })();
        if prepared.is_err() {
            let cleanup = owner.close();
            return Err(io::Error::other(StartupFailure { cleanup }));
        }
        Ok(owner)
    }
    pub fn cancellation(&self) -> Cancellation {
        self.gate.clone()
    }
    pub fn process_id(&self) -> u32 {
        self.group as u32
    }
    fn send(&mut self, operation: Operation, mut value: Value) -> io::Result<u64> {
        if self.gate.0.closed.load(Ordering::Acquire)
            || self.pending.len() >= 8
            || self.sequence >= CLOSE_ID - 1
        {
            return Err(failed());
        }
        let mut writer = self.gate.0.writer.lock().map_err(|_| failed())?;
        if self.gate.0.closed.load(Ordering::Acquire) {
            return Err(failed());
        }
        self.sequence += 1;
        value["id"] = json!(self.sequence);
        value["captureId"] = json!(self.gate.0.capture);
        if write(&mut writer, &value, Duration::from_millis(100)).is_err() {
            self.gate.0.broken.store(true, Ordering::Release);
            let _ = writer.shutdown(Shutdown::Both);
            return Err(failed());
        }
        self.pending.insert(self.sequence, operation);
        Ok(self.sequence)
    }
    pub fn permission(&mut self, request_id: &str, allow: bool) -> io::Result<u64> {
        if self.permission.as_deref() != Some(request_id) {
            return Err(failed());
        }
        let id = self.send(
            Operation::Permission,
            json!({"type":"permission","requestId":request_id,"allow":allow}),
        )?;
        self.permission = None;
        Ok(id)
    }
    pub fn record(&mut self) -> io::Result<u64> {
        if self.phase != Phase::Prepared {
            return Err(failed());
        }
        let id = self.send(Operation::Record, json!({"type":"record"}))?;
        self.phase = Phase::Recording;
        Ok(id)
    }
    pub fn stop(&mut self) -> io::Result<u64> {
        if self.phase != Phase::Recording {
            return Err(failed());
        }
        let id = self.send(Operation::Stop, json!({"type":"stop"}))?;
        self.phase = Phase::Stopping;
        Ok(id)
    }
    pub fn next(&mut self, budget: Duration) -> io::Result<Option<Event>> {
        let deadline = Instant::now() + budget;
        while let Some(packet) = self.packet(deadline)? {
            if packet.0 != 0 {
                if self.gate.0.closed.load(Ordering::Acquire) {
                    continue;
                }
                if !matches!(self.phase, Phase::Recording | Phase::Stopping)
                    || (packet.0 == 1) != (self.mode == Mode::Streaming)
                    || packet.1.len() % 4 != 0
                    || self.pcm_bytes > MAX_PCM - packet.1.len()
                {
                    return Err(failed());
                }
                if !packet
                    .1
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|bytes| f32::from_le_bytes(*bytes).is_finite())
                {
                    return Err(failed());
                }
                self.runtime.verify()?;
                self.assets.verify()?;
                self.pcm_bytes += packet.1.len();
                return Ok(Some(Event::Pcm(packet.1)));
            }
            let value = self.control(packet)?;
            match value["type"].as_str() {
                Some("permission-request") => {
                    let id = value["requestId"]
                        .as_str()
                        .filter(|id| {
                            id.len() == 48 && id.bytes().all(|value| value.is_ascii_hexdigit())
                        })
                        .ok_or_else(failed)?;
                    if value["media"] != "audio" || self.permission.is_some() {
                        return Err(failed());
                    }
                    self.permission = Some(id.into());
                    return Ok(Some(Event::Permission {
                        request_id: id.into(),
                    }));
                }
                Some("level") => {
                    let level = value["value"]
                        .as_f64()
                        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                        .ok_or_else(failed)?;
                    if !self.gate.0.closed.load(Ordering::Acquire) {
                        return Ok(Some(Event::Level(level)));
                    }
                }
                Some("duration-limit") => return Ok(Some(Event::DurationLimit)),
                Some("capture-error") => return Ok(Some(Event::CaptureError)),
                Some("result") => {
                    let id = value["id"].as_u64().ok_or_else(failed)?;
                    let ok = value["ok"].as_bool().ok_or_else(failed)?;
                    if id == CLOSE_ID {
                        if !self.gate.0.closed.load(Ordering::Acquire)
                            || !ok
                            || value["value"] != json!({"cleanup":"confirmed"})
                        {
                            return Err(failed());
                        }
                        self.close_ack = true;
                        return Ok(None);
                    }
                    let operation = self.pending.remove(&id).ok_or_else(failed)?;
                    // A close can overtake a buffered stop's already queued
                    // PCM. Discard that result along with its audio instead of
                    // comparing its byte count with intentionally dropped data.
                    if self.gate.0.closed.load(Ordering::Acquire) {
                        continue;
                    }
                    if ok {
                        match operation {
                            Operation::Prepare => self.phase = Phase::Prepared,
                            Operation::Stop => {
                                if value["value"]["mode"] != self.mode.name()
                                    || value["value"]["pcmBytes"].as_u64()
                                        != Some(self.pcm_bytes as u64)
                                {
                                    return Err(failed());
                                }
                                self.phase = Phase::Stopped;
                            }
                            _ => {}
                        }
                    } else {
                        self.phase = Phase::Failed;
                    }
                    let error = if ok {
                        None
                    } else {
                        Some(
                            value["value"]["error"]
                                .as_str()
                                .filter(|value| {
                                    matches!(
                                        *value,
                                        "capture_cancelled"
                                            | "capture_permission_denied"
                                            | "capture_failed"
                                            | "stale_permission"
                                    )
                                })
                                .ok_or_else(failed)?
                                .to_owned(),
                        )
                    };
                    return Ok(Some(Event::Result { id, ok, error }));
                }
                _ => return Err(failed()),
            }
        }
        Ok(None)
    }
    fn control(&self, packet: (u32, Vec<u8>)) -> io::Result<Value> {
        if packet.0 != 0 {
            return Err(failed());
        }
        let value: Value = serde_json::from_slice(&packet.1).map_err(|_| failed())?;
        if !value.is_object() || value["captureId"] != self.gate.0.capture {
            return Err(failed());
        }
        Ok(value)
    }
    fn packet(&mut self, deadline: Instant) -> io::Result<Option<(u32, Vec<u8>)>> {
        loop {
            if self.buffer.len() >= 12 {
                if &self.buffer[..4] != b"OVC1" {
                    return Err(failed());
                }
                let kind = u32::from_le_bytes(self.buffer[4..8].try_into().unwrap());
                let len = u32::from_le_bytes(self.buffer[8..12].try_into().unwrap()) as usize;
                if kind > 2 || len == 0 || len > if kind == 0 { 65536 } else { 32000 } {
                    return Err(failed());
                }
                if self.buffer.len() >= 12 + len {
                    let body = self.buffer[12..12 + len].to_vec();
                    self.buffer.drain(..12 + len);
                    self.partial_since = if self.buffer.is_empty() {
                        None
                    } else {
                        Some(Instant::now())
                    };
                    return Ok(Some((kind, body)));
                }
            }
            if self
                .partial_since
                .is_some_and(|start| start.elapsed() > Duration::from_secs(5))
            {
                return Err(failed());
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            if !poll(self.reader.as_raw_fd(), libc::POLLIN, deadline)? {
                return Ok(None);
            }
            let mut bytes = [0_u8; 32768];
            match self.reader.read(&mut bytes) {
                Ok(0) => return Err(failed()),
                Ok(count) => {
                    if self.buffer.len() + count > 65536 + 12 + 32768 {
                        return Err(failed());
                    }
                    if self.buffer.is_empty() {
                        self.partial_since = Some(Instant::now())
                    }
                    self.buffer.extend_from_slice(&bytes[..count]);
                }
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        || error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => return Err(failed()),
            }
        }
    }
    /// A successful receipt requires the exact renderer close acknowledgement,
    /// observed leader exit, terminal owned-group fence and subsequent reap.
    /// Forced/uncertain retirement remains Unknown and must block owner reuse.
    pub fn close(&mut self) -> Cleanup {
        if let Some(cleanup) = self.cleanup {
            return cleanup;
        }
        self.gate.cancel();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.close_ack && Instant::now() < deadline {
            if self
                .next(deadline.saturating_duration_since(Instant::now()))
                .is_err()
            {
                break;
            }
        }
        let exited = poll(self.pidfd.as_raw_fd(), libc::POLLIN, deadline).unwrap_or(false);
        let fenced = unsafe { libc::kill(-self.group, libc::SIGKILL) } == 0
            || io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        let _ = self.reader.shutdown(Shutdown::Both);
        let mut cleanup = Cleanup::Unknown;
        if let Some(mut child) = self.child.take() {
            if exited {
                if child.wait().is_ok_and(|status| status.success())
                    && self.close_ack
                    && fenced
                    && !self.gate.0.broken.load(Ordering::Acquire)
                {
                    cleanup = Cleanup::Confirmed
                }
            } else {
                // The numeric identity is never signalled again after wait can
                // reap it. A failed exit observation retains uncertainty.
                let _ = std::thread::Builder::new()
                    .name("capture-reap".into())
                    .spawn(move || {
                        let _ = child.wait();
                    });
            }
        }
        self.cleanup = Some(cleanup);
        cleanup
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
fn write(writer: &mut UnixStream, value: &Value, budget: Duration) -> io::Result<()> {
    let body = serde_json::to_vec(value).map_err(|_| failed())?;
    if body.len() > 65536 {
        return Err(failed());
    }
    let mut bytes = Vec::with_capacity(body.len() + 12);
    bytes.extend_from_slice(b"OVC1");
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    let deadline = Instant::now() + budget;
    let mut offset = 0;
    while offset < bytes.len() {
        if !poll(writer.as_raw_fd(), libc::POLLOUT, deadline)? {
            return Err(failed());
        }
        match writer.write(&bytes[offset..]) {
            Ok(0) => return Err(failed()),
            Ok(count) => offset += count,
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    || error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => return Err(failed()),
        }
    }
    Ok(())
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
fn random() -> io::Result<String> {
    let mut bytes = [0_u8; 24];
    let mut offset = 0;
    while offset < bytes.len() {
        let count = unsafe {
            libc::getrandom(bytes[offset..].as_mut_ptr().cast(), bytes.len() - offset, 0)
        };
        if count < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        }
        if count <= 0 {
            return Err(failed());
        }
        offset += count as usize;
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
