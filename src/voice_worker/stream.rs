//! One explicitly owned runtime drives one SDK stream. It never drops a pending
//! SDK read merely to receive another command; poll_event owns its 100ms poll.
use super::{Owner, MAX_PCM};
use crate::{
    protocol::speech::{
        LiveEvent, LiveFailure, RequestCancellation, SpeechStreamOpening, TranscriptionOptions,
    },
    voice_transport::BoundSpeech,
};
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    thread,
};
use zeroize::Zeroizing;
pub enum StreamEvent {
    Connected,
    Voice(LiveEvent),
    Failed(LiveFailure),
}
struct Queue {
    pcm: VecDeque<Zeroizing<Vec<u8>>>,
    total: usize,
    finish: bool,
    finish_taken: bool,
    closed: bool,
    claimed: bool,
}
struct Shared {
    queue: Mutex<Queue>,
    transport: Arc<BoundSpeech>,
    options: TranscriptionOptions,
    owner: Owner,
    cancel: RequestCancellation,
}
/// Only RecorderWorker can feed this capability. Its captured transport,
/// options and owner must be the exact same preparation as the stream.
#[derive(Clone)]
pub struct StreamFeeder(Arc<Shared>);
pub(super) struct RetireOnDrop(pub(super) Option<StreamFeeder>);
impl Drop for RetireOnDrop {
    fn drop(&mut self) {
        if let Some(stream) = &self.0 {
            stream.retire();
        }
    }
}
impl StreamFeeder {
    pub(super) fn claim(
        &self,
        transport: &Arc<BoundSpeech>,
        options: &TranscriptionOptions,
        owner: &Owner,
    ) -> bool {
        let matches = Arc::ptr_eq(transport, &self.0.transport)
            && Arc::ptr_eq(owner, &self.0.owner)
            && options == &self.0.options;
        if !matches {
            return false;
        }
        let Ok(mut queue) = self.0.queue.lock() else {
            return false;
        };
        if queue.claimed || queue.closed || self.0.cancel.is_cancelled() {
            return false;
        }
        queue.claimed = true;
        true
    }
    pub(super) fn feed(&self, bytes: Zeroizing<Vec<u8>>) -> Result<(), LiveFailure> {
        let mut queue = self.0.queue.lock().map_err(|_| LiveFailure::Transport)?;
        if queue.closed || queue.finish || self.0.cancel.is_cancelled() {
            return Err(LiveFailure::Cancelled);
        }
        if bytes.len() > MAX_PCM - queue.total {
            return Err(LiveFailure::QueueLimit);
        }
        // Coalesce adjacent tiny packets without waiting for a full chunk. This
        // bounds allocation count as well as bytes, with unchanged PCM order.
        for part in bytes.chunks(32000) {
            let mut offset = 0;
            if let Some(last) = queue.pcm.back_mut() {
                let count = (32000 - last.len()).min(part.len());
                last.extend_from_slice(&part[..count]);
                offset = count;
            }
            if offset < part.len() {
                queue.pcm.push_back(Zeroizing::new(part[offset..].to_vec()));
            }
        }
        queue.total += bytes.len();
        Ok(())
    }
    pub(super) fn finish(&self) -> Result<(), LiveFailure> {
        let mut queue = self.0.queue.lock().map_err(|_| LiveFailure::Transport)?;
        if queue.closed || queue.finish || self.0.cancel.is_cancelled() {
            return Err(LiveFailure::Cancelled);
        }
        queue.finish = true;
        Ok(())
    }
    pub(super) fn retire(&self) {
        self.0.cancel.cancel();
        if let Ok(mut queue) = self.0.queue.lock() {
            queue.closed = true;
            queue.pcm.clear();
        }
    }
}
/// Ticket acquisition is separate from connection. The original platform can
/// start connection and capture preparation in parallel without a UI ticket.
pub struct PreparedStream {
    opening: SpeechStreamOpening,
    feeder: StreamFeeder,
}
impl PreparedStream {
    pub async fn prepare(
        transport: Arc<BoundSpeech>,
        options: TranscriptionOptions,
        owner: Owner,
        cancel: RequestCancellation,
    ) -> Result<Self, LiveFailure> {
        let opening = transport
            .prepare_stream(&options, &cancel, owner.clone())
            .await?;
        Ok(Self {
            opening,
            feeder: StreamFeeder(Arc::new(Shared {
                queue: Mutex::new(Queue {
                    pcm: VecDeque::new(),
                    total: 0,
                    finish: false,
                    finish_taken: false,
                    closed: false,
                    claimed: false,
                }),
                transport,
                options,
                owner,
                cancel,
            })),
        })
    }
    pub fn connect(
        self,
        notify: impl Fn(StreamEvent) + Send + Sync + 'static,
    ) -> io::Result<StreamWorker> {
        let feeder = self.feeder;
        let worker_feeder = feeder.clone();
        let thread = thread::Builder::new()
            .name("native-voice-stream".into())
            .spawn(move || {
                let _retirement = RetireOnDrop(Some(worker_feeder.clone()));
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                let result = match runtime {
                    Ok(runtime) => {
                        let result = runtime.block_on(async {
                            let mut stream = self.opening.connect().await?;
                            notify(StreamEvent::Connected);
                            loop {
                                loop {
                                    let next = {
                                        let mut queue = worker_feeder
                                            .0
                                            .queue
                                            .lock()
                                            .map_err(|_| LiveFailure::Transport)?;
                                        if queue.closed {
                                            return Err(LiveFailure::Cancelled);
                                        }
                                        if let Some(pcm) = queue.pcm.pop_front() {
                                            Some(Ok(pcm))
                                        } else if queue.finish && !queue.finish_taken {
                                            queue.finish_taken = true;
                                            Some(Err(()))
                                        } else {
                                            None
                                        }
                                    };
                                    match next {
                                        Some(Ok(bytes)) => {
                                            if bytes.len() % 4 != 0 {
                                                return Err(LiveFailure::Protocol);
                                            }
                                            let samples = Zeroizing::new(
                                                bytes
                                                    .as_chunks::<4>()
                                                    .0
                                                    .iter()
                                                    .map(|bytes| f32::from_le_bytes(*bytes))
                                                    .collect::<Vec<_>>(),
                                            );
                                            stream.feed(&samples).await?;
                                        }
                                        Some(Err(())) => stream.request_finish().await?,
                                        None => break,
                                    }
                                }
                                if let Some(event) = stream.poll_event().await? {
                                    let finished = if let LiveEvent::Finished(text) = &event {
                                        Some(text.clone())
                                    } else {
                                        None
                                    };
                                    notify(StreamEvent::Voice(event));
                                    if let Some(text) = finished {
                                        return Ok(text);
                                    }
                                }
                            }
                        });
                        // A cancelled resolver may outlive its future in libc. It
                        // receives no credentials/PCM and cannot keep the owner join
                        // waiting after its actual stream has been retired.
                        runtime.shutdown_background();
                        result
                    }
                    Err(_) => Err(LiveFailure::Transport),
                };
                worker_feeder.retire();
                if let Err(error) = &result {
                    notify(StreamEvent::Failed(error.clone()));
                }
                result
            })?;
        Ok(StreamWorker {
            feeder,
            thread: Some(thread),
        })
    }
}
pub struct StreamWorker {
    feeder: StreamFeeder,
    thread: Option<thread::JoinHandle<Result<String, LiveFailure>>>,
}
impl StreamWorker {
    pub fn feeder(&self) -> StreamFeeder {
        self.feeder.clone()
    }
    pub fn cancel(&self) {
        self.feeder.retire();
    }
    /// Called by the original controller only after recorder.stop and its
    /// releaseAudioSession have settled. Capture stop never sends this frame.
    pub fn finish(&self) -> Result<(), LiveFailure> {
        self.feeder.finish()
    }
    pub fn join(mut self) -> Result<String, LiveFailure> {
        self.thread
            .take()
            .and_then(|thread| thread.join().ok())
            .unwrap_or(Err(LiveFailure::Transport))
    }
}
impl Drop for StreamWorker {
    fn drop(&mut self) {
        self.cancel();
    }
}
