//! One owned local voice socket. Mutations and audio are never replayed.
use super::*;
use std::{
    collections::VecDeque,
    net::Shutdown,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};
use tungstenite::{client::IntoClientRequest, Message};
use zeroize::Zeroizing;
mod asynchronous;
pub use asynchronous::{AsyncSpeechStream, SpeechStreamOpening};

const CHUNK: usize = SAMPLE_RATE * 4;
const WATCHDOG_MS: u64 = 120_000;
type Socket = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

#[derive(Clone, PartialEq, Eq)]
pub struct LiveText {
    pub committed: String,
    pub tentative: String,
}
impl std::fmt::Debug for LiveText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveText").finish_non_exhaustive()
    }
}
#[derive(Clone, PartialEq)]
pub enum LiveEvent {
    Ready,
    Update {
        revision: f64,
        text: Option<LiveText>,
    },
    Finished(String),
}
impl std::fmt::Debug for LiveEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Ready => "Ready",
            Self::Update { .. } => "Update { .. }",
            Self::Finished(_) => "Finished { .. }",
        })
    }
}
#[derive(Clone, PartialEq, Eq)]
pub enum LiveFailure {
    Cancelled,
    ConsentRevoked,
    Timeout,
    QueueLimit,
    Protocol,
    InvalidResponse,
    Remote(String),
    Transport,
}
impl std::fmt::Debug for LiveFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Cancelled",
            Self::ConsentRevoked => "ConsentRevoked",
            Self::Timeout => "Timeout",
            Self::QueueLimit => "QueueLimit",
            Self::Protocol => "Protocol",
            Self::InvalidResponse => "InvalidResponse",
            Self::Remote(_) => "Remote { .. }",
            Self::Transport => "Transport",
        })
    }
}
impl LiveFailure {
    pub fn message(&self) -> &str {
        match self {
            Self::Cancelled => "Voice transcription was cancelled.",
            Self::ConsentRevoked => "Voice consent was revoked.",
            Self::Timeout => "Live transcription stopped responding.",
            Self::QueueLimit => "Voice recordings are limited to five minutes.",
            Self::Protocol => "Unexpected speech stream response.",
            Self::InvalidResponse => "Invalid speech stream response.",
            Self::Remote(message) => message,
            Self::Transport => "The voice connection closed. Please record again.",
        }
    }
}
type LiveResult<T> = std::result::Result<T, LiveFailure>;
enum Outbound {
    Text(String),
    Audio(Zeroizing<Vec<u8>>),
    Close,
}
struct Protocol {
    options: String,
    queue: VecDeque<Zeroizing<Vec<u8>>>,
    queued: usize,
    flight: VecDeque<usize>,
    flight_bytes: usize,
    finishing: bool,
    finish_sent: bool,
    closed: bool,
    ready: bool,
    revision: f64,
    deadline: Option<u64>,
    outbound: VecDeque<Outbound>,
    events: VecDeque<LiveEvent>,
    failure: Option<LiveFailure>,
    finished: Option<String>,
    notify: bool,
}
impl Protocol {
    fn new(options: &TranscriptionOptions, now: u64) -> Self {
        Self {
            options: options.encode_json(),
            queue: VecDeque::new(),
            queued: 0,
            flight: VecDeque::new(),
            flight_bytes: 0,
            finishing: false,
            finish_sent: false,
            closed: false,
            ready: false,
            revision: -1.0,
            deadline: Some(now.saturating_add(WATCHDOG_MS)),
            outbound: VecDeque::new(),
            events: VecDeque::new(),
            failure: None,
            finished: None,
            notify: false,
        }
    }
    fn fail(&mut self, error: LiveFailure, notify: bool) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.deadline = None;
        self.queue.clear();
        self.queued = 0;
        self.failure = Some(error);
        self.notify = notify;
        self.outbound.clear();
        self.outbound.push_back(Outbound::Close);
    }
    fn consent(&mut self, allowed: bool) -> bool {
        if !allowed {
            self.fail(LiveFailure::ConsentRevoked, true);
            false
        } else {
            true
        }
    }
    fn opened(&mut self, allowed: bool) {
        if !self.consent(allowed) || self.closed {
            return;
        }
        self.outbound.push_back(Outbound::Text(format!(
            "{{\"type\":\"start\",\"options\":{}}}",
            self.options
        )));
    }
    fn pump(&mut self, now: u64, allowed: bool) {
        if !self.ready || self.closed || self.finish_sent || !self.consent(allowed) {
            return;
        }
        while self.flight.len() < 2 {
            let Some(chunk) = self.queue.pop_front() else {
                break;
            };
            self.flight.push_back(chunk.len());
            self.flight_bytes += chunk.len();
            self.queued -= chunk.len();
            self.outbound.push_back(Outbound::Audio(chunk));
            self.deadline = Some(now.saturating_add(WATCHDOG_MS));
        }
        if self.finishing && self.queue.is_empty() && self.flight.is_empty() {
            self.finish_sent = true;
            self.outbound
                .push_back(Outbound::Text("{\"type\":\"finish\"}".into()));
            self.deadline = Some(now.saturating_add(WATCHDOG_MS));
        }
    }
    fn feed(&mut self, pcm: &[f32], now: u64, allowed: bool) {
        if self.closed || self.finishing {
            return;
        }
        let Some(size) = pcm.len().checked_mul(4) else {
            self.fail(LiveFailure::QueueLimit, true);
            return;
        };
        if size > MAX_PCM_BYTES - self.queued - self.flight_bytes {
            self.fail(LiveFailure::QueueLimit, true);
            return;
        }
        for sample in pcm {
            if self.queue.back().is_none_or(|v| v.len() == CHUNK) {
                self.queue
                    .push_back(Zeroizing::new(Vec::with_capacity(CHUNK)));
            }
            self.queue
                .back_mut()
                .unwrap()
                .extend_from_slice(&sample.to_le_bytes());
            self.queued += 4;
        }
        self.pump(now, allowed);
    }
    fn finish(&mut self, now: u64, allowed: bool) {
        self.finishing = true;
        self.pump(now, allowed);
    }
    fn tick(&mut self, now: u64) {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.fail(LiveFailure::Timeout, true);
        }
    }
    fn receive(&mut self, text: &str, now: u64, allowed: bool) {
        if self.closed || !self.consent(allowed) {
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            self.fail(LiveFailure::InvalidResponse, true);
            return;
        };
        match v["type"].as_str() {
            Some("ready") if !self.ready => {
                self.ready = true;
                self.deadline = None;
                self.events.push_back(LiveEvent::Ready);
                self.pump(now, allowed);
            }
            Some("update") => {
                let revision = v["revision"].as_f64().filter(|r| {
                    r.is_finite() && r.fract() == 0.0 && r.abs() <= 9_007_199_254_740_991.0
                });
                let text = match v.get("text") {
                    Some(Value::Null) => Some(None),
                    Some(value) => value["committed"]
                        .as_str()
                        .zip(value["tentative"].as_str())
                        .map(|(committed, tentative)| {
                            Some(LiveText {
                                committed: committed.into(),
                                tentative: tentative.into(),
                            })
                        }),
                    None => None,
                };
                let (Some(revision), Some(text)) = (revision, text) else {
                    self.fail(LiveFailure::InvalidResponse, true);
                    return;
                };
                if self.flight.is_empty() || revision < self.revision {
                    self.fail(LiveFailure::Protocol, true);
                    return;
                }
                self.revision = revision;
                self.flight_bytes -= self.flight.pop_front().unwrap();
                self.deadline = None;
                self.events.push_back(LiveEvent::Update { revision, text });
                self.pump(now, allowed);
                if !self.flight.is_empty() {
                    self.deadline = Some(now.saturating_add(WATCHDOG_MS));
                }
            }
            Some("finished") if v["text"].is_string() => {
                if !self.finish_sent {
                    self.fail(LiveFailure::Protocol, true);
                    return;
                }
                let text = v["text"].as_str().unwrap().to_owned();
                self.closed = true;
                self.deadline = None;
                self.queue.clear();
                self.queued = 0;
                self.finished = Some(text.clone());
                self.events.push_back(LiveEvent::Finished(text));
                self.outbound.push_back(Outbound::Close);
            }
            Some("error") if v["message"].is_string() => self.fail(
                LiveFailure::Remote(v["message"].as_str().unwrap().into()),
                true,
            ),
            Some("ready") => self.fail(LiveFailure::Protocol, true),
            _ => self.fail(LiveFailure::InvalidResponse, true),
        }
    }
}

struct Cancellation {
    cancelled: AtomicBool,
    socket: std::net::TcpStream,
}
/// Independent cancellation can interrupt a blocked TLS read without replay.
#[derive(Clone)]
pub struct SpeechCancellation(Arc<Cancellation>);
impl SpeechCancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        let _ = self.0.socket.shutdown(Shutdown::Both);
    }
}
/// One local live transcription, with two acknowledged frames in flight.
/// The host explicitly supplies its current consent/authority gate. No microphone
/// is recorded here, and a failed stream is never replaced automatically.
pub struct SpeechStream {
    socket: Socket,
    protocol: Protocol,
    cancellation: SpeechCancellation,
    can_send: Arc<dyn Fn() -> bool + Send + Sync>,
    started: Instant,
}
impl Connection {
    pub fn speech_stream(
        &self,
        options: &TranscriptionOptions,
        can_send: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> LiveResult<SpeechStream> {
        if !can_send() {
            return Err(LiveFailure::ConsentRevoked);
        }
        let response = self
            .speech_request("POST", "/api/auth/websocket-ticket/media", None, false)
            .map_err(|_| LiveFailure::Transport)?;
        let ticket =
            crate::decode_with_limit(response, 64 * 1024).map_err(|_| LiveFailure::Transport)?;
        let ticket = Zeroizing::new(
            ticket["ticket"]
                .as_str()
                .filter(|v| !v.is_empty())
                .ok_or(LiveFailure::Protocol)?
                .to_owned(),
        );
        let mut url = endpoint(&self.base, "/ws/voice").map_err(|_| LiveFailure::Transport)?;
        url.set_scheme(if url.scheme() == "https" { "wss" } else { "ws" })
            .map_err(|_| LiveFailure::Transport)?;
        url.query_pairs_mut().append_pair("wsTicket", &ticket);
        let request = url
            .as_str()
            .into_client_request()
            .map_err(|_| LiveFailure::Transport)?;
        let host = url.host_str().ok_or(LiveFailure::Transport)?;
        let port = url.port_or_known_default().ok_or(LiveFailure::Transport)?;
        use std::net::ToSocketAddrs;
        let started = Instant::now();
        let until = started + Duration::from_millis(WATCHDOG_MS);
        let mut stream = None;
        for address in (host, port)
            .to_socket_addrs()
            .map_err(|_| LiveFailure::Transport)?
        {
            let Some(left) = until.checked_duration_since(Instant::now()) else {
                break;
            };
            if let Ok(socket) = std::net::TcpStream::connect_timeout(&address, left) {
                stream = Some(socket);
                break;
            }
        }
        let stream = stream.ok_or_else(|| {
            if Instant::now() >= until {
                LiveFailure::Timeout
            } else {
                LiveFailure::Transport
            }
        })?;
        let left = until
            .checked_duration_since(Instant::now())
            .ok_or(LiveFailure::Timeout)?;
        stream
            .set_read_timeout(Some(left))
            .map_err(|_| LiveFailure::Transport)?;
        stream
            .set_write_timeout(Some(left))
            .map_err(|_| LiveFailure::Transport)?;
        let cancellation = SpeechCancellation(Arc::new(Cancellation {
            cancelled: AtomicBool::new(false),
            socket: stream.try_clone().map_err(|_| LiveFailure::Transport)?,
        }));
        let config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(8 * 1024 * 1024))
            .max_frame_size(Some(8 * 1024 * 1024));
        let (socket, _) = tungstenite::client_tls_with_config(request, stream, Some(config), None)
            .map_err(|_| LiveFailure::Transport)?;
        cancellation
            .0
            .socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|_| LiveFailure::Transport)?;
        let mut protocol = Protocol::new(options, 0);
        protocol.opened(can_send());
        let mut result = SpeechStream {
            socket,
            protocol,
            cancellation,
            can_send,
            started,
        };
        result.flush()?;
        Ok(result)
    }
}
impl SpeechStream {
    pub fn cancellation(&self) -> SpeechCancellation {
        self.cancellation.clone()
    }
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    fn guard(&mut self) -> LiveResult<()> {
        if self.cancellation.0.cancelled.load(Ordering::Acquire) {
            self.protocol.fail(LiveFailure::Cancelled, false);
        } else if !(self.can_send)() {
            self.protocol.fail(LiveFailure::ConsentRevoked, true);
        }
        self.protocol.tick(self.now());
        if let Some(error) = self.protocol.failure.clone() {
            let _ = self.flush();
            return Err(error);
        }
        Ok(())
    }
    fn flush(&mut self) -> LiveResult<()> {
        while let Some(frame) = self.protocol.outbound.pop_front() {
            if !matches!(&frame, Outbound::Close) {
                if self.cancellation.0.cancelled.load(Ordering::Acquire) {
                    self.protocol.fail(LiveFailure::Cancelled, false);
                    continue;
                }
                if !(self.can_send)() {
                    self.protocol.fail(LiveFailure::ConsentRevoked, true);
                    continue;
                }
            }
            let result = match frame {
                Outbound::Text(text) => self.socket.send(Message::Text(text.into())),
                Outbound::Audio(bytes) => self.socket.send(Message::Binary(bytes.to_vec().into())),
                Outbound::Close => {
                    let _ = self
                        .cancellation
                        .0
                        .socket
                        .set_write_timeout(Some(Duration::from_millis(100)));
                    let _ = self.socket.close(None);
                    let _ = self.cancellation.0.socket.shutdown(Shutdown::Both);
                    Ok(())
                }
            };
            if result.is_err() {
                self.protocol.fail(LiveFailure::Transport, true);
                let _ = self.cancellation.0.socket.shutdown(Shutdown::Both);
                return Err(LiveFailure::Transport);
            }
        }
        self.protocol.failure.clone().map_or(Ok(()), Err)
    }
    /// Copies the supplied samples, splitting/coalescing into at most one second
    /// per frame. The native queue bound counts queued plus unacknowledged bytes.
    pub fn feed(&mut self, pcm: &[f32]) -> LiveResult<()> {
        self.guard()?;
        self.protocol.feed(pcm, self.now(), (self.can_send)());
        self.flush()
    }
    pub fn request_finish(&mut self) -> LiveResult<()> {
        self.guard()?;
        self.protocol.finish(self.now(), (self.can_send)());
        self.flush()
    }
    /// Services one bounded physical read (at most 100 ms without data). A host
    /// actor can alternate this with capture input without another socket reader.
    pub fn poll_event(&mut self) -> LiveResult<Option<LiveEvent>> {
        self.guard()?;
        if let Some(event) = self.protocol.events.pop_front() {
            return Ok(Some(event));
        }
        if self.protocol.closed {
            return Err(LiveFailure::Protocol);
        }
        match self.socket.read() {
            Ok(Message::Text(text)) => {
                self.protocol.receive(&text, self.now(), (self.can_send)());
                self.flush()?;
            }
            Ok(Message::Ping(_) | Message::Pong(_)) => {
                if self.socket.flush().is_err() {
                    self.protocol.fail(LiveFailure::Transport, true);
                    let _ = self.flush();
                    return Err(LiveFailure::Transport);
                }
            }
            Ok(Message::Binary(_)) => {
                self.protocol.fail(LiveFailure::InvalidResponse, true);
                return self.flush().and(Err(LiveFailure::InvalidResponse));
            }
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            _ => {
                if self.cancellation.0.cancelled.load(Ordering::Acquire) {
                    self.protocol.fail(LiveFailure::Cancelled, false);
                } else {
                    self.protocol.fail(LiveFailure::Transport, true);
                }
                let _ = self.flush();
                return Err(self.protocol.failure.clone().unwrap());
            }
        }
        Ok(self.protocol.events.pop_front())
    }
    /// Blocks while servicing the native response watchdog and current host gate.
    /// No text callback is invoked from another thread by this library.
    pub fn next_event(&mut self) -> LiveResult<LiveEvent> {
        loop {
            if let Some(event) = self.poll_event()? {
                return Ok(event);
            }
        }
    }
    pub fn cancel(&mut self) {
        self.protocol.fail(LiveFailure::Cancelled, false);
        let _ = self.flush();
        self.cancellation.cancel();
    }
}
impl Drop for SpeechStream {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn normalized(error: &LiveFailure) -> &str {
        if *error == LiveFailure::InvalidResponse {
            "invalid_response"
        } else {
            error.message()
        }
    }
    #[test]
    fn original_complete_client_producer_matches_flow_control_and_lifetime() {
        assert!(std::env::var_os("ANDROID_SANDBOX").is_some());
        let fixture: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/speech-stream-4314.json"))
                .unwrap();
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 66);
        let options = TranscriptionOptions::decode(&fixture["options"]).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut protocol = Protocol::new(&options, 0);
            let mut now = 0;
            let mut allowed = true;
            let mut observing_final = false;
            let mut frames = vec![];
            let mut texts = vec![];
            let mut errors = vec![];
            let mut error_observed = false;
            for (step, expected) in case["steps"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["output"].as_array().unwrap())
            {
                match step["op"].as_str().unwrap() {
                    "open" => protocol.opened(allowed),
                    "feed" => {
                        let n = step["count"].as_u64().unwrap() as usize;
                        let value = if step["pattern"] == "nan" {
                            f32::NAN
                        } else {
                            step["value"].as_f64().unwrap() as f32
                        };
                        let mut backing = vec![99.0; n + 11];
                        backing[7..7 + n].fill(value);
                        protocol.feed(&backing[7..7 + n], now, allowed);
                    }
                    "message" => protocol.receive(&step["value"].to_string(), now, allowed),
                    "raw" => protocol.receive(step["value"].as_str().unwrap(), now, allowed),
                    "finish" => {
                        observing_final = true;
                        protocol.finish(now, allowed);
                    }
                    "cancel" => protocol.fail(LiveFailure::Cancelled, false),
                    "consent" => allowed = step["value"].as_bool().unwrap(),
                    "socket-close" => protocol.fail(LiveFailure::Transport, true),
                    "socket-error" => protocol.fail(
                        LiveFailure::Remote("Could not connect to live transcription.".into()),
                        true,
                    ),
                    "tick" => {
                        now += step["ms"].as_u64().unwrap();
                        protocol.tick(now);
                    }
                    _ => panic!("Unknown source operation"),
                }
                while let Some(frame) = protocol.outbound.pop_front() {
                    frames.push(match frame {
                    Outbound::Text(text)=>json!({"kind":"text","value":serde_json::from_str::<Value>(&text).unwrap()}),
                    Outbound::Audio(bytes)=>json!({"kind":"audio","bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&*bytes))}),
                    Outbound::Close=>json!({"kind":"close"}),
                });
                }
                while let Some(event) = protocol.events.pop_front() {
                    if let LiveEvent::Update {
                        text: Some(text), ..
                    } = event
                    {
                        texts.push(json!({"committed":text.committed,"tentative":text.tentative}));
                    }
                }
                if let Some(error) = &protocol.failure {
                    if protocol.notify && !error_observed {
                        errors.push(normalized(error).to_owned());
                        error_observed = true;
                    }
                }
                let final_value = if observing_final {
                    if let Some(error) = &protocol.failure {
                        json!({"ok":false,"error":normalized(error)})
                    } else if let Some(text) = &protocol.finished {
                        json!({"ok":true,"text":text})
                    } else {
                        Value::Null
                    }
                } else {
                    Value::Null
                };
                let timers = protocol
                    .deadline
                    .map(|d| vec![d.saturating_sub(now)])
                    .unwrap_or_default();
                assert_eq!(
                    json!({"frames":frames,"texts":texts,"errors":errors,"final":final_value,"timers":timers}),
                    *expected,
                    "{} {step}",
                    case["name"]
                );
            }
        }
    }
    #[test]
    fn original_pcm_parser_boundaries_preserve_only_the_supplied_view() {
        assert!(std::env::var_os("ANDROID_SANDBOX").is_some());
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/speech-pcm-boundaries-4314.json"
        ))
        .unwrap();
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 18);
        fn bytes(hex: &str) -> Vec<u8> {
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect()
        }
        for case in fixture["cases"].as_array().unwrap() {
            let body = bytes(case["bodyHex"].as_str().unwrap());
            let mut backing = vec![255; body.len() + 11];
            backing[7..7 + body.len()].copy_from_slice(&body);
            let result = decode_pcm_request(&backing[7..7 + body.len()]);
            assert_eq!(
                result.is_ok(),
                case["allowed"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
            if let Ok((options, pcm)) = result {
                assert_eq!(
                    serde_json::from_str::<Value>(&options.encode_json()).unwrap(),
                    case["options"]
                );
                assert_eq!(pcm, bytes(case["pcmHex"].as_str().unwrap()));
            }
        }
    }
}
