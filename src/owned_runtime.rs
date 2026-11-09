//! Explicit private desktop session. Service mode never instantiates this owner.
//! Browser authority follows the child and its inherited descriptors; RPC
//! reconnects refresh admission without replacing the existing page leases.
use crate::{browser_sessions::Manager, desktop_browser_ipc as ipc, owned_backend, *};
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tungstenite::{stream::MaybeTlsStream, Message};

pub const ENVIRONMENT_ID: &str = "owned-o3";
static LAUNCH: Mutex<()> = Mutex::new(());
type ActiveOwner = (Arc<owned_backend::Owner>, Arc<AtomicBool>);
static ACTIVE: OnceLock<Mutex<Option<ActiveOwner>>> = OnceLock::new();
fn active() -> &'static Mutex<Option<ActiveOwner>> {
    ACTIVE.get_or_init(|| Mutex::new(None))
}
fn environment(owner: &owned_backend::Owner) -> Result<Environment> {
    Ok(Environment {
        id: ENVIRONMENT_ID.into(),
        name: "O3 desktop session".into(),
        base_url: owner.origin().into(),
        token_file: PathBuf::new(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(owner.fresh_connection()?)),
    })
}
/// Copy the owner out before potentially blocking authorization renewal.
pub fn current_environment() -> Result<Option<Environment>> {
    let owner = active()
        .lock()
        .map_err(|_| "owned_backend_state_unavailable")?
        .clone();
    owner
        .map(|(owner, cancelled)| {
            if cancelled.load(Ordering::SeqCst) {
                return Err("owned_backend_stopped".into());
            }
            environment(&owner)
        })
        .transpose()
}

pub struct Runtime {
    owner: Arc<owned_backend::Owner>,
    manager: Manager,
    endpoints: ipc::ParentEndpoints,
    cancelled: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
    stopped: bool,
}
impl Runtime {
    /// Starts only a dedicated owned profile. No service is stopped or adopted.
    /// Returns after global registration and authoritative DB-thread recovery.
    pub fn launch(config: &owned_backend::Config, manager: Manager) -> Result<Self> {
        let _launch = LAUNCH
            .try_lock()
            .map_err(|_| "owned_backend_control_busy")?;
        if active()
            .lock()
            .map_err(|_| "owned_backend_state_unavailable")?
            .is_some()
        {
            return Err("owned_backend_already_running".into());
        }
        let (owner, endpoints) = owned_backend::Owner::launch_with_browser(config)?;
        let owner = Arc::new(owner);
        let env = environment(&owner)?;
        let (_, admission) = preview_transport::inspect(&env)?;
        if admission["canOperate"] != true || admission["serverBrowser"] != true {
            return Err("server_browser_surface_required".into());
        }
        let origin = text(&admission, "originKey").to_owned();
        let writer = Arc::new(Mutex::new(
            endpoints
                .events
                .try_clone()
                .map_err(|_| "owned_backend_ipc_unavailable")?,
        ));
        let cancelled = Arc::new(AtomicBool::new(false));
        let failed = cancelled.clone();
        let event_failure_control = endpoints
            .control
            .try_clone()
            .map_err(|_| "owned_backend_ipc_unavailable")?;
        manager.bind_private(
            ENVIRONMENT_ID,
            &origin,
            Arc::new(move |event| {
                let Some(bytes) = ipc::encode_event(&event) else {
                    failed.store(true, Ordering::SeqCst);
                    let _ = event_failure_control.shutdown(std::net::Shutdown::Both);
                    return;
                };
                if writer
                    .lock()
                    .ok()
                    .is_none_or(|mut stream| stream.write_all(&bytes).is_err())
                {
                    failed.store(true, Ordering::SeqCst);
                    let _ = event_failure_control.shutdown(std::net::Shutdown::Both);
                }
            }),
        )?;
        let mut runtime = Self {
            owner,
            manager,
            endpoints,
            cancelled,
            workers: vec![],
            stopped: false,
        };
        let mut control = runtime
            .endpoints
            .control
            .try_clone()
            .map_err(|_| "owned_backend_ipc_unavailable")?;
        let stop = runtime.cancelled.clone();
        let manager = runtime.manager.clone();
        let control_origin = origin.clone();
        runtime.workers.push(thread::spawn(move || {
            let mut decoder = ipc::ControlDecoder::new(ipc::MAX_CONTROL_WIRE_BYTES);
            let mut bytes = [0; 64 * 1024];
            while !stop.load(Ordering::SeqCst) {
                let Ok(count) = control.read(&mut bytes) else {
                    break;
                };
                let decoded = if count == 0 {
                    decoder.finish()
                } else {
                    decoder.feed(&bytes[..count])
                };
                for line in decoded.values {
                    if let Some(command) = ipc::decode_command_line(&line) {
                        if manager
                            .private_command(ENVIRONMENT_ID, &control_origin, command)
                            .is_err()
                        {
                            stop.store(true, Ordering::SeqCst);
                            break;
                        }
                    }
                }
                if count == 0 || decoded.cancelled {
                    break;
                }
            }
            stop.store(true, Ordering::SeqCst);
            manager.unbind_private(ENVIRONMENT_ID);
        }));
        let (ready, wait) = std::sync::mpsc::sync_channel(1);
        let owner = runtime.owner.clone();
        let stop = runtime.cancelled.clone();
        let manager = runtime.manager.clone();
        runtime.workers.push(thread::spawn(move || {
            let mut first = Some(ready);
            while !stop.load(Ordering::SeqCst) {
                let result = observe(&owner, &manager, &stop, &mut first);
                if let Some(ready) = first.take() {
                    let _ = ready.send(result);
                    break;
                }
                // Preserve the actual pages across a transient observer failure.
                for _ in 0..10 {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }));
        wait.recv_timeout(Duration::from_secs(40))
            .map_err(|_| "owned_backend_observer_unavailable")??;
        if runtime.cancelled.load(Ordering::SeqCst) {
            return Err("owned_backend_ipc_unavailable".into());
        }
        let mut slot = active()
            .lock()
            .map_err(|_| "owned_backend_state_unavailable")?;
        if slot.is_some() {
            return Err("owned_backend_already_running".into());
        }
        *slot = Some((runtime.owner.clone(), runtime.cancelled.clone()));
        drop(slot);
        Ok(runtime)
    }
    pub fn status(&self) -> Value {
        json!({"owner":"gui","id":ENVIRONMENT_ID,"running":!self.cancelled.load(Ordering::SeqCst),"canControl":true,"installed":true})
    }
    pub fn stop(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        self.cancelled.store(true, Ordering::SeqCst);
        // Interrupt physical I/O before joining; never wait under the slot lock.
        self.endpoints.close();
        let result = self.owner.stop();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        self.manager.unbind_private(ENVIRONMENT_ID);
        {
            let mut slot = active()
                .lock()
                .map_err(|_| "owned_backend_state_unavailable")?;
            if slot
                .as_ref()
                .is_some_and(|(owner, _)| Arc::ptr_eq(owner, &self.owner))
            {
                slot.take();
            }
        }
        result
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn reconcile(owner: &owned_backend::Owner, manager: &Manager, thread: &str) -> Result<()> {
    manager.reconcile_private(preview_transport::read(&environment(owner)?, thread)?)
}
fn reconcile_database(owner: &owned_backend::Owner, manager: &Manager) -> Result<()> {
    let connection = owner.fresh_connection()?;
    let shell = connection.request_json("GET", "/api/orchestration/shell", None, true)?;
    let mut ids = HashSet::new();
    for collection in ["threads", "archivedThreads"] {
        for row in shell[collection].as_array().ok_or("invalid_server_state")? {
            let id = row["id"]
                .as_str()
                .or_else(|| row["threadId"].as_str())
                .ok_or("invalid_server_state")?;
            if ids.insert(id.to_owned()) {
                reconcile(owner, manager, id)?;
            }
        }
    }
    Ok(())
}
fn observe(
    owner: &owned_backend::Owner,
    manager: &Manager,
    stop: &AtomicBool,
    ready: &mut Option<std::sync::mpsc::SyncSender<Result<()>>>,
) -> Result<()> {
    let connection = owner.fresh_connection()?;
    let ticket = connection.request_json("POST", "/api/auth/websocket-ticket/rpc", None, false)?;
    let mut endpoint = reqwest::Url::parse(owner.origin()).map_err(|_| "invalid_server_state")?;
    endpoint
        .set_scheme("ws")
        .map_err(|_| "invalid_server_state")?;
    endpoint.set_path("/ws");
    endpoint
        .query_pairs_mut()
        .append_pair("orchestrationProtocol", "2")
        .append_pair(
            "wsTicket",
            ticket["ticket"].as_str().ok_or("invalid_server_state")?,
        );
    let (mut socket, _) =
        tungstenite::connect(endpoint.as_str()).map_err(|_| "preview_unavailable")?;
    let MaybeTlsStream::Plain(stream): &mut MaybeTlsStream<TcpStream> = socket.get_mut() else {
        return Err("invalid_server_state".into());
    };
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| "preview_unavailable")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| "preview_unavailable")?;
    socket.send(Message::Text(json!({"_tag":"Request","id":"events","tag":"subscribePreviewEvents","payload":{},"headers":[]}).to_string().into())).map_err(|_| "preview_unavailable")?;
    socket
        .send(Message::Text(json!({"_tag":"Ping"}).to_string().into()))
        .map_err(|_| "preview_unavailable")?;
    let mut registered = false;
    let mut seen = Instant::now();
    let mut ping = Instant::now();
    while !stop.load(Ordering::SeqCst) {
        match socket.read() {
            Ok(Message::Text(raw)) => {
                seen = Instant::now();
                let packet: Value =
                    serde_json::from_str(&raw).map_err(|_| "invalid_preview_state")?;
                match text(&packet, "_tag") {
                    "Pong" if !registered => {
                        // The Pong is ordered after subscription registration.
                        reconcile_database(owner, manager)?;
                        registered = true;
                        if let Some(ready) = ready.take() {
                            let _ = ready.send(Ok(()));
                        }
                    }
                    "Chunk" if packet["requestId"] == "events" => {
                        for event in packet["values"].as_array().ok_or("invalid_preview_state")? {
                            let event = preview_transport::event(event)?;
                            reconcile(owner, manager, text(&event, "threadId"))?;
                        }
                        socket
                            .send(Message::Text(
                                json!({"_tag":"Ack","requestId":"events"})
                                    .to_string()
                                    .into(),
                            ))
                            .map_err(|_| "preview_unavailable")?;
                    }
                    "Exit" if packet["requestId"] == "events" => {
                        return Err("preview_unavailable".into())
                    }
                    "Ping" => {
                        socket
                            .send(Message::Text(json!({"_tag":"Pong"}).to_string().into()))
                            .map_err(|_| "preview_unavailable")?;
                    }
                    _ => {}
                }
            }
            Ok(Message::Ping(data)) => {
                socket
                    .send(Message::Pong(data))
                    .map_err(|_| "preview_unavailable")?;
            }
            Ok(Message::Close(_)) => return Err("preview_unavailable".into()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Err("preview_unavailable".into()),
        }
        if seen.elapsed() > Duration::from_secs(30) {
            return Err("preview_unavailable".into());
        }
        if ping.elapsed() > Duration::from_secs(10) {
            socket
                .send(Message::Text(json!({"_tag":"Ping"}).to_string().into()))
                .map_err(|_| "preview_unavailable")?;
            ping = Instant::now();
        }
    }
    let _ = socket.close(None);
    Ok(())
}
