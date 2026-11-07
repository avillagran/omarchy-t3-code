//! Opt-in shared WebSocket RPC. Hosts replace a failed session explicitly;
//! this owner never reconnects or replays a request.
use crate::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc, Condvar, Mutex, Weak,
    },
};

type Socket = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>;

/// Resource bounds for one physical RPC session. Responses share the frame limit;
/// completed unread calls retain their in-flight permit until their handle drops.
#[derive(Clone, Debug)]
pub struct SessionOptions {
    pub max_in_flight: usize,
    pub max_message_bytes: u64,
    pub max_queued_bytes: usize,
}
impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            max_in_flight: 16,
            max_message_bytes: MAX_BYTES,
            max_queued_bytes: 16 * 1024 * 1024,
        }
    }
}
impl SessionOptions {
    fn validate(&self) -> Result<()> {
        if self.max_in_flight == 0
            || self.max_in_flight > 128
            || !valid_limit(self.max_message_bytes)
            || self.max_queued_bytes == 0
            || self.max_queued_bytes > 64 * 1024 * 1024
        {
            return Err("invalid_session_options".into());
        }
        Ok(())
    }
}
struct Shared {
    id: String,
    closed: AtomicBool,
    count: AtomicUsize,
    queued_bytes: AtomicUsize,
    next: AtomicU64,
    commands: mpsc::SyncSender<Call>,
    options: SessionOptions,
    failure: Mutex<Option<String>>,
    completed: Mutex<bool>,
    finished: Condvar,
}
struct Lease(Arc<Shared>);
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.closed.store(true, Ordering::Release)
    }
}
struct Permit(Arc<Shared>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.count.fetch_sub(1, Ordering::AcqRel);
    }
}
struct QueuedBytes {
    shared: Arc<Shared>,
    len: usize,
}
impl Drop for QueuedBytes {
    fn drop(&mut self) {
        self.shared
            .queued_bytes
            .fetch_sub(self.len, Ordering::AcqRel);
    }
}
struct Call {
    id: String,
    message: Option<String>,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    reply: mpsc::SyncSender<Result<Value>>,
    _permit: Arc<Permit>,
    bytes: Option<QueuedBytes>,
    sent: bool,
}
/// One authenticated physical WebSocket and its opaque cache identity. Clones
/// share that socket. Last-owner drop or `close` stops it after a bounded I/O poll.
#[derive(Clone)]
pub struct RpcSession {
    lease: Arc<Lease>,
}
/// Lifecycle observer that does not retain a session owner. Waiting on it
/// cannot keep the socket alive after the last owner drops.
pub struct SessionClosed {
    shared: Arc<Shared>,
}
impl SessionClosed {
    pub fn wait(self) -> Result<Option<String>> {
        let mut done = self.shared.completed.lock().map_err(|_| "local_error")?;
        while !*done {
            done = self.shared.finished.wait(done).map_err(|_| "local_error")?;
        }
        self.shared
            .failure
            .lock()
            .map(|failure| failure.clone())
            .map_err(|_| "local_error".into())
    }
}
/// A request-scoped cancellation signal that does not retain a socket or permit.
/// Cloning or dropping this signal never cancels a request; call `cancel` explicitly.
#[derive(Clone)]
pub struct RpcCancellation {
    cancelled: Weak<AtomicBool>,
}
impl RpcCancellation {
    /// Best effort only: a transmitted mutation may already have executed.
    /// Completed requests and future requests on the same session are unaffected.
    pub fn cancel(&self) {
        if let Some(cancelled) = self.cancelled.upgrade() {
            cancelled.store(true, Ordering::Release)
        }
    }
}
/// An independently cancellable call. Dropping it cancels, without replay.
pub struct PendingRpc {
    reply: mpsc::Receiver<Result<Value>>,
    cancelled: Arc<AtomicBool>,
    _permit: Arc<Permit>,
}
impl PendingRpc {
    /// Retain this signal before moving the pending call into a blocking waiter.
    pub fn cancellation(&self) -> RpcCancellation {
        RpcCancellation {
            cancelled: Arc::downgrade(&self.cancelled),
        }
    }
    /// Cancellation does not imply the server did not execute a transmitted call.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release)
    }
    pub fn wait(self) -> Result<Value> {
        self.reply
            .recv()
            .map_err(|_| "rpc_outcome_unknown".to_owned())?
    }
}
impl Drop for PendingRpc {
    fn drop(&mut self) {
        self.cancel()
    }
}
impl Connection {
    /// Compare private route/authentication admission for a host session owner.
    /// This is not a session identity: equal admission can open distinct physical
    /// sockets. No credential or credential-derived fingerprint is exposed.
    pub fn same_session_admission(&self, other: &Self) -> bool {
        if self.base != other.base || self.ws != other.ws {
            return false;
        }
        match (&self.credential, &other.credential) {
            (Credential::Bearer(a), Credential::Bearer(b)) => a == b,
            (
                Credential::Dpop {
                    access_token: a,
                    key: ak,
                    expires_at: ae,
                },
                Credential::Dpop {
                    access_token: b,
                    key: bk,
                    expires_at: be,
                },
            ) => a == b && ae == be && Arc::ptr_eq(ak, bk),
            _ => false,
        }
    }
    /// Connect once. Route/auth changes must close this owner and open a new one.
    /// The existing one-shot `rpc` API remains unchanged.
    pub fn rpc_session(&self, options: SessionOptions) -> Result<RpcSession> {
        options.validate()?;
        let mut socket = self.websocket_with_limit(options.max_message_bytes)?;
        match socket.get_mut() {
            tungstenite::stream::MaybeTlsStream::Plain(s) => socket_timeouts(s),
            tungstenite::stream::MaybeTlsStream::Rustls(s) => socket_timeouts(&s.sock),
            _ => return Err("unsupported_tls".into()),
        }
        .map_err(|_| "disconnected")?;
        let (tx, rx) = mpsc::sync_channel(options.max_in_flight);
        let shared = Arc::new(Shared {
            id: uuid::Uuid::new_v4().to_string(),
            closed: AtomicBool::new(false),
            count: AtomicUsize::new(0),
            queued_bytes: AtomicUsize::new(0),
            next: AtomicU64::new(1),
            commands: tx,
            options,
            failure: Mutex::new(None),
            completed: Mutex::new(false),
            finished: Condvar::new(),
        });
        let owner = RpcSession {
            lease: Arc::new(Lease(shared.clone())),
        };
        std::thread::Builder::new()
            .name("t3-rpc-session".into())
            .spawn(move || run(socket, rx, shared))
            .map_err(|_| "local_error")?;
        Ok(owner)
    }
}
impl RpcSession {
    /// Random identity of this physical authenticated socket, never a credential.
    pub fn id(&self) -> &str {
        &self.lease.0.id
    }
    pub fn is_open(&self) -> bool {
        !self.lease.0.closed.load(Ordering::Acquire)
    }
    pub fn closed(&self) -> SessionClosed {
        SessionClosed {
            shared: self.lease.0.clone(),
        }
    }
    pub fn close(&self) {
        self.lease.0.closed.store(true, Ordering::Release)
    }
    /// Terminal transport error, without server text or credentials. A rejected
    /// application request does not fail the session.
    pub fn failure(&self) -> Option<String> {
        self.lease.0.failure.lock().ok().and_then(|v| v.clone())
    }
    /// Queue one bounded unary request. Its deadline includes queueing. The
    /// handle must be kept until the result is needed; dropping it cancels it.
    pub fn start(&self, tag: &str, payload: Value, timeout: Duration) -> Result<PendingRpc> {
        let s = &self.lease.0;
        if !self.is_open() {
            return Err("rpc_session_closed".into());
        }
        if tag.is_empty()
            || tag.len() > 256
            || tag.chars().any(char::is_control)
            || timeout.is_zero()
            || timeout > Duration::from_secs(300)
        {
            return Err("invalid_rpc_request".into());
        }
        if !reserve(&s.count, 1, s.options.max_in_flight) {
            return Err("rpc_session_busy".into());
        }
        let permit = Arc::new(Permit(s.clone()));
        let serial = next_id(&s.next).ok_or("rpc_session_exhausted")?;
        let id = serial.to_string();
        let mut writer = FrameWriter {
            bytes: Vec::new(),
            limit: s.options.max_message_bytes as usize,
        };
        serde_json::to_writer(
            &mut writer,
            &json!({"_tag":"Request","id":id,"tag":tag,"payload":payload,"headers":[]}),
        )
        .map_err(|_| "rpc_request_too_large")?;
        let message = String::from_utf8(writer.bytes).map_err(|_| "invalid_rpc_request")?;
        if !reserve(&s.queued_bytes, message.len(), s.options.max_queued_bytes) {
            return Err("rpc_session_busy".into());
        }
        let bytes = QueuedBytes {
            shared: s.clone(),
            len: message.len(),
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::sync_channel(1);
        let call = Call {
            id,
            message: Some(message),
            deadline: Instant::now() + timeout,
            cancelled: cancelled.clone(),
            reply: tx,
            _permit: permit.clone(),
            bytes: Some(bytes),
            sent: false,
        };
        s.commands
            .try_send(call)
            .map_err(|_| "rpc_session_closed")?;
        Ok(PendingRpc {
            reply: rx,
            cancelled,
            _permit: permit,
        })
    }
    pub fn rpc(&self, tag: &str, payload: Value, timeout: Duration) -> Result<Value> {
        self.start(tag, payload, timeout)?.wait()
    }
    /// Validate a registered unary function with the same payload builder and
    /// default deadline as the existing one-shot API. Composed reads and streams
    /// remain on their explicit APIs; they cannot masquerade as unary requests.
    pub fn start_function(
        &self,
        name: &str,
        args: &Value,
        timeout: Option<Duration>,
    ) -> Result<PendingRpc> {
        let function = crate::api::function(name).ok_or("unknown_function")?;
        let payload = function.payload(args)?;
        if function.stream {
            return Err("stream_function".into());
        }
        let tag = function.tag.ok_or("session_requires_unary_rpc")?;
        self.start(tag, payload, timeout.unwrap_or(function.timeout()))
    }
    pub fn call_function(
        &self,
        name: &str,
        args: &Value,
        timeout: Option<Duration>,
    ) -> Result<Value> {
        self.start_function(name, args, timeout)?.wait()
    }
}
fn reserve(counter: &AtomicUsize, amount: usize, limit: usize) -> bool {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_add(amount).filter(|n| *n <= limit) else {
            return false;
        };
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
    }
}
fn next_id(counter: &AtomicU64) -> Option<u64> {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        let next = current.checked_add(1)?;
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Some(current),
            Err(actual) => current = actual,
        }
    }
}
struct FrameWriter {
    bytes: Vec<u8>,
    limit: usize,
}
impl std::io::Write for FrameWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(buf.len()) > self.limit {
            return Err(std::io::Error::other("frame_limit"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn socket_timeouts(socket: &TcpStream) -> std::io::Result<()> {
    socket.set_read_timeout(Some(Duration::from_millis(25)))?;
    socket.set_write_timeout(Some(Duration::from_millis(100)))
}
fn send(socket: &mut Socket, value: Value) -> Result<()> {
    socket
        .send(Message::Text(value.to_string().into()))
        .map_err(|_| "rpc_connection_failed".into())
}
fn finish(call: Call, result: Result<Value>) {
    let Call { reply, _permit, .. } = call;
    // Release the worker's permit before waking a consuming caller. Its handle
    // still holds the same permit, so unread completed replies remain bounded.
    drop(_permit);
    let _ = reply.try_send(result);
}
fn expired(call: &Call) -> Option<&'static str> {
    if call.cancelled.load(Ordering::Acquire) {
        Some(if call.sent {
            "rpc_cancelled_outcome_unknown"
        } else {
            "rpc_cancelled"
        })
    } else if Instant::now() >= call.deadline {
        Some(if call.sent {
            "rpc_outcome_unknown"
        } else {
            "rpc_timeout"
        })
    } else {
        None
    }
}
fn run(mut socket: Socket, rx: mpsc::Receiver<Call>, shared: Arc<Shared>) {
    let mut pending = HashMap::<String, Call>::new();
    let mut last_frame = Instant::now();
    let mut last_ping = last_frame;
    let result = (|| -> Result<()> {
        while !shared.closed.load(Ordering::Acquire) {
            let expired_ids = pending
                .iter()
                .filter_map(|(id, c)| expired(c).map(|reason| (id.clone(), reason)))
                .collect::<Vec<_>>();
            for (id, reason) in expired_ids {
                if let Some(c) = pending.remove(&id) {
                    send(&mut socket, json!({"_tag":"Interrupt","requestId":id}))?;
                    finish(c, Err(reason.into()));
                }
            }
            while let Ok(mut call) = rx.try_recv() {
                if shared.closed.load(Ordering::Acquire) {
                    finish(call, Err("rpc_session_closed".into()));
                    continue;
                }
                if let Some(reason) = expired(&call) {
                    finish(call, Err(reason.into()));
                    continue;
                }
                let message = call.message.take().expect("queued frame");
                // Sending may partially write. From this point every loss is uncertain.
                call.sent = true;
                let id = call.id.clone();
                pending.insert(id.clone(), call);
                socket
                    .send(Message::Text(message.into()))
                    .map_err(|_| "rpc_connection_failed")?;
                pending.get_mut(&id).expect("pending frame").bytes.take();
            }
            match socket.read() {
                Ok(Message::Text(text)) => {
                    last_frame = Instant::now();
                    let v: Value = serde_json::from_str(&text).map_err(|_| "malformed_rpc")?;
                    match v["_tag"].as_str() {
                        Some("Exit") => {
                            let id = v["requestId"].as_str().ok_or("malformed_rpc")?;
                            if let Some(c) = pending.remove(id) {
                                let result = if let Some(reason) = expired(&c) {
                                    Err(reason.into())
                                } else {
                                    match v["exit"]["_tag"].as_str() {
                                        Some("Success") => Ok(v["exit"]["value"].clone()),
                                        Some("Failure") => Err(rejection(&v["exit"]["cause"])),
                                        _ => Err("rpc_outcome_unknown".into()),
                                    }
                                };
                                finish(c, result)
                            }
                        }
                        Some("Ping") => send(&mut socket, json!({"_tag":"Pong"}))?,
                        Some("Pong") => {}
                        _ => return Err("malformed_rpc".into()),
                    }
                }
                Ok(Message::Ping(p)) => {
                    last_frame = Instant::now();
                    socket
                        .send(Message::Pong(p))
                        .map_err(|_| "rpc_connection_failed")?;
                }
                Ok(Message::Pong(_)) => last_frame = Instant::now(),
                Ok(Message::Close(_)) => return Err("disconnected".into()),
                Ok(_) => return Err("malformed_rpc".into()),
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => return Err("disconnected".into()),
            }
            if last_frame.elapsed() > crate::stream::STREAM_IDLE_TIMEOUT {
                return Err("stream_timeout".into());
            }
            if last_ping.elapsed() >= crate::stream::STREAM_PING_INTERVAL {
                send(&mut socket, json!({"_tag":"Ping"}))?;
                last_ping = Instant::now();
            }
        }
        Ok(())
    })();
    shared.closed.store(true, Ordering::Release);
    if let Err(error) = result {
        if let Ok(mut failure) = shared.failure.lock() {
            *failure = Some(error)
        }
    }
    for (_, call) in pending {
        finish(call, Err("rpc_outcome_unknown".into()));
    }
    while let Ok(call) = rx.try_recv() {
        finish(call, Err("rpc_session_closed".into()));
    }
    let _ = socket.close(None);
    if let Ok(mut completed) = shared.completed.lock() {
        *completed = true;
        shared.finished.notify_all();
    }
}
