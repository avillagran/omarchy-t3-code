//! One owned decoder attempt, bounded diagnostics, and exact child reaping.
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    ffi::CString,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
            process::ExitStatusExt,
        },
    },
    path::{Component, Path},
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
};
const STDERR_LIMIT: usize = 8192;
const PAYLOAD_LIMIT: usize = 1024 * 1024;
const EVENT_LIMIT: usize = 64;
const TRACE_LIMIT: usize = 16;
static TRACES: AtomicUsize = AtomicUsize::new(0);
pub type Slot = Arc<Mutex<Option<Process>>>;
#[derive(Default)]
struct Tail {
    bytes: VecDeque<u8>,
    omitted: usize,
}
impl Tail {
    fn push(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if self.bytes.len() == STDERR_LIMIT {
                self.bytes.pop_front();
                self.omitted += 1;
            }
            self.bytes.push_back(*byte);
        }
    }
}
struct Trace {
    payload: File,
    packets: File,
    report: File,
    accepted: usize,
    wire: usize,
    events: Vec<Value>,
    omitted_events: usize,
    omitted_payload: usize,
    io_failed: bool,
    terminal: Option<Value>,
}
impl Trace {
    fn open(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(io::Error::other("Diagnostic root must be absolute"));
        }
        let mut directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open("/")?;
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    let name = CString::new(name.as_bytes()).map_err(io::Error::other)?;
                    // The parent descriptor is owned and every component refuses links.
                    let fd = unsafe {
                        libc::openat(
                            directory.as_raw_fd(),
                            name.as_ptr(),
                            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                        )
                    };
                    if fd < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    // openat returned a new, exclusively owned descriptor.
                    directory = unsafe { File::from_raw_fd(fd) };
                }
                _ => {
                    return Err(io::Error::other(
                        "Diagnostic root must not traverse parents",
                    ))
                }
            }
        }
        let metadata = directory.metadata()?;
        // getuid has no memory or resource ownership requirements.
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::other(
                "Diagnostic root must be private and owned",
            ));
        }
        let name = uuid::Uuid::new_v4().to_string();
        let create = |extension: &str| -> io::Result<File> {
            let name = CString::new(format!("decoder-{name}.{extension}")).unwrap();
            // New regular slots only; no replacement, traversal, or cleanup.
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // The successful exclusive creation owns this descriptor.
            Ok(unsafe { File::from_raw_fd(fd) })
        };
        Ok(Self {
            payload: create("h264")?,
            packets: create("packets")?,
            report: create("json")?,
            accepted: 0,
            wire: 0,
            events: vec![],
            omitted_events: 0,
            omitted_payload: 0,
            io_failed: false,
            terminal: None,
        })
    }
    fn event(&mut self, event: Value) {
        if self.events.len() < EVENT_LIMIT {
            self.events.push(event);
        } else {
            self.omitted_events += 1;
        }
    }
    fn packet(&mut self, wire: &[u8], parsed: Option<(bool, &[u8])>) {
        let header = wire.get(..wire.len().min(24)).unwrap();
        self.event(json!({"kind":"binary","wireBytes":wire.len(),"semu":wire.starts_with(b"SEMU"),"version":wire.get(4),"headerHex":header.iter().map(|byte|format!("{byte:02x}")).collect::<String>(),"parsed":parsed.is_some(),"keyFrame":parsed.map(|(key,_)|key),"payloadBytes":parsed.map(|(_,bytes)|bytes.len())}));
        let left = PAYLOAD_LIMIT.saturating_sub(self.wire);
        if left >= 4 {
            let bytes = &wire[..wire.len().min(left - 4)];
            if !self.io_failed
                && self
                    .packets
                    .write_all(&(bytes.len() as u32).to_be_bytes())
                    .and_then(|()| self.packets.write_all(bytes))
                    .is_err()
            {
                self.io_failed = true;
            }
            self.wire += bytes.len() + 4;
        }
        if let Some((_, bytes)) = parsed {
            let length = bytes.len().min(PAYLOAD_LIMIT.saturating_sub(self.accepted));
            if !self.io_failed && self.payload.write_all(&bytes[..length]).is_err() {
                self.io_failed = true;
            }
            self.accepted += length;
            self.omitted_payload += bytes.len() - length;
        }
    }
    fn text(&mut self, text: &str) {
        let Ok(value) = serde_json::from_str::<Value>(text) else {
            self.event(json!({"kind":"text","bytes":text.len(),"json":false}));
            return;
        };
        let mut safe = serde_json::Map::new();
        for key in [
            "type",
            "codec",
            "width",
            "height",
            "orientation",
            "inputUnavailable",
            "ok",
            "code",
        ] {
            if let Some(value) = value.get(key) {
                let allowed = match value {
                    Value::Bool(_) | Value::Number(_) => true,
                    Value::String(text) => match key {
                        "type" => matches!(
                            text.as_str(),
                            "config"
                                | "state"
                                | "error"
                                | "ack"
                                | "input"
                                | "ready"
                                | "video-reset"
                        ),
                        "codec" => {
                            matches!(text.as_str(), "h264" | "h265" | "avc" | "hevc")
                                || text.strip_prefix("avc1.").is_some_and(|suffix| {
                                    suffix.len() == 6
                                        && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
                                })
                        }
                        "orientation" => matches!(
                            text.as_str(),
                            "portrait"
                                | "landscape"
                                | "portrait-upside-down"
                                | "landscape-left"
                                | "landscape-right"
                        ),
                        "code" => matches!(
                            text.as_str(),
                            "session_changed"
                                | "failed"
                                | "rejected"
                                | "invalid_input"
                                | "device_unavailable"
                                | "stream_unavailable"
                        ),
                        _ => false,
                    },
                    _ => false,
                };
                if allowed {
                    safe.insert(key.into(), value.clone());
                }
            }
        }
        self.event(json!({"kind":"text","bytes":text.len(),"json":true,"fields":safe}));
    }
    fn finish(&mut self, pid: u32, status: Option<ExitStatus>, owner_stopped: bool, stderr: &Tail) {
        self.io_failed |= self.payload.flush().is_err() || self.packets.flush().is_err();
        let payload_bytes = self
            .payload
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or_else(|_| {
                self.io_failed = true;
                0
            });
        let wire_bytes = self
            .packets
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or_else(|_| {
                self.io_failed = true;
                0
            });
        let value = json!({"format":1,"pid":pid,"ownerStopped":owner_stopped,"exitSignal":status.and_then(|status|status.signal()),"traceWriteFailed":self.io_failed,"transportTerminal":self.terminal,"exitCode":status.and_then(|status|status.code()),"success":status.is_some_and(|status|status.success()),"stderr":String::from_utf8_lossy(&stderr.bytes.iter().copied().collect::<Vec<_>>()),"omittedStderrBytes":stderr.omitted,"events":self.events,"omittedEvents":self.omitted_events,"payloadBytes":payload_bytes,"wireBytes":wire_bytes,"omittedPayloadBytes":self.omitted_payload,"limits":{"stderrBytes":STDERR_LIMIT,"payloadBytes":PAYLOAD_LIMIT,"wireBytes":PAYLOAD_LIMIT,"events":EVENT_LIMIT}});
        let _ = serde_json::to_writer(&mut self.report, &value);
        let _ = self.report.write_all(b"\n");
        let _ = self.report.flush();
        let _ = self.payload.flush();
        let _ = self.packets.flush();
    }
}
struct State {
    active: Mutex<bool>,
    stderr: Mutex<Tail>,
    trace: Mutex<Option<Trace>>,
}
pub struct Process {
    child: Child,
    state: Arc<State>,
    stderr_worker: Option<JoinHandle<()>>,
    finished: bool,
}
impl Process {
    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        *self.state.active.lock().unwrap() = false;
        let owner_stopped = !matches!(self.child.try_wait(), Ok(Some(_)));
        if owner_stopped {
            let _ = self.child.kill();
        }
        let status = self.child.wait().ok();
        if let Some(worker) = self.stderr_worker.take() {
            let _ = worker.join();
        }
        let stderr = self.state.stderr.lock().unwrap();
        if let Some(trace) = self.state.trace.lock().unwrap().as_mut() {
            trace.finish(self.child.id(), status, owner_stopped, &stderr);
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.finish();
    }
}
#[derive(Clone)]
pub struct Attempt {
    slot: Slot,
    state: Arc<State>,
}
impl Attempt {
    pub fn publish(&self, publish: impl FnOnce()) {
        let active = self.state.active.lock().unwrap();
        if *active {
            publish();
        }
    }
    pub fn exited(&self) -> bool {
        let mut slot = self.slot.lock().unwrap();
        match slot
            .as_mut()
            .filter(|process| Arc::ptr_eq(&process.state, &self.state))
        {
            Some(process) => !matches!(process.child.try_wait(), Ok(None)),
            None => true,
        }
    }
    pub fn packet(&self, wire: &[u8], parsed: Option<(bool, &[u8])>) {
        if let Some(trace) = self.state.trace.lock().unwrap().as_mut() {
            trace.packet(wire, parsed);
        }
    }
    pub fn text(&self, text: &str) {
        if let Some(trace) = self.state.trace.lock().unwrap().as_mut() {
            trace.text(text);
        }
    }
    pub fn closed(&self, code: Option<u16>, reason: &str) {
        let reason_known = matches!(
            reason,
            "" | "Unauthorized"
                | "device not ready"
                | "device session changed"
                | "message too large"
                | "client too slow"
                | "frame send failed"
                | "session is stopped"
                | "session is error"
                | "session is starting"
                | "Server stopping"
        );
        if let Some(trace) = self.state.trace.lock().unwrap().as_mut() {
            trace.terminal = Some(
                json!({"kind":"close","code":code,"reasonBytes":reason.len(),"knownReason":if reason_known {Some(reason)} else {None}}),
            );
        }
    }
    pub fn read_error(&self, category: &'static str) {
        if let Some(trace) = self.state.trace.lock().unwrap().as_mut() {
            trace.terminal = Some(json!({"kind":"read-error","category":category}));
        }
    }
    pub fn stop(&self) -> String {
        let child = {
            let mut slot = self.slot.lock().unwrap();
            if slot
                .as_ref()
                .is_some_and(|process| Arc::ptr_eq(&process.state, &self.state))
            {
                slot.take()
            } else {
                None
            }
        };
        drop(child); // Child kill/wait and stderr drain never hold the slot mutex.
        let stderr = self.state.stderr.lock().unwrap();
        String::from_utf8_lossy(&stderr.bytes.iter().copied().collect::<Vec<_>>()).into_owned()
    }
}
pub struct Running {
    pub attempt: Attempt,
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.attempt.stop();
    }
}
pub fn stop(slot: &Slot) {
    let child = { slot.lock().unwrap().take() };
    drop(child);
}
#[derive(Debug)]
pub enum StartError {
    Spawn(io::Error),
    Diagnostics(io::Error),
    Pipes,
}
impl std::fmt::Display for StartError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(error) => write!(output, "Decoder process: {error}"),
            Self::Diagnostics(error) => write!(output, "Decoder diagnostics: {error}"),
            Self::Pipes => write!(output, "Decoder pipes unavailable"),
        }
    }
}
impl std::error::Error for StartError {}
pub fn start(
    slot: &Slot,
    cancel: &AtomicBool,
    command: &mut Command,
) -> Result<Option<Running>, StartError> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    stop(slot);
    let trace = std::env::var_os("OMARCHY_T3_DEVICE_DIAGNOSTICS_DIR")
        .map(|path| {
            TRACES
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                    (count < TRACE_LIMIT).then_some(count + 1)
                })
                .map_err(|_| io::Error::other("Decoder diagnostic attempt budget exhausted"))?;
            Trace::open(Path::new(&path))
        })
        .transpose()
        .map_err(StartError::Diagnostics)?;
    let mut child = command.spawn().map_err(StartError::Spawn)?;
    let state = Arc::new(State {
        active: Mutex::new(true),
        stderr: Mutex::new(Tail::default()),
        trace: Mutex::new(trace),
    });
    let stderr = child.stderr.take();
    let target = state.clone();
    let stderr_worker = if let Some(mut stderr) = stderr {
        match std::thread::Builder::new()
            .name("device-decoder-stderr".into())
            .spawn(move || {
                let mut bytes = [0; 2048];
                while let Ok(length) = stderr.read(&mut bytes) {
                    if length == 0 {
                        break;
                    }
                    target.stderr.lock().unwrap().push(&bytes[..length]);
                }
            }) {
            Ok(worker) => Some(worker),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(StartError::Spawn(error));
            }
        }
    } else {
        None
    };
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let process = Process {
        child,
        state: state.clone(),
        stderr_worker,
        finished: false,
    };
    if stdin.is_none() || stdout.is_none() {
        drop(process);
        return Err(StartError::Pipes);
    }
    if !install(slot, cancel, process) {
        return Ok(None);
    }
    Ok(Some(Running {
        attempt: Attempt {
            slot: slot.clone(),
            state,
        },
        stdin,
        stdout,
    }))
}

fn install(slot: &Slot, cancel: &AtomicBool, process: Process) -> bool {
    let mut current = slot.lock().unwrap();
    if cancel.load(Ordering::Relaxed) {
        drop(current);
        drop(process);
        return false;
    }
    let previous = current.replace(process);
    drop(current);
    drop(previous);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        sync::atomic::AtomicUsize,
        time::{Duration, Instant},
    };
    fn isolated() {
        assert!(
            Path::new("/run/.containerenv").is_file(),
            "Decoder fixtures require private rootless Podman"
        );
    }
    fn command(code: &str) -> Command {
        let mut command = Command::new("python3");
        command
            .args(["-c", code])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        command
    }
    fn root() -> std::path::PathBuf {
        isolated();
        let root = std::env::temp_dir().join(format!("owned-decoder-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        root
    }
    #[test]
    fn replacement_reaps_owned_children_and_stale_attempt_cannot_stop_successor() {
        isolated();
        let slot = Slot::default();
        let cancel = AtomicBool::new(false);
        let first = start(&slot, &cancel, &mut command("import time;time.sleep(30)"))
            .unwrap()
            .unwrap();
        let first_pid = slot.lock().unwrap().as_ref().unwrap().child.id();
        let second = start(&slot, &cancel, &mut command("import time;time.sleep(30)"))
            .unwrap()
            .unwrap();
        let second_pid = slot.lock().unwrap().as_ref().unwrap().child.id();
        assert!(!Path::new(&format!("/proc/{first_pid}")).exists());
        first.attempt.stop();
        drop(first);
        assert!(!second.attempt.exited());
        stop(&slot);
        assert!(!Path::new(&format!("/proc/{second_pid}")).exists());
        drop(second);
    }
    #[test]
    fn failed_child_is_reaped_and_stderr_remains_bounded_and_observable() {
        isolated();
        let slot = Slot::default();
        let cancel = AtomicBool::new(false);
        let running=start(&slot,&cancel,&mut command("import sys;sys.stderr.write('x'*20000+'\\nInvalid argument codec fixture\\n');sys.stderr.flush();sys.exit(234)")).unwrap().unwrap();
        let pid = slot.lock().unwrap().as_ref().unwrap().child.id();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !running.attempt.exited() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let stderr = running.attempt.stop();
        assert!(stderr.len() <= STDERR_LIMIT);
        assert!(stderr.ends_with("Invalid argument codec fixture\n"));
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
        assert!(running.attempt.state.stderr.lock().unwrap().omitted > 0);
        drop(running);
    }
    #[test]
    fn cancelled_start_never_spawns_and_post_spawn_cancellation_reaps() {
        isolated();
        let slot = Slot::default();
        let cancel = AtomicBool::new(true);
        assert!(start(&slot, &cancel, &mut Command::new("/not-a-decoder"))
            .unwrap()
            .is_none());
        let child = command("import time;time.sleep(30)").spawn().unwrap();
        let pid = child.id();
        let state = Arc::new(State {
            active: Mutex::new(true),
            stderr: Mutex::new(Tail::default()),
            trace: Mutex::new(None),
        });
        assert!(!install(
            &slot,
            &cancel,
            Process {
                child,
                state,
                stderr_worker: None,
                finished: false
            }
        ));
        assert!(slot.lock().unwrap().is_none());
        assert!(!Path::new(&format!("/proc/{pid}")).exists());
    }
    #[test]
    fn reaping_and_stderr_join_do_not_hold_slot_mutex() {
        isolated();
        let slot = Slot::default();
        let child = command("import time;time.sleep(30)").spawn().unwrap();
        let state = Arc::new(State {
            active: Mutex::new(true),
            stderr: Mutex::new(Tail::default()),
            trace: Mutex::new(None),
        });
        let (reached_tx, reached_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            reached_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        *slot.lock().unwrap() = Some(Process {
            child,
            state,
            stderr_worker: Some(worker),
            finished: false,
        });
        let other = slot.clone();
        let reaper = std::thread::spawn(move || stop(&other));
        reached_rx.recv().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if slot.try_lock().is_ok_and(|slot| slot.is_none()) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(slot.try_lock().is_ok());
        release_tx.send(()).unwrap();
        reaper.join().unwrap();
    }
    #[test]
    fn stopped_attempt_cannot_publish_a_late_frame() {
        isolated();
        let slot = Slot::default();
        let cancel = AtomicBool::new(false);
        let running = start(&slot, &cancel, &mut command("import time;time.sleep(30)"))
            .unwrap()
            .unwrap();
        let count = AtomicUsize::new(0);
        running.attempt.publish(|| {
            count.fetch_add(1, Ordering::Relaxed);
        });
        stop(&slot);
        running.attempt.publish(|| {
            count.fetch_add(1, Ordering::Relaxed);
        });
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn diagnostic_files_are_private_capped_and_exclude_authority_fields() {
        let root = root();
        let mut trace = Trace::open(&root).unwrap();
        let packet = vec![7; PAYLOAD_LIMIT + 64];
        trace.packet(&packet, Some((true, &packet)));
        trace.text(r#"{"type":"config","width":480,"height":800,"url":"https://private.invalid/token","token":"private-authority-marker","code":"private-authority-marker"}"#);
        for _ in 0..EVENT_LIMIT + 10 {
            trace.text("not-json");
        }
        let mut stderr = Tail::default();
        stderr.push(&vec![b'x'; STDERR_LIMIT + 200]);
        trace.finish(123, None, false, &stderr);
        drop(trace);
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            let metadata = path.metadata().unwrap();
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
            assert!(metadata.len() <= PAYLOAD_LIMIT as u64);
            if path.extension().unwrap() == "json" {
                let text = std::fs::read_to_string(path).unwrap();
                assert!(
                    !text.contains("private-authority-marker") && !text.contains("private.invalid")
                );
                let value: Value = serde_json::from_str(&text).unwrap();
                assert_eq!(value["events"].as_array().unwrap().len(), EVENT_LIMIT);
                assert!(value["omittedEvents"].as_u64().unwrap() > 0);
                assert_eq!(value["omittedStderrBytes"], 200);
            }
        }
    }
    #[test]
    fn failed_capture_is_reported_without_claiming_unwritten_payload() {
        let root = root();
        let mut trace = Trace::open(&root).unwrap();
        let payload = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().unwrap() == "h264")
            .unwrap();
        trace.payload = File::open(payload).unwrap();
        trace.packet(&[1; 100], Some((true, &[1; 100])));
        trace.finish(123, None, false, &Tail::default());
        drop(trace);
        let report = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().unwrap() == "json")
            .unwrap();
        let value: Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
        assert_eq!(value["traceWriteFailed"], true);
        assert_eq!(value["payloadBytes"], 0);
    }
    #[test]
    fn unknown_close_reason_is_omitted_but_code_and_length_survive() {
        let root = root();
        let state = Arc::new(State {
            active: Mutex::new(true),
            stderr: Mutex::new(Tail::default()),
            trace: Mutex::new(Some(Trace::open(&root).unwrap())),
        });
        let attempt = Attempt {
            slot: Slot::default(),
            state: state.clone(),
        };
        attempt.closed(
            Some(1008),
            "https://private.invalid/private-authority-marker",
        );
        state
            .trace
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .finish(123, None, false, &Tail::default());
        let report = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().unwrap() == "json")
            .unwrap();
        let text = std::fs::read_to_string(report).unwrap();
        assert!(!text.contains("private.invalid") && !text.contains("private-authority-marker"));
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["transportTerminal"]["code"], 1008);
        assert!(value["transportTerminal"]["reasonBytes"].as_u64().unwrap() > 0);
        assert!(value["transportTerminal"]["knownReason"].is_null());
    }
    #[test]
    fn diagnostic_directory_refuses_owned_ancestor_links_and_broad_permissions() {
        let root = root();
        let private = root.join("private");
        std::fs::create_dir(&private).unwrap();
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
        let alias = root.join("alias");
        std::os::unix::fs::symlink(&private, &alias).unwrap();
        assert!(Trace::open(&alias).is_err());
        assert!(std::fs::read_dir(&private).unwrap().next().is_none());
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(Trace::open(&private).is_err());
    }
}
