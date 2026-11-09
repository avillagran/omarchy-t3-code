//! The same native protocol over a caller-owned Tokio transport. No second
//! queue, retry, hidden runtime or credential-bearing renderer connection.
use super::*;
use crate::speech::{CancellableSpeech, RequestCancellation, RequestFailure};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type AsyncSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
/// One async stream, using the same native flow control as `SpeechStream`.
/// Cancelling its request token also closes an idle connected socket. Dropping
/// any entered operation retires the stream; it cannot be resumed or replayed.
pub struct AsyncSpeechStream {
    socket: AsyncSocket,
    protocol: Protocol,
    cancellation: SpeechCancellation,
    token: RequestCancellation,
    _registration: crate::speech::request::CancelRegistration,
    can_send: Arc<dyn Fn() -> bool + Send + Sync>,
    started: Instant,
}
/// One admitted media ticket, kept opaque and consumed by exactly one connect.
/// Hosts can drive connection and capture preparation concurrently, as in the
/// original browser platform, without exposing a URL or credential to the UI.
pub struct SpeechStreamOpening {
    url: reqwest::Url,
    options: TranscriptionOptions,
    token: RequestCancellation,
    can_send: Arc<dyn Fn() -> bool + Send + Sync>,
}
struct RetireOperation {
    cancellation: SpeechCancellation,
    armed: bool,
}
impl RetireOperation {
    fn new(cancellation: SpeechCancellation) -> Self {
        Self {
            cancellation,
            armed: true,
        }
    }
}
impl Drop for RetireOperation {
    fn drop(&mut self) {
        if self.armed {
            self.cancellation.cancel();
        }
    }
}
fn request_failure(failure: RequestFailure) -> LiveFailure {
    match failure {
        RequestFailure::Cancelled => LiveFailure::Cancelled,
        RequestFailure::ConsentRevoked => LiveFailure::ConsentRevoked,
        _ => LiveFailure::Transport,
    }
}
impl CancellableSpeech {
    /// Native media ticket, DNS/TCP/TLS/WebSocket opening and the response
    /// watchdog are cancellable on the caller's Tokio runtime. Audio is never
    /// sent until the host's current guard permits the original producer.
    pub async fn speech_stream(
        &self,
        options: &TranscriptionOptions,
        token: &RequestCancellation,
        can_send: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> LiveResult<AsyncSpeechStream> {
        self.prepare_speech_stream(options, token, can_send)
            .await?
            .connect()
            .await
    }
    pub async fn prepare_speech_stream(
        &self,
        options: &TranscriptionOptions,
        token: &RequestCancellation,
        can_send: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> LiveResult<SpeechStreamOpening> {
        let url = self
            .stream_url(token, &|| can_send())
            .await
            .map_err(request_failure)?;
        Ok(SpeechStreamOpening {
            url,
            options: options.clone(),
            token: token.clone(),
            can_send,
        })
    }
}
impl SpeechStreamOpening {
    pub async fn connect(self) -> LiveResult<AsyncSpeechStream> {
        let Self {
            url,
            options,
            token,
            can_send,
        } = self;
        crate::speech::request::admitted(&token, &|| can_send()).map_err(request_failure)?;
        let started = Instant::now();
        let config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(8 * 1024 * 1024))
            .max_frame_size(Some(8 * 1024 * 1024));
        let opening =
            tokio_tungstenite::connect_async_with_config(url.as_str(), Some(config), false);
        let (socket, _) = token
            .run(tokio::time::timeout(
                Duration::from_millis(WATCHDOG_MS),
                opening,
            ))
            .await
            .map_err(request_failure)?
            .map_err(|_| LiveFailure::Timeout)?
            .map_err(|_| LiveFailure::Transport)?;
        let tcp = match socket.get_ref() {
            MaybeTlsStream::Plain(tcp) => tcp,
            MaybeTlsStream::Rustls(tls) => tls.get_ref().0,
            _ => return Err(LiveFailure::Transport),
        };
        let duplicate = socket2::SockRef::from(tcp)
            .try_clone()
            .map_err(|_| LiveFailure::Transport)?;
        let cancellation = SpeechCancellation(Arc::new(Cancellation {
            cancelled: AtomicBool::new(false),
            socket: duplicate.into(),
        }));
        let interrupt = cancellation.clone();
        let registration = token.register(Arc::new(move || interrupt.cancel()));
        let mut stream = AsyncSpeechStream {
            socket,
            protocol: Protocol::new(&options, 0),
            cancellation,
            token: token.clone(),
            _registration: registration,
            can_send,
            started,
        };
        stream.guard()?;
        stream.protocol.opened((stream.can_send)());
        stream.flush().await?;
        Ok(stream)
    }
}
impl AsyncSpeechStream {
    pub fn cancellation(&self) -> SpeechCancellation {
        self.cancellation.clone()
    }
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    fn shutdown(&self) {
        let _ = self.cancellation.0.socket.shutdown(Shutdown::Both);
    }
    fn fail(&mut self, failure: LiveFailure) -> LiveFailure {
        self.protocol
            .fail(failure.clone(), !matches!(failure, LiveFailure::Cancelled));
        self.shutdown();
        failure
    }
    fn guard(&mut self) -> LiveResult<()> {
        if self.token.is_cancelled() || self.cancellation.0.cancelled.load(Ordering::Acquire) {
            return Err(self.fail(LiveFailure::Cancelled));
        }
        let allowed = (self.can_send)();
        if self.token.is_cancelled() || self.cancellation.0.cancelled.load(Ordering::Acquire) {
            return Err(self.fail(LiveFailure::Cancelled));
        }
        if !allowed {
            return Err(self.fail(LiveFailure::ConsentRevoked));
        }
        self.protocol.tick(self.now());
        if let Some(failure) = self.protocol.failure.clone() {
            self.shutdown();
            return Err(failure);
        }
        Ok(())
    }
    async fn flush(&mut self) -> LiveResult<()> {
        while let Some(frame) = self.protocol.outbound.pop_front() {
            if matches!(frame, Outbound::Close) {
                // Match the blocking adapter's bounded best-effort close,
                // followed by unconditional physical socket retirement.
                let _ =
                    tokio::time::timeout(Duration::from_millis(100), self.socket.close(None)).await;
                self.shutdown();
                continue;
            }
            self.guard()?;
            let message = match frame {
                Outbound::Text(text) => Message::Text(text.into()),
                Outbound::Audio(bytes) => Message::Binary(bytes.to_vec().into()),
                Outbound::Close => unreachable!(),
            };
            let remaining = self
                .protocol
                .deadline
                .map(|deadline| deadline.saturating_sub(self.now()))
                .unwrap_or(WATCHDOG_MS);
            let write = self
                .token
                .run(tokio::time::timeout(
                    Duration::from_millis(remaining),
                    self.socket.send(message),
                ))
                .await;
            match write {
                Err(_) => return Err(self.fail(LiveFailure::Cancelled)),
                Ok(Err(_)) => return Err(self.fail(LiveFailure::Timeout)),
                Ok(Ok(Err(_))) => {
                    let failure = if self.cancellation.0.cancelled.load(Ordering::Acquire) {
                        LiveFailure::Cancelled
                    } else {
                        LiveFailure::Transport
                    };
                    return Err(self.fail(failure));
                }
                Ok(Ok(Ok(()))) => {}
            }
        }
        self.protocol.failure.clone().map_or(Ok(()), Err)
    }
    /// Feed only the supplied view. Frames, the two-ack window and the 19.2 MB
    /// outstanding bound come from the same original protocol as the sync API.
    pub async fn feed(&mut self, pcm: &[f32]) -> LiveResult<()> {
        let mut retirement = RetireOperation::new(self.cancellation.clone());
        let result = async {
            self.guard()?;
            self.protocol.feed(pcm, self.now(), (self.can_send)());
            self.flush().await
        }
        .await;
        retirement.armed = false;
        result
    }
    pub async fn request_finish(&mut self) -> LiveResult<()> {
        let mut retirement = RetireOperation::new(self.cancellation.clone());
        let result = async {
            self.guard()?;
            self.protocol.finish(self.now(), (self.can_send)());
            self.flush().await
        }
        .await;
        retirement.armed = false;
        result
    }
    async fn poll_inner(&mut self) -> LiveResult<Option<LiveEvent>> {
        self.guard()?;
        if let Some(event) = self.protocol.events.pop_front() {
            return Ok(Some(event));
        }
        if self.protocol.closed {
            return Err(LiveFailure::Protocol);
        }
        let remaining = self
            .protocol
            .deadline
            .map(|deadline| deadline.saturating_sub(self.now()).min(100))
            .unwrap_or(100);
        let read = tokio::time::timeout(
            Duration::from_millis(remaining),
            self.token.run(self.socket.next()),
        )
        .await;
        match read {
            Err(_) => {
                self.guard()?;
                return Ok(None);
            }
            Ok(Err(_)) => return Err(self.fail(LiveFailure::Cancelled)),
            Ok(Ok(Some(Ok(Message::Text(text))))) => {
                self.guard()?;
                self.protocol.receive(&text, self.now(), (self.can_send)());
                self.flush().await?
            }
            Ok(Ok(Some(Ok(Message::Ping(_) | Message::Pong(_))))) => {
                let remaining = self
                    .protocol
                    .deadline
                    .map(|deadline| deadline.saturating_sub(self.now()))
                    .unwrap_or(WATCHDOG_MS);
                let write = self
                    .token
                    .run(tokio::time::timeout(
                        Duration::from_millis(remaining),
                        self.socket.flush(),
                    ))
                    .await;
                if !matches!(write, Ok(Ok(Ok(())))) {
                    let failure = if self.token.is_cancelled()
                        || self.cancellation.0.cancelled.load(Ordering::Acquire)
                    {
                        LiveFailure::Cancelled
                    } else if matches!(write, Ok(Err(_))) {
                        LiveFailure::Timeout
                    } else {
                        LiveFailure::Transport
                    };
                    return Err(self.fail(failure));
                }
            }
            Ok(Ok(Some(Ok(Message::Binary(_))))) => {
                return Err(self.fail(LiveFailure::InvalidResponse))
            }
            _ => {
                let failure = if self.cancellation.0.cancelled.load(Ordering::Acquire) {
                    LiveFailure::Cancelled
                } else {
                    LiveFailure::Transport
                };
                return Err(self.fail(failure));
            }
        }
        Ok(self.protocol.events.pop_front())
    }
    /// One owned read, bounded to 100 ms without data. Caller observation can
    /// alternate with capture without another reader or abandoning this future.
    pub async fn poll_event(&mut self) -> LiveResult<Option<LiveEvent>> {
        let mut retirement = RetireOperation::new(self.cancellation.clone());
        let result = self.poll_inner().await;
        retirement.armed = false;
        result
    }
    pub async fn next_event(&mut self) -> LiveResult<LiveEvent> {
        let mut retirement = RetireOperation::new(self.cancellation.clone());
        let result = async {
            loop {
                if let Some(event) = self.poll_inner().await? {
                    return Ok(event);
                }
            }
        }
        .await;
        retirement.armed = false;
        result
    }
    pub fn cancel(&mut self) {
        self.protocol.fail(LiveFailure::Cancelled, false);
        self.cancellation.cancel();
    }
}
impl Drop for AsyncSpeechStream {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}
