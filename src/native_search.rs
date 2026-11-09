//! Original Nightly search in the application's admitted local V8 runtime.
//! Only metadata crosses the child pipe; the executable and program are factory-owned.
use crate::voice_capture::resources::Runtime;
use crate::{json, text, Result};
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{Read, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{mpsc, Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_PACKET: usize = 16 * 1024 * 1024;
const MAX_PENDING: usize = 32;
const REQUEST_TIME: Duration = Duration::from_secs(10);

/// Preserve the original search inputs while omitting unrelated PR watch data.
/// Missing authored-message timestamps stay absent for native older-server behavior.
pub fn thread_shell(thread: &Value, environment: &str) -> Value {
    let mut result = json!({"environmentId":environment});
    for key in [
        "id",
        "title",
        "branch",
        "createdAt",
        "updatedAt",
        "latestUserMessageAt",
        "latestUserAuthoredMessageAt",
        "lastVisitedAt",
        "opens",
    ] {
        if let Some(value) = thread.get(key) {
            result[key] = value.clone();
        }
    }
    let link = |value: &Value| {
        let mut result = json!({});
        for key in ["number", "repository", "url", "source", "linkedAt"] {
            if let Some(value) = value.get(key) {
                result[key] = value.clone();
            }
        }
        if let Some(snapshot) = value.get("snapshot").filter(|value| value.is_object()) {
            result["snapshot"] =
                json!({"title":snapshot["title"],"updatedAt":snapshot["updatedAt"]});
        }
        result
    };
    if let Some(links) = thread.get("pullRequests").and_then(Value::as_array) {
        result["pullRequests"] = Value::Array(links.iter().map(link).collect());
    }
    for key in ["linkedPullRequest", "branchPullRequest"] {
        if let Some(value) = thread.get(key) {
            result[key] = if value.is_null() {
                Value::Null
            } else {
                link(value)
            };
        }
    }
    result
}

pub struct Engine {
    runtime: Arc<Runtime>,
    child: Child,
    input: Option<ChildStdin>,
    output: ChildStdout,
    buffered: Vec<u8>,
    sequence: u64,
    broken: bool,
    terminal: bool,
}
impl Engine {
    /// Explicit native factory admission, also used by private integration fixtures.
    pub fn launch(runtime: Arc<Runtime>) -> Result<Self> {
        runtime
            .verify()
            .map_err(|_| "native_search_runtime_unavailable")?;
        let source = concat!(
            include_str!("../ui/search/NativeSearchRuntime.cjs"),
            "\n",
            include_str!("../ui/search/search-worker.cjs")
        );
        let parent_pid = unsafe { libc::getpid() };
        let mut command = Command::new(runtime.executable());
        command
            .arg("--eval")
            .arg(source)
            .env_clear()
            .env("ELECTRON_RUN_AS_NODE", "1")
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("HOME", "/nonexistent")
            .env("TMPDIR", "/nonexistent")
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0
                    || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent_pid
                {
                    return Err(std::io::Error::other(
                        "Native search process ownership failed",
                    ));
                }
                Ok(())
            });
        }
        let mut child = command.spawn().map_err(|_| "native_search_start_failed")?;
        let input = child.stdin.take().expect("Owned piped stdin");
        let output = child.stdout.take().expect("Owned piped stdout");
        let mut engine = Self {
            runtime,
            child,
            input: Some(input),
            output,
            buffered: Vec::new(),
            sequence: 0,
            broken: false,
            terminal: false,
        };
        for descriptor in [
            engine.output.as_raw_fd(),
            engine.input.as_ref().unwrap().as_raw_fd(),
        ] {
            let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
            if flags < 0
                || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                engine.broken = true;
                return Err("native_search_pipe_unavailable".into());
            }
        }
        let ready = engine.read_packet(Instant::now() + REQUEST_TIME);
        if !ready.is_ok_and(|packet| {
            packet
                == json!({"type":"ready",
            "protocol":"o3-native-search-v1","electron":"44.10.1"})
        }) {
            engine.broken = true;
            return Err("native_search_start_failed".into());
        }
        engine
            .runtime
            .verify()
            .map_err(|_| "native_search_runtime_unavailable")?;
        Ok(engine)
    }
    fn discover() -> Result<Self> {
        let runtime = if let Some(path) = std::env::var_os("O3_NATIVE_ELECTRON_RUNTIME") {
            PathBuf::from(path)
        } else {
            std::env::current_exe()
                .map_err(|_| "native_search_runtime_unavailable")?
                .parent()
                .ok_or("native_search_runtime_unavailable")?
                .join("native-voice/electron-44.10.1-linux-x64")
        };
        let runtime =
            Arc::new(Runtime::admit(&runtime).map_err(|_| "native_search_runtime_unavailable")?);
        Self::launch(runtime)
    }
    fn write_packet(&mut self, bytes: &[u8], deadline: Instant) -> Result<()> {
        let input = self
            .input
            .as_mut()
            .ok_or("native_search_process_unavailable")?;
        let mut offset = 0;
        while offset < bytes.len() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("native_search_timeout".into());
            }
            match input.write(&bytes[offset..]) {
                Ok(0) => return Err("native_search_pipe_unavailable".into()),
                Ok(count) => offset += count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    let mut descriptor = libc::pollfd {
                        fd: input.as_raw_fd(),
                        events: libc::POLLOUT,
                        revents: 0,
                    };
                    let waited = unsafe {
                        libc::poll(&mut descriptor, 1, remaining.as_millis().min(100) as i32)
                    };
                    if waited < 0
                        && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
                    {
                        return Err("native_search_pipe_unavailable".into());
                    }
                }
                Err(_) => return Err("native_search_pipe_unavailable".into()),
            }
        }
        Ok(())
    }
    fn read_packet(&mut self, deadline: Instant) -> Result<Value> {
        loop {
            if let Some(end) = self.buffered.iter().position(|byte| *byte == b'\n') {
                let bytes = self.buffered.drain(..=end).collect::<Vec<_>>();
                return serde_json::from_slice(&bytes)
                    .map_err(|_| "native_search_protocol_failed".into());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("native_search_timeout".into());
            }
            let mut descriptor = libc::pollfd {
                fd: self.output.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let waited =
                unsafe { libc::poll(&mut descriptor, 1, remaining.as_millis().min(100) as i32) };
            if waited < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err("native_search_pipe_unavailable".into());
            }
            if waited == 0 {
                continue;
            }
            let mut bytes = [0u8; 65536];
            match self.output.read(&mut bytes) {
                Ok(0) => return Err("native_search_process_ended".into()),
                Ok(count) => {
                    if self.buffered.len() + count > MAX_PACKET {
                        return Err("native_search_packet_too_large".into());
                    }
                    self.buffered.extend_from_slice(&bytes[..count]);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => return Err("native_search_pipe_unavailable".into()),
            }
        }
    }
    fn exchange(&mut self, mut input: Value) -> Result<Value> {
        if self.broken || self.terminal {
            return Err("native_search_process_unavailable".into());
        }
        self.runtime
            .verify()
            .map_err(|_| "native_search_runtime_unavailable")?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("native_search_sequence_exhausted")?;
        let id = self.sequence.to_string();
        input["id"] = json!(id);
        let mut bytes = serde_json::to_vec(&input).map_err(|_| "invalid_native_search_request")?;
        if bytes.len() >= MAX_PACKET {
            return Err("native_search_packet_too_large".into());
        }
        bytes.push(b'\n');
        let deadline = Instant::now() + REQUEST_TIME;
        self.write_packet(&bytes, deadline)?;
        let response = self.read_packet(deadline)?;
        if response["id"] != id || !self.buffered.is_empty() {
            return Err("native_search_protocol_failed".into());
        }
        self.runtime
            .verify()
            .map_err(|_| "native_search_runtime_unavailable")?;
        Ok(response)
    }
    pub fn call(&mut self, input: Value) -> Result<Value> {
        if !input.is_object() {
            return Err("invalid_native_search_request".into());
        }
        let response = match self.exchange(input) {
            Ok(response) => response,
            Err(error) => {
                self.broken = true;
                return Err(error);
            }
        };
        if response.get("error").is_some() {
            return Err("native_search_request_failed".into());
        }
        if let Some(value) = response.get("resultJson").and_then(Value::as_str) {
            return Ok(json!({"resultJson":value}));
        }
        response.get("result").cloned().ok_or_else(|| {
            self.broken = true;
            "native_search_protocol_failed".into()
        })
    }
    /// Return only after this exact child was reaped. Private fixture data is retained.
    pub fn close(&mut self) -> Result<()> {
        if self.terminal {
            return Ok(());
        }
        if !self.broken && self.call(json!({"op":"quit"})).is_err() {
            self.broken = true;
        }
        self.input.take();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.terminal = true;
                    return if status.success() && !self.broken {
                        Ok(())
                    } else {
                        Err("native_search_close_failed".into())
                    };
                }
                Ok(None) if !self.broken && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5))
                }
                Ok(None) => {
                    self.child
                        .kill()
                        .map_err(|_| "native_search_close_failed")?;
                    self.child
                        .wait()
                        .map_err(|_| "native_search_close_failed")?;
                    self.terminal = true;
                    return Err("native_search_close_failed".into());
                }
                Err(_) => return Err("native_search_close_failed".into()),
            }
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

struct Queue {
    pending: VecDeque<Value>,
    closed: bool,
}
struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
}
pub struct Manager {
    shared: Arc<Shared>,
    task: Option<JoinHandle<Result<()>>>,
    events: mpsc::Sender<Value>,
}
fn completed(events: &mpsc::Sender<Value>, request: &Value, result: Result<Value>) {
    let _ = events.send(
        json!({"op":"native-search-result","requestId":request["requestId"],
        "result":result.as_ref().ok(),"error":result.as_ref().err()}),
    );
}
impl Manager {
    pub fn new(events: mpsc::Sender<Value>) -> Result<Self> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                pending: VecDeque::new(),
                closed: false,
            }),
            changed: Condvar::new(),
        });
        let state = shared.clone();
        let replies = events.clone();
        let task = thread::Builder::new()
            .name("native-search".into())
            .spawn(move || {
                let mut engine = None;
                loop {
                    let next = {
                        let mut queue = state
                            .queue
                            .lock()
                            .map_err(|_| "native_search_queue_unavailable")?;
                        while queue.pending.is_empty() && !queue.closed {
                            queue = state
                                .changed
                                .wait(queue)
                                .map_err(|_| "native_search_queue_unavailable")?;
                        }
                        if queue.closed {
                            for request in queue.pending.drain(..) {
                                completed(&events, &request, Err("native_search_closed".into()));
                            }
                            None
                        } else {
                            queue.pending.pop_front()
                        }
                    };
                    let Some(request) = next else { break };
                    let mut input = request.clone();
                    input["op"] = request["action"].clone();
                    let result = (|| {
                        if engine.is_none() {
                            engine = Some(Engine::discover()?);
                        }
                        engine.as_mut().unwrap().call(input)
                    })();
                    completed(&events, &request, result);
                }
                if let Some(mut engine) = engine {
                    engine.close()?;
                }
                Ok(())
            })
            .map_err(|_| "native_search_queue_unavailable")?;
        Ok(Self {
            shared,
            task: Some(task),
            events: replies,
        })
    }
    pub fn submit(&self, input: Value) -> Result<()> {
        if text(&input, "op") != "native-search"
            || text(&input, "requestId").is_empty()
            || !matches!(
                text(&input, "action"),
                "snapshot" | "open" | "palette" | "sidebar" | "query"
            )
        {
            return Err("invalid_native_search_request".into());
        }
        let mut queue = self
            .shared
            .queue
            .lock()
            .map_err(|_| "native_search_queue_unavailable")?;
        if queue.closed {
            return Err("native_search_closed".into());
        }
        // Only obsolete pending rankings coalesce. Opens retain arrival order
        // and every removed request receives a terminal reply for its UI waiter.
        let superseded = if matches!(text(&input, "action"), "palette" | "sidebar") {
            queue
                .pending
                .iter()
                .position(|pending| pending["action"] == input["action"])
                .and_then(|index| queue.pending.remove(index))
        } else {
            None
        };
        if queue.pending.len() >= MAX_PENDING {
            return Err("native_search_busy".into());
        }
        queue.pending.push_back(input);
        self.shared.changed.notify_one();
        drop(queue);
        if let Some(request) = superseded {
            completed(
                &self.events,
                &request,
                Err("native_search_superseded".into()),
            );
        }
        Ok(())
    }
    pub fn close(&mut self) -> Result<()> {
        {
            let mut queue = self
                .shared
                .queue
                .lock()
                .map_err(|_| "native_search_queue_unavailable")?;
            queue.closed = true;
            self.shared.changed.notify_one();
        }
        match self.task.take() {
            Some(task) => task.join().map_err(|_| "native_search_queue_failed")?,
            None => Ok(()),
        }
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
