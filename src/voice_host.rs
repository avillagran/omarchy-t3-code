//! Private watch-process bridge for the original Qt platform/controller. Wire
//! actions contain metadata and opaque handles, never audio or executable paths.
use crate::{
    browser_native_display::Display,
    json,
    protocol::speech::{
        LiveEvent, LiveFailure, PostProcessRequest, RequestCancellation, RequestFailure,
        TranscriptionOptions,
    },
    text,
    voice_capture::{
        resources::{Assets, DescriptorArtifact, Runtime},
        Cleanup, Mode, StartupCancellation,
    },
    voice_consent::{CloudAcknowledgement, ConsentChange, ConsentOwner, Snapshot},
    voice_transport::{BoundCleanup, BoundSpeech},
    voice_worker::{
        CaptureFactory, Owner, PreparedStream, RecorderEvent, RecorderPlan, RecorderWorker,
        Recording, StreamEvent, StreamWorker,
    },
    Environment, Value,
};
use std::{
    collections::HashMap,
    fs, io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Arc, Condvar, Mutex,
    },
    thread,
};
type Emit = Arc<dyn Fn(Value) + Send + Sync>;
type Resolve = Arc<dyn Fn(&str) -> crate::Result<Environment> + Send + Sync>;
fn unavailable() -> String {
    "voice_host_unavailable".into()
}
type DisplayRequest = SyncSender<io::Result<Arc<Display>>>;
struct DisplayOwner {
    requests: Option<SyncSender<DisplayRequest>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl DisplayOwner {
    fn new() -> io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<DisplayRequest>(1);
        let thread = thread::Builder::new()
            .name("native-voice-display".into())
            .spawn(move || {
                let mut display: Option<Arc<Display>> = None;
                let mut watch: Option<OwnedFd> = None;
                while let Ok(reply) = receiver.recv() {
                    let result = (|| {
                        if display.is_none() {
                            let owned = Arc::new(
                                Display::launch(Path::new("/usr/bin/Xvfb"))
                                    .map_err(io::Error::other)?,
                            );
                            let raw = unsafe {
                                libc::syscall(libc::SYS_pidfd_open, owned.server_pid(), 0)
                            };
                            if raw < 0 {
                                return Err(io::Error::other("voice_display_unavailable"));
                            }
                            watch = Some(unsafe { OwnedFd::from_raw_fd(raw as i32) });
                            display = Some(owned);
                        }
                        Ok(display.as_ref().unwrap().clone())
                    })();
                    let _ = reply.try_send(result);
                }
                // PDEATHSIG follows the creating Linux thread. Release our lease,
                // then keep that thread alive until the actual display has exited,
                // including capture leases still finishing after owner cancellation.
                drop(display);
                if let Some(watch) = watch {
                    let mut descriptor = libc::pollfd {
                        fd: watch.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    loop {
                        if unsafe { libc::poll(&mut descriptor, 1, -1) } >= 0 {
                            break;
                        }
                        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                            break;
                        }
                    }
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            thread: Some(thread),
        })
    }
    fn acquire(&self) -> io::Result<Arc<Display>> {
        let (reply, result) = mpsc::sync_channel(1);
        self.requests
            .as_ref()
            .ok_or_else(|| io::Error::other("voice_display_unavailable"))?
            .try_send(reply)
            .map_err(|_| io::Error::other("voice_display_unavailable"))?;
        result
            .recv()
            .map_err(|_| io::Error::other("voice_display_unavailable"))?
    }
}
impl Drop for DisplayOwner {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn private_directory(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::other("voice_directory_unavailable"));
    }
    let mut current = PathBuf::from("/");
    for part in path.components().skip(1) {
        let std::path::Component::Normal(part) = part else {
            return Err(io::Error::other("voice_directory_unavailable"));
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Err(io::Error::other("voice_directory_unavailable")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::DirBuilder::new().mode(0o700).create(&current)?
            }
            Err(error) => return Err(error),
        }
    }
    let metadata = fs::symlink_metadata(path)?;
    if path.canonicalize()? != path
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::Error::other("voice_directory_unavailable"));
    }
    Ok(())
}
/// Constructed at native application startup. No action can choose its files,
/// display, storage namespace or synthetic media mode.
pub struct Resources {
    runtime: Arc<Runtime>,
    assets: Arc<Assets>,
    consent: ConsentOwner,
    root: PathBuf,
    display: Mutex<Option<DisplayOwner>>,
    synthetic: bool,
}
impl Resources {
    pub fn new(
        runtime: Arc<Runtime>,
        descriptor: &DescriptorArtifact,
        root: &Path,
        namespace: &Path,
    ) -> io::Result<Self> {
        private_directory(root)?;
        private_directory(namespace)?;
        let process = root.join(format!("host-{}", uuid::Uuid::new_v4()));
        private_directory(&process)?;
        let assets = Arc::new(Assets::capture(
            &process.join("capture-assets"),
            descriptor,
        )?);
        let consent_assets = Arc::new(Assets::consent(
            &process.join("consent-assets"),
            descriptor,
        )?);
        let consent = ConsentOwner::launch(
            runtime.clone(),
            consent_assets,
            &process.join("consent"),
            namespace,
        )?;
        Ok(Self {
            runtime,
            assets,
            consent,
            root: process,
            display: Mutex::new(None),
            synthetic: false,
        })
    }
    pub fn with_synthetic_device(mut self) -> io::Result<Self> {
        if !Path::new("/run/.containerenv").exists() {
            return Err(io::Error::other("synthetic_capture_requires_container"));
        }
        self.synthetic = true;
        Ok(self)
    }
    /// Explicit trusted startup configuration, separate from browser selection.
    /// Missing resources leave the ordinary application usable.
    pub fn configured() -> io::Result<Option<Self>> {
        let Some(runtime) = std::env::var_os("O3_NATIVE_VOICE_RUNTIME") else {
            return Ok(None);
        };
        let addon = std::env::var_os("O3_NATIVE_VOICE_ADDON")
            .ok_or_else(|| io::Error::other("voice_addon_unavailable"))?;
        let sha256 = std::env::var("O3_NATIVE_VOICE_ADDON_SHA256")
            .map_err(|_| io::Error::other("voice_addon_unavailable"))?;
        let bytes = std::env::var("O3_NATIVE_VOICE_ADDON_BYTES")
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("voice_addon_unavailable"))?;
        let runtime = Arc::new(Runtime::admit(Path::new(&runtime))?);
        let descriptor = DescriptorArtifact {
            path: PathBuf::from(addon),
            sha256,
            bytes,
        };
        Self::new(
            runtime,
            &descriptor,
            &crate::xdg("XDG_CACHE_HOME", ".cache").join("native-voice"),
            &crate::xdg("XDG_STATE_HOME", ".local/state").join("voice-consent"),
        )
        .map(Some)
    }
    fn factory(&self, microphone: String) -> io::Result<CaptureFactory> {
        let display = {
            let mut slot = self
                .display
                .lock()
                .map_err(|_| io::Error::other("voice_display_unavailable"))?;
            if slot.is_none() {
                *slot = Some(DisplayOwner::new()?);
            }
            slot.as_ref().unwrap().acquire()?
        };
        let factory = CaptureFactory::new(
            self.runtime.clone(),
            self.assets.clone(),
            self.root
                .join(format!("recording-{}", uuid::Uuid::new_v4())),
            display,
            microphone,
        );
        if self.synthetic {
            factory.with_synthetic_device()
        } else {
            Ok(factory)
        }
    }
}
#[derive(Clone)]
struct Request {
    generation: u64,
    cancel: RequestCancellation,
    session: String,
}
#[derive(Clone)]
struct Ticket {
    id: String,
    generation: u64,
}
struct Options {
    value: TranscriptionOptions,
    cleanup: Option<Arc<BoundCleanup>>,
    cleanup_failed: bool,
}
struct Session {
    owner_key: String,
    draft_environment: String,
    owner_epoch: u64,
    active: Arc<AtomicBool>,
    owner: Owner,
    transport: Arc<BoundSpeech>,
    options: Mutex<Option<Options>>,
    opening: Mutex<Option<PreparedStream>>,
    stream: Mutex<Option<StreamWorker>>,
    recorder: Mutex<Option<RecorderWorker>>,
    startup: StartupCancellation,
    network: RequestCancellation,
    recordings: Mutex<HashMap<String, Arc<Recording>>>,
    capture_pending: Mutex<HashMap<u64, Ticket>>,
    prepare_request: Mutex<Option<Ticket>>,
    closed: AtomicBool,
    gate: Mutex<()>,
    prepare_used: AtomicBool,
    ticket_used: AtomicBool,
    connect_used: AtomicBool,
    released: AtomicBool,
    cleanup_unknown: AtomicBool,
    stopped: AtomicBool,
}
impl Session {
    fn abort(&self) {
        self.network.cancel();
        self.startup.cancel();
        if let Ok(stream) = self.stream.lock() {
            if let Some(stream) = stream.as_ref() {
                stream.cancel();
            }
        }
    }
    fn invalidate(&self) {
        self.active.store(false, Ordering::Release);
        self.abort();
    }
}
struct State {
    resources: Resources,
    resolve: Resolve,
    emit: Emit,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    owner: Mutex<(String, String, u64)>,
    requests: Mutex<HashMap<String, Request>>,
    acknowledgements: Mutex<HashMap<String, CloudAcknowledgement>>,
    sequence: AtomicU64,
    shutdown: AtomicBool,
    consent_alive: AtomicBool,
    owner_revision: AtomicU64,
    actions: Mutex<usize>,
    actions_done: Condvar,
    consent_listener: Mutex<Option<thread::JoinHandle<()>>>,
}
struct ActionGuard(Arc<State>);
impl Drop for State {
    fn drop(&mut self) {
        // No callback holds this owner alive. Retire transports before resource
        // fields can wait for metadata helper or microphone process cleanup.
        if let Ok(sessions) = self.sessions.get_mut() {
            for session in sessions.values() {
                session.invalidate();
            }
        }
    }
}
impl Drop for ActionGuard {
    fn drop(&mut self) {
        if let Ok(mut count) = self.0.actions.lock() {
            *count -= 1;
            self.0.actions_done.notify_all();
        }
    }
}
#[derive(Clone)]
pub struct Host(Arc<State>);
impl Host {
    pub fn new(
        resources: Resources,
        resolve: impl Fn(&str) -> crate::Result<Environment> + Send + Sync + 'static,
        emit: impl Fn(Value) + Send + Sync + 'static,
    ) -> Self {
        let changes = resources.consent.subscribe_changes();
        let host = Self(Arc::new(State {
            resources,
            resolve: Arc::new(resolve),
            emit: Arc::new(emit),
            sessions: Mutex::new(HashMap::new()),
            owner: Mutex::new((String::new(), String::new(), 0)),
            requests: Mutex::new(HashMap::new()),
            acknowledgements: Mutex::new(HashMap::new()),
            sequence: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
            consent_alive: AtomicBool::new(true),
            owner_revision: AtomicU64::new(0),
            actions: Mutex::new(0),
            actions_done: Condvar::new(),
            consent_listener: Mutex::new(None),
        }));
        if let Ok(changes) = changes {
            let weak = Arc::downgrade(&host.0);
            match thread::Builder::new().name("native-voice-consent-events".into()).spawn(move||{
                loop {
                    let event=changes.recv();
                    let Some(state)=weak.upgrade()else{break;};
                    let host=Host(state);
                    match event {
                        Ok(ConsentChange::Changed{generation,snapshot})=>{
                            let sessions=host.0.sessions.lock().map(|sessions|sessions.values().cloned().collect::<Vec<_>>()).unwrap_or_default();
                            // Check the current original producer, not a historical
                            // event's JSON, so queued events cannot revoke a newer
                            // valid binding merely because they arrived late.
                            for session in sessions {if session.active.load(Ordering::Acquire)&&!session.transport.current(&||session.active.load(Ordering::Acquire)){session.invalidate();}}
                            if !host.0.shutdown.load(Ordering::Acquire){(host.0.emit)(json!({"kind":"consent","generation":generation,"value":snapshot_value(snapshot)}));}
                        }
                        Ok(ConsentChange::Closed)|Err(_)=>{
                            host.0.consent_alive.store(false,Ordering::Release);host.invalidate_all();
                            if !host.0.shutdown.load(Ordering::Acquire){(host.0.emit)(json!({"kind":"ready","available":false}));}
                            break;
                        }
                    }
                }
            }) {
                Ok(listener)=>{if let Ok(mut slot)=host.0.consent_listener.lock(){*slot=Some(listener);}},
                Err(_)=>{host.0.shutdown.store(true,Ordering::Release);let _=host.0.resources.consent.close();}
            }
        } else {
            host.0.shutdown.store(true, Ordering::Release);
            let _ = host.0.resources.consent.close();
        }
        host
    }
    pub fn ready(&self) {
        (self.0.emit)(
            json!({"kind":"ready","available":self.0.consent_alive.load(Ordering::Acquire)&&!self.0.shutdown.load(Ordering::Acquire)}),
        );
    }
    pub fn owner_changed(&self, key: &str, environment: &str, epoch: u64) {
        let Ok(mut owner) = self.0.owner.lock() else {
            return;
        };
        if epoch < owner.2 {
            return;
        }
        if key == owner.0 && environment == owner.1 && epoch == owner.2 {
            return;
        }
        *owner = (key.into(), environment.into(), epoch);
        self.0.owner_revision.fetch_add(1, Ordering::AcqRel);
        if let Ok(sessions) = self.0.sessions.lock() {
            for session in sessions.values() {
                if session.owner_key != key
                    || session.draft_environment != environment
                    || session.owner_epoch != epoch
                {
                    session.invalidate();
                }
            }
        }
    }
    fn result(&self, request: &Ticket, session: &str, result: Result<Value, String>) {
        let emit = if let Ok(mut requests) = self.0.requests.lock() {
            if requests
                .get(&request.id)
                .is_some_and(|entry| entry.generation == request.generation)
            {
                requests.remove(&request.id);
                true
            } else {
                false
            }
        } else {
            false
        };
        if !emit {
            return;
        }
        (self.0.emit)(
            json!({"kind":"result","requestId":request.id,"session":session,"value":result.as_ref().ok(),"error":result.as_ref().err()}),
        );
    }
    fn session(&self, id: &str, cleanup: bool) -> Result<Arc<Session>, String> {
        self.0
            .sessions
            .lock()
            .map_err(|_| unavailable())?
            .get(id)
            .cloned()
            .filter(|session| cleanup || session.active.load(Ordering::Acquire))
            .ok_or_else(|| "voice_owner_changed".into())
    }
    pub fn request(&self, input: &Value) {
        let action = text(input, "action");
        if self.0.shutdown.load(Ordering::Acquire) {
            return;
        }
        if action == "owner" {
            if let Some(epoch) = input["epoch"].as_u64() {
                self.owner_changed(
                    text(input, "ownerKey"),
                    text(input, "draftEnvironment"),
                    epoch,
                );
            }
            return;
        }
        if action == "abort-request" {
            let request = self
                .0
                .requests
                .lock()
                .ok()
                .and_then(|requests| requests.get(text(input, "target")).cloned());
            if let Some(request) = request {
                request.cancel.cancel();
                if let Ok(session) = self.session(&request.session, true) {
                    session.abort();
                }
            }
            return;
        }
        if action == "abort" {
            if let Ok(requests) = self.0.requests.lock() {
                for request in requests.values() {
                    if request.session == text(input, "session") {
                        request.cancel.cancel();
                    }
                }
            }
            if let Ok(session) = self.session(text(input, "session"), true) {
                session.abort();
            }
            return;
        }
        let request = text(input, "requestId");
        if request.is_empty() || request.len() > 128 || request.chars().any(char::is_control) {
            return;
        }
        let cancel = RequestCancellation::default();
        let ticket = Ticket {
            id: request.into(),
            generation: self.0.sequence.fetch_add(1, Ordering::AcqRel),
        };
        if let Ok(mut requests) = self.0.requests.lock() {
            if requests.len() >= 32 || requests.contains_key(request) {
                (self.0.emit)(
                    json!({"kind":"result","requestId":request,"session":text(input,"session"),"error":"voice_operation_busy"}),
                );
                return;
            }
            requests.insert(
                request.into(),
                Request {
                    generation: ticket.generation,
                    cancel: cancel.clone(),
                    session: text(input, "session").into(),
                },
            );
        } else {
            return;
        }
        {
            let Ok(mut count) = self.0.actions.lock() else {
                self.result(&ticket, text(input, "session"), Err(unavailable()));
                return;
            };
            if self.0.shutdown.load(Ordering::Acquire) {
                drop(count);
                self.result(
                    &ticket,
                    text(input, "session"),
                    Err("voice_cancelled".into()),
                );
                return;
            }
            *count += 1;
        }
        let action_guard = ActionGuard(self.0.clone());
        let host = self.clone();
        let input = input.clone();
        let failed = ticket.clone();
        let failure_session = text(&input, "session").to_owned();
        let spawned = thread::Builder::new()
            .name("native-voice-action".into())
            .spawn(move || {
                let _action_guard = action_guard;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    host.execute(&input, &cancel, &ticket)
                }));
                match result {
                    Ok(Some(result)) => {
                        host.forget_capture(&ticket);
                        host.result(&ticket, text(&input, "session"), result)
                    }
                    Ok(None) => {}
                    Err(_) => {
                        if let Ok(session) = host.session(text(&input, "session"), true) {
                            session.invalidate();
                        }
                        host.forget_capture(&ticket);
                        host.result(&ticket, text(&input, "session"), Err(unavailable()))
                    }
                }
            });
        if spawned.is_err() {
            self.result(&failed, &failure_session, Err(unavailable()));
        }
    }
    fn execute(
        &self,
        input: &Value,
        cancel: &RequestCancellation,
        ticket: &Ticket,
    ) -> Option<Result<Value, String>> {
        let action = text(input, "action");
        let session_id = text(input, "session");
        Some((||{
            if cancel.is_cancelled(){return Err("voice_cancelled".into());}
            if action=="snapshot"{return self.snapshot();}
            if action=="update-preferences"{return crate::voice_preferences::update(&self.0.resources.consent,text(input,"revision"),&input["patch"]);}
            if action=="project-context"{
                let (owner,revision)={let owner=self.0.owner.lock().map_err(|_|unavailable())?;(owner.clone(),self.0.owner_revision.load(Ordering::Acquire))};
                let project=match input.get("projectId"){Some(Value::Null)|None=>None,Some(Value::String(value))=>Some(value.as_str()),_=>return Err("voice_project_context_invalid".into())};
                if owner.0.is_empty()||owner.1.is_empty(){return Err("voice_owner_changed".into());}
                let value_json=match (self.0.resolve)(&owner.1){
                    Ok(environment) if environment.id==owner.1=>crate::voice_context::project_words(&self.0.resources.consent,&environment,project)?,
                    Ok(_)=>return Err("voice_owner_changed".into()),
                    Err(_)=>self.0.resources.consent.decode_speech("project-context","{}").map_err(|_|unavailable())?.ok_or_else(unavailable)?,
                };
                let current=self.0.owner.lock().map_err(|_|unavailable())?;
                if *current!=owner||self.0.owner_revision.load(Ordering::Acquire)!=revision||cancel.is_cancelled()||self.0.shutdown.load(Ordering::Acquire){return Err("voice_owner_changed".into());}
                return Ok(json!({"valueJson":value_json}));
            }
            if action=="cloud-message"{let message=self.0.resources.consent.cloud_message(text(input,"provider"),text(input,"reason")).map_err(|_|unavailable())?;return Ok(json!({"message":message}));}
            if action=="decode-preferences"{let value_json=self.0.resources.consent.decode_speech("preferences",text(input,"valueJson")).map_err(|_|unavailable())?.ok_or_else(||"voice_preferences_invalid".to_owned())?;return Ok(json!({"valueJson":value_json}));}
            if action=="choose-audio"{self.invalidate_all();let snapshot=self.0.resources.consent.choose_audio(text(input,"destinationJson")).map_err(|_|unavailable())?;return Ok(snapshot_value(snapshot));}
            if action=="choose-cleanup"{let enabled=input["enabled"].as_bool().ok_or_else(unavailable)?;return self.0.resources.consent.choose_cleanup(enabled).map(snapshot_value).map_err(|_|unavailable());}
            if action=="pending-cloud"{
                let acknowledgement=self.0.resources.consent.pending_cloud(text(input,"destinationJson")).map_err(|_|unavailable())?;
                let id=uuid::Uuid::new_v4().to_string();let mut table=self.0.acknowledgements.lock().map_err(|_|unavailable())?;
                if table.len()>=16{return Err("voice_operation_busy".into());}table.insert(id.clone(),acknowledgement);return Ok(json!({"acknowledgement":id}));
            }
            if action=="acknowledge-cloud"{
                let acknowledgement=self.0.acknowledgements.lock().map_err(|_|unavailable())?.remove(text(input,"acknowledgement")).ok_or_else(unavailable)?;
                let snapshot=acknowledgement.acknowledge().map(snapshot_value).map_err(|_|unavailable())?;self.invalidate_revoked();return Ok(snapshot);
            }
            if action=="begin"{return self.begin(input,cancel);}
            let cleanup=matches!(action,"delete-recording"|"release-capture"|"dispose"|"stream-finish");
            let session=self.session(session_id,cleanup)?;
            if action=="dispose"{session.invalidate();}
            let _gate=session.gate.lock().map_err(|_|unavailable())?;
            if !cleanup&&!session.active.load(Ordering::Acquire){return Err("voice_owner_changed".into());}
            match action {
                "configure"=>{
                    let decoded=self.0.resources.consent.decode_speech("options",text(input,"optionsJson")).map_err(|_|unavailable())?.ok_or_else(||"voice_options_invalid".to_owned())?;
                    let value=TranscriptionOptions::decode(&serde_json::from_str(&decoded).map_err(|_|"voice_options_unavailable")?).map_err(|_|"voice_options_invalid")?;
                    let expected=input["cleanupGrantJson"].as_str().ok_or_else(||"voice_cleanup_identity_missing".to_owned())?;
                    let cleanup=(self.0.resolve)(&session.draft_environment).and_then(|draft|session.transport.bind_cleanup_expected(&draft,&session.owner_key,expected));
                    let cleanup_failed=cleanup.is_err();let cleanup=cleanup.ok().flatten().map(Arc::new);
                    let mut options=session.options.lock().map_err(|_|unavailable())?;if options.is_some(){return Err("voice_already_prepared".into());}
                    *options=Some(Options{value,cleanup,cleanup_failed});Ok(json!({}))
                }
                "status"=>{
                    let value=run(session.transport.status(cancel,||session.active.load(Ordering::Acquire))).map_err(request_error)?;
                    let value_json=self.0.resources.consent.decode_speech("status",&value.to_string()).map_err(|_|unavailable())?.ok_or_else(||"voice_status_invalid".to_owned())?;Ok(json!({"valueJson":value_json}))
                }
                "cloud-keys"=>{let keys=run(session.transport.cloud_keys(cancel,||session.active.load(Ordering::Acquire))).map_err(request_error)?;let mut value=json!({"openai":keys.openai,"xai":keys.xai});if let Some(openrouter)=keys.openrouter{value["openrouter"]=json!(openrouter);}Ok(value)}
                "stream-ticket"=>{
                    if session.ticket_used.swap(true,Ordering::AcqRel){return Err("voice_already_prepared".into());}
                    let options=session.options.lock().map_err(|_|unavailable())?.as_ref().ok_or_else(unavailable)?.value.clone();
                    let opening=run(PreparedStream::prepare(session.transport.clone(),options,session.owner.clone(),cancel.clone())).map_err(live_error)?;
                    let mut slot=session.opening.lock().map_err(|_|unavailable())?;if slot.is_some(){return Err("voice_already_prepared".into());}*slot=Some(opening);Ok(json!({}))
                }
                "stream-connect"=>{
                    if session.connect_used.swap(true,Ordering::AcqRel){return Err("voice_already_prepared".into());}
                    let opening=session.opening.lock().map_err(|_|unavailable())?.take().ok_or_else(unavailable)?;
                    let weak=Arc::downgrade(&self.0);let id=session_id.to_owned();
                    let stream=opening.connect(move|event|{let Some(state)=weak.upgrade()else{return;};let host=Host(state);let value=match event{
                        StreamEvent::Connected=>json!({"event":"connected"}),
                        StreamEvent::Voice(LiveEvent::Ready)=>json!({"event":"ready"}),
                        StreamEvent::Voice(LiveEvent::Update{revision,text})=>json!({"event":"text","revision":revision,"text":text.map(|text|json!({"committed":text.committed,"tentative":text.tentative}))}),
                        StreamEvent::Voice(LiveEvent::Finished(_))=>return,
                        StreamEvent::Failed(error)=>json!({"event":"error","error":live_error(error)}),
                    };(host.0.emit)(json!({"kind":"stream","session":id,"value":value}));}).map_err(|_|unavailable())?;
                    *session.stream.lock().map_err(|_|unavailable())?=Some(stream);Ok(json!({}))
                }
                "capture-prepare"=>{
                    if session.prepare_used.swap(true,Ordering::AcqRel){return Err("voice_already_prepared".into());}
                    session.released.store(false,Ordering::Release);
                    let mode=match text(input,"mode"){"streaming"=>Mode::Streaming,"buffered"=>Mode::Buffered,_=>return Err("voice_mode_invalid".into())};
                    if session.closed.load(Ordering::Acquire){return Err("voice_capture_closed".into());}
                    let streaming=session.stream.lock().map_err(|_|unavailable())?.as_ref().map(StreamWorker::feeder);
                    let options=session.options.lock().map_err(|_|unavailable())?.as_ref().ok_or_else(unavailable)?.value.clone();
                    *session.prepare_request.lock().map_err(|_|unavailable())?=Some(ticket.clone());
                    let weak=Arc::downgrade(&self.0);let id=session_id.to_owned();let target=Arc::downgrade(&session);
                    let worker=RecorderWorker::spawn(RecorderPlan{transport:session.transport.clone(),options,owner:session.owner.clone(),mode,streaming},self.0.resources.factory(text(input,"microphoneId").into()).map_err(|_|unavailable())?,session.startup.clone(),move|event|{if let (Some(state),Some(target))=(weak.upgrade(),target.upgrade()){Host(state).recorder_event(&id,&target,event);}}).map_err(|_|unavailable())?;
                    *session.recorder.lock().map_err(|_|unavailable())?=Some(worker);Ok(json!({"deferred":true}))
                }
                "permission"=>{
                    let id=self.capture_request(&session,ticket)?;let recorder=session.recorder.lock().map_err(|_|unavailable())?;
                    recorder.as_ref().ok_or_else(unavailable)?.permission(id,text(input,"permissionId").into(),input["allow"].as_bool().ok_or_else(unavailable)?).map_err(|_|unavailable())?;Ok(json!({"deferred":true}))
                }
                "record"=>{
                    let id=self.capture_request(&session,ticket)?;let duration=input["seconds"].as_f64().filter(|v|v.is_finite()&&*v>0.0&&*v<=300.0).ok_or_else(unavailable)?;
                    session.recorder.lock().map_err(|_|unavailable())?.as_ref().ok_or_else(unavailable)?.record(id,std::time::Duration::from_secs_f64(duration)).map_err(|_|unavailable())?;Ok(json!({"deferred":true}))
                }
                "stop"=>{let id=self.capture_request(&session,ticket)?;session.recorder.lock().map_err(|_|unavailable())?.as_ref().ok_or_else(unavailable)?.stop(id).map_err(|_|unavailable())?;Ok(json!({"deferred":true}))}
                "release-capture"=>{
                    let worker=session.recorder.lock().map_err(|_|unavailable())?.take();
                    // No worker and no child means physical ownership never began.
                    // Once a child existed only an actual confirmed receipt can
                    // release it; a consumed unknown handle remains sticky.
                    let cleanup=if session.cleanup_unknown.load(Ordering::Acquire){
                        if let Some(worker)=worker{let _=worker.close();}Cleanup::Unknown
                    }else{worker.map(RecorderWorker::close).unwrap_or(
                        if !session.startup.child_started()||session.released.load(Ordering::Acquire){Cleanup::Confirmed}else{Cleanup::Unknown}
                    )};
                    session.released.store(cleanup==Cleanup::Confirmed,Ordering::Release);
                    if cleanup==Cleanup::Unknown{session.cleanup_unknown.store(true,Ordering::Release);}
                    if cleanup==Cleanup::Confirmed{Ok(json!({}))}else{Err("voice_cleanup_unknown".into())}
                }
                "stream-finish"=>{
                    if !session.stopped.load(Ordering::Acquire)||!session.released.load(Ordering::Acquire){return Err("voice_capture_not_released".into());}
                    let worker=session.stream.lock().map_err(|_|unavailable())?.take().ok_or_else(unavailable)?;
                    worker.finish().map_err(live_error)?;worker.join().map(|text|json!({"text":text})).map_err(live_error)
                }
                "transcribe"=>{
                    let recording=session.recordings.lock().map_err(|_|unavailable())?.get(text(input,"uri")).cloned().ok_or_else(||"voice_recording_unavailable".to_owned())?;
                    run(recording.transcribe(cancel)).map(|text|json!({"text":text})).map_err(request_error)
                }
                "delete-recording"=>{session.recordings.lock().map_err(|_|unavailable())?.remove(text(input,"uri"));Ok(json!({}))}
                "post-process"=>{
                    let (cleanup,failed)={let options=session.options.lock().map_err(|_|unavailable())?;let options=options.as_ref().ok_or_else(unavailable)?;(options.cleanup.clone(),options.cleanup_failed)};
                    if failed{return Err("voice_cleanup_unavailable".into());}
                    let Some(cleanup)=cleanup else{return Ok(json!({"text":Value::Null}));};
                    let decoded=self.0.resources.consent.decode_speech("postprocess-request",text(input,"inputJson")).map_err(|_|unavailable())?.ok_or_else(||"voice_cleanup_input_invalid".to_owned())?;
                    // Native-valid UTF16 outside Rust's scalar JSON domain is
                    // refused explicitly, never replaced or silently omitted.
                    let scalar:Value=serde_json::from_str(&decoded).map_err(|_|"voice_cleanup_input_unrepresentable")?;
                    let payload=PostProcessRequest::decode(&scalar).map_err(|_|"voice_cleanup_input_invalid")?;
                    run(cleanup.post_process(&session.owner_key,&session.draft_environment,&payload,cancel,||session.active.load(Ordering::Acquire))).map(|text|json!({"text":text})).map_err(request_error)
                }
                "dispose"=>{session.invalidate();let worker=session.recorder.lock().map_err(|_|unavailable())?.take();if let Some(worker)=worker{if worker.close()!=Cleanup::Confirmed{session.cleanup_unknown.store(true,Ordering::Release);}}
if session.cleanup_unknown.load(Ordering::Acquire){return Err("voice_cleanup_unknown".into());}
if let Some(worker)=session.stream.lock().map_err(|_|unavailable())?.take(){worker.cancel();let _=worker.join();}session.opening.lock().map_err(|_|unavailable())?.take();self.0.sessions.lock().map_err(|_|unavailable())?.remove(session_id);Ok(json!({}))}
                _=>Err("voice_action_invalid".into()),
            }
        })()).filter(|result|!result.as_ref().is_ok_and(|value|value["deferred"]==true))
    }
    fn begin(&self, input: &Value, cancel: &RequestCancellation) -> Result<Value, String> {
        let (owner, revision) = {
            let owner = self.0.owner.lock().map_err(|_| unavailable())?;
            (owner.clone(), self.0.owner_revision.load(Ordering::Acquire))
        };
        if owner.0.is_empty() || owner.0 != text(input, "ownerKey") {
            return Err("voice_owner_changed".into());
        }
        let grant = self
            .0
            .resources
            .consent
            .bind_audio(text(input, "grantIdJson"))
            .map_err(|_| unavailable())?;
        let transport = Arc::new(BoundSpeech::bind(grant, |id| (self.0.resolve)(id))?);
        let active = Arc::new(AtomicBool::new(true));
        let allowed = active.clone();
        let predicate: Owner = Arc::new(move || allowed.load(Ordering::Acquire));
        let session = Arc::new(Session {
            owner_key: owner.0,
            draft_environment: owner.1,
            owner_epoch: owner.2,
            active,
            owner: predicate,
            transport,
            options: Mutex::new(None),
            opening: Mutex::new(None),
            stream: Mutex::new(None),
            recorder: Mutex::new(None),
            startup: StartupCancellation::default(),
            network: RequestCancellation::default(),
            recordings: Mutex::new(HashMap::new()),
            capture_pending: Mutex::new(HashMap::new()),
            prepare_request: Mutex::new(None),
            closed: AtomicBool::new(false),
            gate: Mutex::new(()),
            prepare_used: AtomicBool::new(false),
            ticket_used: AtomicBool::new(false),
            connect_used: AtomicBool::new(false),
            released: AtomicBool::new(false),
            cleanup_unknown: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        });
        if cancel.is_cancelled() {
            session.invalidate();
            return Err("voice_cancelled".into());
        }
        let snapshot = self.snapshot()?;
        let current = self.0.owner.lock().map_err(|_| unavailable())?;
        if self.0.shutdown.load(Ordering::Acquire)
            || self.0.owner_revision.load(Ordering::Acquire) != revision
            || current.0 != session.owner_key
            || current.1 != session.draft_environment
            || current.2 != session.owner_epoch
        {
            return Err("voice_owner_changed".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut sessions = self.0.sessions.lock().map_err(|_| unavailable())?;
        if sessions.len() >= 32 {
            return Err("voice_operation_busy".into());
        }
        sessions.insert(id.clone(), session);
        Ok(json!({"session":id,"consent":snapshot}))
    }
    fn forget_capture(&self, ticket: &Ticket) {
        let sessions = self
            .0
            .sessions
            .lock()
            .map(|sessions| sessions.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for session in sessions {
            if let Ok(mut pending) = session.capture_pending.lock() {
                pending.retain(|_, entry| entry.generation != ticket.generation);
            }
            if let Ok(mut prepare) = session.prepare_request.lock() {
                if prepare
                    .as_ref()
                    .is_some_and(|entry| entry.generation == ticket.generation)
                {
                    prepare.take();
                }
            }
        }
    }
    /// Stops admission first, then drains owned capture and stream workers.
    /// Unknown capture cleanup remains an error rather than a success receipt.
    pub fn shutdown(&self) -> Result<(), String> {
        {
            let _admission = self.0.actions.lock().map_err(|_| unavailable())?;
            self.0.shutdown.store(true, Ordering::Release);
        }
        if let Ok(requests) = self.0.requests.lock() {
            for request in requests.values() {
                request.cancel.cancel();
            }
        }
        let sessions = self
            .0
            .sessions
            .lock()
            .map_err(|_| unavailable())?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for session in &sessions {
            session.invalidate();
        }
        let mut unknown = false;
        for session in sessions {
            let _gate = session.gate.lock().map_err(|_| unavailable())?;
            let recorder = session.recorder.lock().map_err(|_| unavailable())?.take();
            if let Some(recorder) = recorder {
                if recorder.close() != Cleanup::Confirmed {
                    session.cleanup_unknown.store(true, Ordering::Release);
                }
            }
            unknown |= session.cleanup_unknown.load(Ordering::Acquire);
            let stream = session.stream.lock().map_err(|_| unavailable())?.take();
            if let Some(stream) = stream {
                stream.cancel();
                let _ = stream.join();
            }
        }
        {
            let mut count = self.0.actions.lock().map_err(|_| unavailable())?;
            while *count != 0 {
                count = self.0.actions_done.wait(count).map_err(|_| unavailable())?;
            }
        }
        unknown |= self.0.resources.consent.close() != Cleanup::Confirmed;
        let listener = self
            .0
            .consent_listener
            .lock()
            .map_err(|_| unavailable())?
            .take();
        if let Some(listener) = listener {
            unknown |= listener.join().is_err();
        }
        if unknown {
            return Err("voice_cleanup_unknown".into());
        }
        self.0.sessions.lock().map_err(|_| unavailable())?.clear();
        self.0
            .acknowledgements
            .lock()
            .map_err(|_| unavailable())?
            .clear();
        Ok(())
    }
    fn snapshot(&self) -> Result<Value, String> {
        self.0
            .resources
            .consent
            .snapshot()
            .map(snapshot_value)
            .map_err(|_| unavailable())
    }
    fn invalidate_all(&self) {
        if let Ok(sessions) = self.0.sessions.lock() {
            for session in sessions.values() {
                session.invalidate();
            }
        }
    }
    fn invalidate_revoked(&self) {
        let sessions = self
            .0
            .sessions
            .lock()
            .map(|sessions| sessions.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for session in sessions {
            if session.active.load(Ordering::Acquire)
                && !session
                    .transport
                    .current(&|| session.active.load(Ordering::Acquire))
            {
                session.invalidate();
            }
        }
    }
    fn capture_request(&self, session: &Session, request: &Ticket) -> Result<u64, String> {
        let id = self.0.sequence.fetch_add(1, Ordering::AcqRel);
        session
            .capture_pending
            .lock()
            .map_err(|_| unavailable())?
            .insert(id, request.clone());
        Ok(id)
    }
    fn recorder_event(&self, id: &str, session: &Session, event: RecorderEvent) {
        let complete = |operation: u64, value: Value| {
            if let Ok(mut pending) = session.capture_pending.lock() {
                if let Some(request) = pending.remove(&operation) {
                    self.result(&request, id, Ok(value));
                }
            }
        };
        match event {
            RecorderEvent::Prepared => {
                if let Ok(mut pending) = session.prepare_request.lock() {
                    if let Some(request) = pending.take() {
                        self.result(&request, id, Ok(json!({})));
                    }
                }
            }
            RecorderEvent::Permission { request_id } => {
                (self.0.emit)(json!({"kind":"permission","session":id,"permissionId":request_id}))
            }
            RecorderEvent::Level(level) => {
                (self.0.emit)(json!({"kind":"level","session":id,"value":level}))
            }
            RecorderEvent::DurationLimit => {
                (self.0.emit)(json!({"kind":"duration-limit","session":id}))
            }
            RecorderEvent::Recorded { id } => complete(id, json!({})),
            RecorderEvent::PermissionResult { id, ok } => complete(id, json!({"allowed":ok})),
            RecorderEvent::Stopped {
                id: operation,
                recording,
                ..
            } => {
                session.stopped.store(true, Ordering::Release);
                let uri = recording.map(|recording| {
                    let uri = recording.id().to_owned();
                    if let Ok(mut recordings) = session.recordings.lock() {
                        recordings.insert(uri.clone(), Arc::new(recording));
                    }
                    uri
                });
                complete(operation, json!({"uri":uri}));
            }
            RecorderEvent::Failed {
                id: operation,
                error,
            } => {
                let request = operation
                    .and_then(|id| session.capture_pending.lock().ok()?.remove(&id))
                    .or_else(|| session.prepare_request.lock().ok()?.take());
                if let Some(request) = request {
                    self.result(&request, id, Err(error.into()));
                } else {
                    (self.0.emit)(json!({"kind":"capture-error","session":id,"error":error}));
                }
            }
            RecorderEvent::Closed(cleanup) => {
                session.closed.store(true, Ordering::Release);
                if cleanup == Cleanup::Unknown {
                    session.cleanup_unknown.store(true, Ordering::Release);
                }
                let error = if cleanup == Cleanup::Confirmed {
                    "voice_cancelled"
                } else {
                    "voice_cleanup_unknown"
                };
                if let Ok(mut prepare) = session.prepare_request.lock() {
                    if let Some(request) = prepare.take() {
                        self.result(&request, id, Err(error.into()));
                    }
                }
                if let Ok(mut pending) = session.capture_pending.lock() {
                    for (_, request) in pending.drain() {
                        self.result(&request, id, Err(error.into()));
                    }
                }
            }
        }
    }
}
fn snapshot_value(snapshot: Snapshot) -> Value {
    json!({"choiceJson":snapshot.choice_json,"cleanupJson":snapshot.cleanup_json})
}
fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Voice action runtime");
    let value = runtime.block_on(future);
    runtime.shutdown_background();
    value
}
fn request_error(error: RequestFailure) -> String {
    match error {
        RequestFailure::Cancelled => "voice_cancelled",
        RequestFailure::ConsentRevoked => "voice_consent_revoked",
        _ => "voice_request_failed",
    }
    .into()
}
fn live_error(error: LiveFailure) -> String {
    match error {
        LiveFailure::Cancelled => "voice_cancelled".into(),
        LiveFailure::ConsentRevoked => "voice_consent_revoked".into(),
        _ => error.message().into(),
    }
}
