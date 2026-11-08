//! Persistent host owners below the original Qt platform/controller. PCM never
//! implements Serialize or crosses the ordinary UI action/status channel.
mod stream;
use crate::{
    protocol::speech::{RequestCancellation, RequestResult, TranscriptionOptions},
    voice_capture::{Capture, Cleanup, Event, Mode, StartupCancellation, StartupFailure},
    voice_transport::BoundSpeech,
};
use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};
pub use stream::{PreparedStream, StreamEvent, StreamFeeder, StreamWorker};
use zeroize::{Zeroize, Zeroizing};
const MAX_PCM: usize = 19_200_000;
pub type Owner = Arc<dyn Fn() -> bool + Send + Sync>;
// One physical microphone owner per application. Unknown retirement is sticky
// until process restart, even if the original logical controller becomes idle.
static CAPTURE_STATE: AtomicU8 = AtomicU8::new(0);
struct CaptureReservation {
    startup: StartupCancellation,
    complete: bool,
}
impl CaptureReservation {
    fn claim(startup: StartupCancellation) -> io::Result<Self> {
        CAPTURE_STATE
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|state| {
                io::Error::other(if state == 2 {
                    "capture_cleanup_unknown"
                } else {
                    "capture_busy"
                })
            })?;
        Ok(Self {
            startup,
            complete: false,
        })
    }
    fn complete(&mut self, cleanup: Cleanup) {
        self.complete = true;
        CAPTURE_STATE.store(
            if cleanup == Cleanup::Confirmed { 0 } else { 2 },
            Ordering::Release,
        );
    }
}
impl Drop for CaptureReservation {
    fn drop(&mut self) {
        if !self.complete {
            CAPTURE_STATE.store(
                if self.startup.child_started() { 2 } else { 0 },
                Ordering::Release,
            );
        }
    }
}

/// A retained buffered recording, not a path or a caller-selected byte string.
/// Its original transport/options/owner remain fixed across explicit retries.
pub struct Recording {
    id: String,
    pcm: Zeroizing<Vec<u8>>,
    transport: Arc<BoundSpeech>,
    options: TranscriptionOptions,
    owner: Owner,
}
impl Recording {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub async fn transcribe(&self, cancel: &RequestCancellation) -> RequestResult<String> {
        self.transport
            .transcribe(&self.pcm, &self.options, cancel, || (self.owner)())
            .await
    }
}
/// Only small metadata and an opaque in-process recording ownership transfer.
/// The host maps Recording to its id, retaining the actual object privately.
pub enum RecorderEvent {
    Permission {
        request_id: String,
    },
    Prepared,
    Level(f64),
    DurationLimit,
    Recorded {
        id: u64,
    },
    Stopped {
        id: u64,
        recording: Option<Recording>,
        pcm_bytes: usize,
    },
    PermissionResult {
        id: u64,
        ok: bool,
    },
    Failed {
        id: Option<u64>,
        error: &'static str,
    },
    Closed(Cleanup),
    Microphones(Vec<crate::voice_capture::Microphone>),
}
enum Command {
    Record {
        id: u64,
        duration: Duration,
    },
    Stop {
        id: u64,
    },
    Permission {
        id: u64,
        request_id: String,
        allow: bool,
    },
}
enum Pending {
    Record(u64),
    Stop(u64),
    Permission(u64),
}
pub struct RecorderPlan {
    pub transport: Arc<BoundSpeech>,
    pub options: TranscriptionOptions,
    pub owner: Owner,
    pub mode: Mode,
    pub streaming: Option<StreamFeeder>,
}
/// Native application artifacts only. Mode comes from the original platform's
/// RecorderPlan; a separate closure cannot silently launch a different mode or
/// bypass the startup cancellation identity.
pub struct CaptureFactory {
    runtime: Arc<crate::voice_capture::resources::Runtime>,
    assets: Arc<crate::voice_capture::resources::Assets>,
    directory: PathBuf,
    display: Arc<crate::browser_native_display::Display>,
    microphone_id: String,
    synthetic: bool,
    profile: Option<Arc<crate::voice_capture::resources::Profile>>,
}
impl CaptureFactory {
    pub fn new(
        runtime: Arc<crate::voice_capture::resources::Runtime>,
        assets: Arc<crate::voice_capture::resources::Assets>,
        directory: PathBuf,
        display: Arc<crate::browser_native_display::Display>,
        microphone_id: String,
    ) -> Self {
        Self {
            runtime,
            assets,
            directory,
            display,
            microphone_id,
            synthetic: false,
            profile: None,
        }
    }
    /// Synthetic media is an explicit private-container fixture capability,
    /// never selected by wire parameters or environment preferences.
    pub fn with_synthetic_device(mut self) -> io::Result<Self> {
        if !std::path::Path::new("/run/.containerenv").exists() {
            return Err(io::Error::other("synthetic_capture_requires_container"));
        }
        self.synthetic = true;
        Ok(self)
    }
    pub(crate) fn with_profile(
        mut self,
        profile: Arc<crate::voice_capture::resources::Profile>,
    ) -> Self {
        self.profile = Some(profile);
        self
    }
    fn launch(self, mode: Mode, startup: &StartupCancellation) -> io::Result<Capture> {
        Capture::factory_cancellable(
            self.runtime,
            self.assets,
            &self.directory,
            self.display,
            crate::voice_capture::FactoryOptions {
                mode,
                microphone_id: &self.microphone_id,
                startup,
                synthetic: self.synthetic,
                profile: self.profile,
            },
        )
    }
}
/// The native factory owns all paths/programs and the one-shot cancellation.
pub struct RecorderWorker {
    commands: mpsc::SyncSender<Command>,
    startup: StartupCancellation,
    thread: Option<thread::JoinHandle<Cleanup>>,
    streaming: Option<StreamFeeder>,
}
impl RecorderWorker {
    pub fn spawn(
        plan: RecorderPlan,
        factory: CaptureFactory,
        startup: StartupCancellation,
        notify: impl Fn(RecorderEvent) + Send + Sync + 'static,
    ) -> io::Result<Self> {
        if startup.is_cancelled() {
            if let Some(stream) = &plan.streaming {
                stream.retire();
            }
            return Err(io::Error::other(StartupFailure {
                cleanup: Cleanup::Confirmed,
            }));
        }
        if !plan.transport.current(&|| (plan.owner)())
            || match (&plan.streaming, plan.mode) {
                (Some(stream), Mode::Streaming) => {
                    !stream.claim(&plan.transport, &plan.options, &plan.owner)
                }
                (None, Mode::Buffered) => false,
                _ => true,
            }
        {
            return Err(io::Error::other("voice_owner_unavailable"));
        }
        let (commands, receiver) = mpsc::sync_channel(8);
        let streaming = plan.streaming.clone();
        let reservation = match CaptureReservation::claim(startup.clone()) {
            Ok(reservation) => reservation,
            Err(error) => {
                if let Some(stream) = &streaming {
                    stream.retire();
                }
                return Err(error);
            }
        };
        let startup_thread = startup.clone();
        let worker = thread::Builder::new()
            .name("native-voice-capture".into())
            .spawn(move || {
                let mut reservation = reservation;
                let mut startup_retirement = stream::RetireOnDrop(plan.streaming.clone());
                let mut capture = match factory.launch(plan.mode, &startup_thread) {
                    Ok(capture) => capture,
                    Err(error) => {
                        if let Some(stream) = &plan.streaming {
                            stream.retire();
                        }
                        let cleanup = error
                            .get_ref()
                            .and_then(|error| error.downcast_ref::<StartupFailure>())
                            .map(|error| error.cleanup)
                            .unwrap_or(if startup_thread.child_started() {
                                Cleanup::Unknown
                            } else {
                                Cleanup::Confirmed
                            });
                        notify(RecorderEvent::Failed {
                            id: None,
                            error: "capture_startup_failed",
                        });
                        notify(RecorderEvent::Closed(cleanup));
                        reservation.complete(cleanup);
                        return cleanup;
                    }
                };
                // On unwind retire network before Capture::drop can wait for
                // microphone shutdown. Keep the pre-launch guard until transfer.
                let mut retirement = stream::RetireOnDrop(startup_retirement.0.take());
                let result = pump(&mut capture, &plan, &startup_thread, receiver, &notify);
                if !matches!(result, Ok(true)) {
                    if let Some(stream) = &plan.streaming {
                        stream.retire();
                    }
                }
                if let Err(error) = result {
                    notify(RecorderEvent::Failed { id: None, error });
                }
                let cleanup = capture.close();
                notify(RecorderEvent::Closed(cleanup));
                reservation.complete(cleanup);
                if matches!(result, Ok(true)) && cleanup == Cleanup::Confirmed {
                    retirement.0 = None;
                }
                cleanup
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(_) => {
                if let Some(stream) = &streaming {
                    stream.retire();
                }
                return Err(io::Error::other(StartupFailure {
                    cleanup: Cleanup::Confirmed,
                }));
            }
        };
        Ok(Self {
            commands,
            startup,
            thread: Some(worker),
            streaming,
        })
    }
    fn send(&self, command: Command) -> io::Result<()> {
        if self.startup.is_cancelled() {
            return Err(io::Error::other("capture_closed"));
        }
        self.commands
            .try_send(command)
            .map_err(|_| io::Error::other("capture_command_pending"))
    }
    pub fn record(&self, id: u64, duration: Duration) -> io::Result<()> {
        if duration.is_zero() || duration > Duration::from_secs(300) {
            return Err(io::Error::other("invalid_recording_duration"));
        }
        self.send(Command::Record { id, duration })
    }
    pub fn stop(&self, id: u64) -> io::Result<()> {
        self.send(Command::Stop { id })
    }
    pub fn permission(&self, id: u64, request_id: String, allow: bool) -> io::Result<()> {
        self.send(Command::Permission {
            id,
            request_id,
            allow,
        })
    }
    /// Urgent, independent of the ordinary queue or pending permission.
    pub fn abort(&self) {
        if let Some(stream) = &self.streaming {
            stream.retire();
        }
        self.startup.cancel();
    }
    /// Call away from the GUI/input thread. The physical capture receipt is
    /// separate from a transcript or the stream worker's terminal result.
    pub fn close(mut self) -> Cleanup {
        self.startup.cancel();
        self.thread
            .take()
            .and_then(|thread| thread.join().ok())
            .unwrap_or(if self.startup.child_started() {
                Cleanup::Unknown
            } else {
                Cleanup::Confirmed
            })
    }
}
impl Drop for RecorderWorker {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.abort();
        }
    }
}
fn pump(
    capture: &mut Capture,
    plan: &RecorderPlan,
    startup: &StartupCancellation,
    receiver: mpsc::Receiver<Command>,
    notify: &impl Fn(RecorderEvent),
) -> Result<bool, &'static str> {
    let mut pending = BTreeMap::new();
    let mut pcm = Zeroizing::new(Vec::new());
    let mut pcm_bytes = 0;
    let mut duration = None;
    let mut prepared = false;
    let mut check_consent = Instant::now();
    loop {
        if startup.is_cancelled() || !(plan.owner)() {
            return Ok(false);
        }
        if Instant::now() >= check_consent {
            if !plan.transport.current(&|| (plan.owner)()) {
                return Err("voice_consent_revoked");
            }
            check_consent = Instant::now() + Duration::from_millis(100);
        }
        match receiver.try_recv() {
            Ok(command) => {
                let id = match &command {
                    Command::Record { id, .. }
                    | Command::Stop { id }
                    | Command::Permission { id, .. } => *id,
                };
                if !plan.transport.current(&|| (plan.owner)()) {
                    return Err("voice_consent_revoked");
                }
                let operation = match command {
                    Command::Record {
                        id,
                        duration: limit,
                    } => capture.record().map(|capture_id| {
                        duration = Some(Instant::now() + limit);
                        (capture_id, Pending::Record(id))
                    }),
                    Command::Stop { id } => capture.stop().map(|capture_id| {
                        duration = None;
                        (capture_id, Pending::Stop(id))
                    }),
                    Command::Permission {
                        id,
                        request_id,
                        allow,
                    } => capture
                        .permission(&request_id, allow)
                        .map(|capture_id| (capture_id, Pending::Permission(id))),
                };
                match operation {
                    Ok((capture_id, operation)) => {
                        pending.insert(capture_id, operation);
                    }
                    Err(_) => notify(RecorderEvent::Failed {
                        id: Some(id),
                        error: "capture_operation_failed",
                    }),
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => return Ok(false),
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if duration.is_some_and(|deadline| Instant::now() >= deadline) {
            duration = None;
            notify(RecorderEvent::DurationLimit);
        }
        match capture
            .next(Duration::from_millis(25))
            .map_err(|_| "capture_failed")?
        {
            Some(Event::Permission { request_id }) => {
                notify(RecorderEvent::Permission { request_id })
            }
            Some(Event::Level(level)) => notify(RecorderEvent::Level(level)),
            Some(Event::DurationLimit) => notify(RecorderEvent::DurationLimit),
            Some(Event::CaptureError) => return Err("capture_failed"),
            Some(Event::Microphones(devices)) => notify(RecorderEvent::Microphones(devices)),
            Some(Event::Pcm(mut bytes)) => {
                if !plan.transport.current(&|| (plan.owner)()) {
                    bytes.zeroize();
                    return Err("voice_consent_revoked");
                }
                pcm_bytes += bytes.len();
                if let Some(stream) = &plan.streaming {
                    stream
                        .feed(Zeroizing::new(bytes))
                        .map_err(|_| "stream_failed")?;
                } else {
                    if bytes.len() > MAX_PCM - pcm.len() {
                        bytes.zeroize();
                        return Err("recording_too_large");
                    }
                    pcm.extend_from_slice(&bytes);
                    bytes.zeroize();
                }
            }
            Some(Event::Result { id, ok, error }) => {
                if id == 1 && !prepared {
                    if !ok {
                        return Err(if error.as_deref() == Some("capture_permission_denied") {
                            "capture_permission_denied"
                        } else {
                            "capture_preparation_failed"
                        });
                    }
                    prepared = true;
                    notify(RecorderEvent::Prepared);
                    continue;
                }
                let operation = pending.remove(&id).ok_or("capture_response_unavailable")?;
                let request = match &operation {
                    Pending::Record(id) | Pending::Stop(id) | Pending::Permission(id) => *id,
                };
                if !ok {
                    notify(RecorderEvent::Failed {
                        id: Some(request),
                        error: "capture_operation_failed",
                    });
                    continue;
                }
                match operation {
                    Pending::Record(id) => notify(RecorderEvent::Recorded { id }),
                    Pending::Permission(id) => {
                        notify(RecorderEvent::PermissionResult { id, ok: true })
                    }
                    Pending::Stop(id) => {
                        let recording = (!pcm.is_empty()).then(|| Recording {
                            id: format!("o3-voice-recording:{}", uuid::Uuid::new_v4()),
                            pcm: std::mem::replace(&mut pcm, Zeroizing::new(Vec::new())),
                            transport: plan.transport.clone(),
                            options: plan.options.clone(),
                            owner: plan.owner.clone(),
                        });
                        notify(RecorderEvent::Stopped {
                            id,
                            recording,
                            pcm_bytes,
                        });
                        return Ok(true);
                    }
                }
            }
            None => {}
        }
    }
}
