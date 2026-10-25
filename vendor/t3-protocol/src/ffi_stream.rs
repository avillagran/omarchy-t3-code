//! Bounded foreign-host event inbox. No host callbacks or unbounded worker queues.
use crate::{api, error_code, stream::{self, Start, Tick}, Connection, Result, MAX_BYTES};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};
pub(crate) const MAX_ITEMS: usize = 64;
/// Streams per handle (shell, open thread, command output, config, preview, terminal,
/// devices…), each with its own WebSocket: T3 serves one request per socket.
pub(crate) const MAX_SUBSCRIPTIONS: usize = 8;
/// Of those, streams in the 64 MiB bounded-snapshot mode. Queued payload per handle
/// stays under 2 × 72 MiB + 6 × 8 MiB = 192 MiB of serialized JSON.
pub(crate) const MAX_LARGE_SUBSCRIPTIONS: usize = 2;
/// Per-subscription bounds, fixed by its mode when it starts. Sizes are serialized JSON.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Limits {
    /// Largest single item: the same limit the WebSocket frame was read with.
    pub(crate) item: usize,
    /// Every queued, not yet polled item together.
    pub(crate) queue: usize,
}
impl Limits {
    pub(crate) const NORMAL: Self = Self { item: MAX_BYTES as usize, queue: MAX_BYTES as usize };
    /// 8 MiB everywhere except `orchestration.subscribeThread` with `acceptBoundedSnapshot`:
    /// that one item may be a 64 MiB snapshot, and the queue keeps a normal stream's
    /// 8 MiB of events behind it (72 MiB).
    pub(crate) fn of(tag: &str, payload: &Value) -> Self {
        let item = stream::item_limit(tag, payload) as usize;
        if item > Self::NORMAL.item {
            Self { item, queue: item + Self::NORMAL.queue }
        } else {
            Self::NORMAL
        }
    }
    pub(crate) fn large(self) -> bool {
        self != Self::NORMAL
    }
}
impl Default for Limits {
    fn default() -> Self {
        Self::NORMAL
    }
}
/// Counts serialized bytes without building a copy of a (possibly 64 MiB) item.
struct Counter(usize);
impl std::io::Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[derive(Default)]
struct Inbox {
    items: VecDeque<(Value, usize)>,
    bytes: usize,
    /// End offsets into `items` of the server chunks that ended since the last poll.
    boundaries: Vec<usize>,
    /// Items of the chunk still being delivered, counted across polls.
    open_chunk: usize,
    /// Caught up: the last chunk ended and a read tick passed without a frame.
    idle: bool,
    /// `idle` turned true after the last poll; wakes one waiting poll.
    idle_unreported: bool,
    /// Sticky once the server's completion marker item was queued.
    completion_marker: bool,
    /// Where to resume: the payload's `afterSequence`, advanced over every polled item.
    cursor: Option<u64>,
    /// What the first polled item meant (`stream::start_of`), once there was one.
    start: Option<Start>,
    first_polled: bool,
    closed: bool,
    error: Option<String>,
}
#[derive(Default)]
struct Shared {
    inbox: Mutex<Inbox>,
    ready: Condvar,
    limits: Limits,
    /// The payload asked for `requestCompletionMarker`, so `{"kind":"synchronized"}` is one.
    marker_requested: bool,
    /// Wire tag, for `stream::advance` and `stream::start_of`.
    tag: String,
}
impl Shared {
    fn new(limits: Limits, marker_requested: bool) -> Self {
        Self { limits, marker_requested, ..Self::default() }
    }
    fn of(tag: &str, payload: &Value) -> Self {
        let shared = Self { tag: tag.into(), ..Self::new(Limits::of(tag, payload), payload["requestCompletionMarker"] == true) };
        if let Ok(mut inbox) = shared.inbox.lock() {
            inbox.cursor = payload["afterSequence"].as_u64();
        }
        shared
    }
    fn push(&self, item: Value) -> Result<()> {
        let mut size = Counter(0);
        serde_json::to_writer(&mut size, &item).map_err(|_| "malformed_stream")?;
        let size = size.0;
        let mut inbox = self.inbox.lock().map_err(|_| "internal_error")?;
        if inbox.closed {
            return Err("stream_cancelled".into());
        }
        if inbox.items.len() >= MAX_ITEMS
            || size > self.limits.item
            || size > self.limits.queue - inbox.bytes
        {
            return Err("stream_backpressure".into());
        }
        if self.marker_requested && item["kind"] == "synchronized" {
            inbox.completion_marker = true;
        }
        inbox.bytes += size;
        inbox.items.push_back((item, size));
        inbox.open_chunk += 1;
        inbox.idle = false;
        self.ready.notify_all();
        Ok(())
    }
    fn chunk_end(&self) {
        if let Ok(mut inbox) = self.inbox.lock() {
            // An empty chunk adds no boundary, so offsets stay strictly ascending.
            if inbox.open_chunk > 0 && !inbox.closed {
                inbox.open_chunk = 0;
                let end = inbox.items.len();
                inbox.boundaries.push(end);
                self.ready.notify_all();
            }
        }
    }
    fn idle(&self) {
        if let Ok(mut inbox) = self.inbox.lock() {
            if !inbox.idle && !inbox.closed {
                inbox.idle = true;
                inbox.idle_unreported = true;
                self.ready.notify_all();
            }
        }
    }
    fn finish(&self, error: Option<String>) {
        if let Ok(mut inbox) = self.inbox.lock() {
            inbox.closed = true;
            inbox.idle = false;
            inbox.error = error;
            self.ready.notify_all();
        }
    }
}
pub(crate) struct Subscription {
    shared: Arc<Shared>,
    cancel: Arc<AtomicBool>,
    /// When the host last asked to probe the socket; the worker takes it on its next tick.
    wake: Arc<Mutex<Option<Instant>>>,
}
impl Subscription {
    pub(crate) fn start(connection: Connection, tag: String, payload: Value) -> Result<Self> {
        let shared = Arc::new(Shared::of(&tag, &payload));
        let cancel = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(Mutex::new(None));
        let (queue, stop, woken) = (shared.clone(), cancel.clone(), wake.clone());
        std::thread::Builder::new()
            .name("t3-events".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    connection.subscribe_ticks(
                        &tag,
                        payload,
                        || stop.load(Ordering::Acquire),
                        || woken.lock().ok()?.take(),
                        |tick| {
                            match tick {
                                Tick::Item(value) => queue.push(value)?,
                                Tick::ChunkEnd => queue.chunk_end(),
                                Tick::Idle => queue.idle(),
                            }
                            Ok(())
                        },
                    )
                }))
                .unwrap_or_else(|_| Err("internal_error".into()));
                queue.finish(if stop.load(Ordering::Acquire) {
                    None
                } else {
                    result.err()
                });
            })
            .map_err(|_| "local_error")?;
        Ok(Self { shared, cancel, wake })
    }
    fn limits(&self) -> Limits {
        self.shared.limits
    }
    /// Pings at once on the worker's next read tick (at most 100 ms) and ends the stream
    /// with `stream_timeout` unless a frame arrives within STREAM_PROBE_TIMEOUT of this
    /// call. Returns false for a stream that already ended. Never reconnects.
    pub(crate) fn wake(&self) -> bool {
        let open = self.shared.inbox.lock().is_ok_and(|inbox| !inbox.closed);
        if open {
            if let Ok(mut asked) = self.wake.lock() {
                asked.get_or_insert_with(Instant::now);
            }
        }
        open
    }

    pub(crate) fn poll(&self, timeout_ms: u64) -> Result<Value> {
        let inbox = self.shared.inbox.lock().map_err(|_| "internal_error")?;
        let (mut inbox, _) = self
            .shared
            .ready
            .wait_timeout_while(inbox, Duration::from_millis(timeout_ms), |v| {
                v.items.is_empty() && v.boundaries.is_empty() && !v.idle_unreported && !v.closed
            })
            .map_err(|_| "internal_error")?;
        let items: Vec<_> = inbox.items.drain(..).map(|(v, _)| v).collect();
        let tag = self.shared.tag.as_str();
        for item in &items {
            if !inbox.first_polled {
                inbox.first_polled = true;
                inbox.start = stream::start_of(tag, item);
            }
            inbox.cursor = stream::advance(tag, inbox.cursor, item);
        }
        let boundaries = std::mem::take(&mut inbox.boundaries);
        inbox.bytes = 0;
        inbox.idle_unreported = false;
        // `error` stays a stable code; a reason after it goes to `errorMessage`.
        let code = inbox.error.as_deref().map(error_code);
        let message = inbox.error.as_deref().zip(code).and_then(|(e, c)| e.get(c.len() + 2..));
        Ok(json!({"items":items,"boundaries":boundaries,"idle":inbox.idle,
            "completionMarker":inbox.completion_marker,
            "closed":inbox.closed,"error":code,"errorMessage":message,
            "cursor":inbox.cursor,"start":inbox.start.map(Start::as_str)}))
    }
    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
        if let Ok(mut inbox) = self.shared.inbox.lock() {
            inbox.items.clear();
            inbox.boundaries.clear();
            inbox.bytes = 0;
            inbox.idle = false;
            inbox.closed = true;
            inbox.error = None;
            self.shared.ready.notify_all();
        }
    }
}
/// Stream limits and poll fields for `capabilities`; all additive to ABI v2.
pub(crate) fn with_capabilities(mut capabilities: Value) -> Value {
    let large = Limits::of("orchestration.subscribeThread", &json!({"acceptBoundedSnapshot":true}));
    for (key, value) in [
        ("maxSubscriptions", json!(MAX_SUBSCRIPTIONS)),
        ("maxLargeSubscriptions", json!(MAX_LARGE_SUBSCRIPTIONS)),
        ("queueMaxItems", json!(MAX_ITEMS)),
        ("queueMaxBytes", json!(Limits::NORMAL.queue)),
        ("itemMaxBytes", json!(Limits::NORMAL.item)),
        ("largeItemMaxBytes", json!(large.item)),
        ("largeQueueMaxBytes", json!(large.queue)),
        ("maxPollMs", json!(1000)),
        ("probeTimeoutMs", json!(stream::STREAM_PROBE_TIMEOUT.as_millis() as u64)),
        ("idleTimeoutMs", json!(stream::STREAM_IDLE_TIMEOUT.as_millis() as u64)),
        ("pollFields", json!(["items", "boundaries", "idle", "completionMarker", "closed", "error", "errorMessage", "cursor", "start"])),
        ("subscribeBy", json!(["tag", "fn"])),
    ] {
        capabilities[key] = value;
    }
    capabilities
}
/// What `subscribe` opens: `fn` + `args` through the typed registry (validated,
/// before any connection is looked up), or the raw `tag` + `payload`, unchecked
/// apart from the tag itself, as before ABI 3. Both at once is `invalid_input`.
pub(crate) fn target(request: &Value) -> Result<(String, Value)> {
    let Some(name) = request.get("fn") else {
        let tag = crate::field(request, "tag")?;
        if tag.len() > 256 || tag.chars().any(char::is_control) {
            return Err("invalid_input".into());
        }
        return Ok((tag.into(), request["payload"].clone()));
    };
    if request.get("tag").is_some() || request.get("payload").is_some() {
        return Err("invalid_input".into());
    }
    let name = name.as_str().filter(|s| !s.is_empty()).ok_or("invalid_input")?;
    let args = match request.get("args") {
        None | Some(Value::Null) => json!({}),
        Some(args) => args.clone(),
    };
    let spec = api::streams::spec(name, &args)?;
    Ok((spec.tag().into(), spec.payload))
}
/// A slot for one more stream, or `subscription_limit`. Ended streams keep their slot
/// until the host cancels them.
pub(crate) fn admit(
    subscriptions: &HashMap<String, Arc<Subscription>>,
    tag: &str,
    payload: &Value,
) -> Result<()> {
    let large = || subscriptions.values().filter(|s| s.limits().large()).count();
    if subscriptions.len() >= MAX_SUBSCRIPTIONS
        || Limits::of(tag, payload).large() && large() >= MAX_LARGE_SUBSCRIPTIONS
    {
        return Err("subscription_limit".into());
    }
    Ok(())
}
/// `{"op":"wakeSubscriptions"}` probes every stream of the handle; with
/// `"subscriptionId"` only that one. Returns `{"probed": open streams probed}`.
pub(crate) fn wake_subscriptions(
    subscriptions: &HashMap<String, Arc<Subscription>>,
    request: &Value,
) -> Result<Value> {
    let probed = match request.get("subscriptionId") {
        None | Some(Value::Null) => subscriptions.values().filter(|s| s.wake()).count(),
        Some(Value::String(id)) => {
            usize::from(subscriptions.get(id).ok_or("unknown_subscription")?.wake())
        }
        Some(_) => return Err("invalid_input".into()),
    };
    Ok(json!({ "probed": probed }))
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.cancel();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_never_silently_drop_events() {
        let shared = Shared::default();
        for n in 0..MAX_ITEMS {
            shared.push(json!({"sequence":n})).unwrap();
        }
        assert_eq!(
            shared.push(json!({"sequence":64})).unwrap_err(),
            "stream_backpressure"
        );
        assert_eq!(shared.inbox.lock().unwrap().items.len(), MAX_ITEMS);
        let shared = Shared::default();
        assert_eq!(
            shared
                .push(json!("a".repeat(MAX_BYTES as usize)))
                .unwrap_err(),
            "stream_backpressure"
        );
        assert!(shared.inbox.lock().unwrap().items.is_empty());
    }
    #[test]
    fn only_a_bounded_thread_subscription_gets_the_large_item_limit() {
        let bounded = json!({"threadId":"t","acceptBoundedSnapshot":true});
        let large = Limits::of("orchestration.subscribeThread", &bounded);
        assert_eq!(large, Limits { item: 64 << 20, queue: 72 << 20 });
        assert!(large.large() && !Limits::NORMAL.large());
        for (tag, payload) in [
            ("orchestration.subscribeThread", json!({"threadId":"t"})),
            ("orchestration.subscribeThread", json!({"acceptBoundedSnapshot":"true"})),
            ("orchestration.subscribeShell", json!({"acceptBoundedSnapshot":true})),
        ] {
            assert_eq!(Limits::of(tag, &payload), Limits::NORMAL, "{tag} {payload}");
        }
        // A large item fits once; the queue then still holds 8 MiB of events behind it.
        let shared = Shared::new(large, false);
        shared.push(json!("a".repeat(63 << 20))).unwrap();
        shared.push(json!("b".repeat(7 << 20))).unwrap();
        assert_eq!(shared.push(json!("c".repeat(3 << 20))).unwrap_err(), "stream_backpressure");
        assert_eq!(shared.inbox.lock().unwrap().items.len(), 2);
        let shared = Shared::new(large, false);
        assert_eq!(shared.push(json!("a".repeat(64 << 20))).unwrap_err(), "stream_backpressure");
    }
    fn subscription(shared: Shared) -> Subscription {
        Subscription { shared: Arc::new(shared), cancel: Arc::default(), wake: Arc::default() }
    }
    #[test]
    fn boundaries_survive_a_poll_in_the_middle_of_a_chunk() {
        let s = subscription(Shared::new(Limits::NORMAL, true));
        let q = &s.shared;
        q.push(json!({"kind":"snapshot"})).unwrap();
        q.push(json!({"kind":"synchronized"})).unwrap();
        q.chunk_end();
        q.chunk_end(); // an empty chunk adds nothing
        q.push(json!({"kind":"event","sequence":1})).unwrap();
        let first = s.poll(0).unwrap();
        assert_eq!(first["items"].as_array().unwrap().len(), 3);
        assert_eq!(first["boundaries"], json!([2]));
        assert_eq!((first["idle"].clone(), first["completionMarker"].clone()), (json!(false), json!(true)));
        // The rest of the open chunk, then its end, land in the next batch.
        q.push(json!({"kind":"event","sequence":2})).unwrap();
        q.chunk_end();
        q.idle();
        let second = s.poll(0).unwrap();
        assert_eq!(second["boundaries"], json!([1]));
        assert_eq!(second["idle"], true);
        assert_eq!(second["completionMarker"], true);
        // A chunk whose items were all polled before it ended reports offset 0.
        q.push(json!({"kind":"event","sequence":3})).unwrap();
        assert_eq!(s.poll(0).unwrap()["boundaries"], json!([]));
        q.chunk_end();
        let third = s.poll(1000).unwrap();
        assert_eq!((third["items"].clone(), third["boundaries"].clone()), (json!([]), json!([0])));
    }
    #[test]
    fn idle_wakes_a_poll_once_and_a_marker_needs_the_request_flag() {
        let s = Arc::new(subscription(Shared::new(Limits::NORMAL, false)));
        s.shared.push(json!({"kind":"synchronized"})).unwrap();
        s.shared.chunk_end();
        assert_eq!(s.poll(0).unwrap()["completionMarker"], false);
        let waiting = s.clone();
        let start = std::time::Instant::now();
        let worker = std::thread::spawn(move || waiting.poll(1000).unwrap());
        std::thread::sleep(Duration::from_millis(30));
        s.shared.idle();
        s.shared.idle();
        let woke = worker.join().unwrap();
        assert!(start.elapsed() < Duration::from_millis(900), "idle did not wake the poll");
        assert_eq!((woke["idle"].clone(), woke["items"].clone()), (json!(true), json!([])));
        // Still idle, but nothing new: the next poll waits its full timeout.
        let start = std::time::Instant::now();
        assert_eq!(s.poll(150).unwrap()["idle"], true);
        assert!(start.elapsed() >= Duration::from_millis(140));
    }
    #[test]
    fn a_terminal_error_keeps_its_code_and_carries_the_reason_apart() {
        let s = subscription(Shared::default());
        s.shared.push(json!({"kind":"event","sequence":1})).unwrap();
        s.shared.finish(Some("rpc_connection_failed: HTTP error: 401 Unauthorized".into()));
        let end = s.poll(0).unwrap();
        assert_eq!(end["items"], json!([{"kind":"event","sequence":1}]));
        assert_eq!((end["closed"].clone(), end["idle"].clone()), (json!(true), json!(false)));
        assert_eq!(end["error"], "rpc_connection_failed");
        assert_eq!(end["errorMessage"], "HTTP error: 401 Unauthorized");
        let s = subscription(Shared::default());
        s.shared.finish(Some("stream_timeout".into()));
        let end = s.poll(0).unwrap();
        assert_eq!((end["error"].clone(), end["errorMessage"].clone()), (json!("stream_timeout"), Value::Null));
    }
    #[test]
    fn poll_advances_the_cursor_from_after_sequence_and_keeps_the_start() {
        let payload = json!({"threadId":"t","afterSequence":41});
        let s = subscription(Shared::of("orchestration.subscribeThread", &payload));
        let q = &s.shared;
        let first = s.poll(0).unwrap();
        assert_eq!((first["cursor"].clone(), first["start"].clone()), (json!(41), Value::Null));
        q.push(json!({"kind":"event","sequence":42})).unwrap();
        q.push(json!({"kind":"synchronized"})).unwrap();
        let resumed = s.poll(0).unwrap();
        assert_eq!((resumed["cursor"].clone(), resumed["start"].clone()), (json!(42), json!("resume")));
        // A later snapshot sets the cursor but the start stays what the first item meant.
        q.push(json!({"kind":"snapshot","snapshotSequence":60})).unwrap();
        q.push(json!({"kind":"event","sequence":61})).unwrap();
        let replaced = s.poll(0).unwrap();
        assert_eq!((replaced["cursor"].clone(), replaced["start"].clone()), (json!(61), json!("resume")));
        // A tag the typed API does not know never invents a cursor.
        let s = subscription(Shared::of("somethingNew", &json!({})));
        s.shared.push(json!({"sequence":5})).unwrap();
        let raw = s.poll(0).unwrap();
        assert_eq!((raw["cursor"].clone(), raw["start"].clone()), (Value::Null, Value::Null));
    }
    #[test]
    fn cancel_wakes_a_waiter_without_a_network_tick() {
        let s = Arc::new(subscription(Shared::default()));
        let waiting = s.clone();
        let worker = std::thread::spawn(move || waiting.poll(1000).unwrap());
        s.cancel();
        let value = worker.join().unwrap();
        assert_eq!(value["closed"], true);
        assert!(value["items"].as_array().unwrap().is_empty());
    }
}
