//! Actual consent, capture owner and async SDK worker in private Podman only.
use omarchy_t3_client::{
    browser_native_display::Display,
    protocol::speech::{
        decode_pcm_request, LiveFailure, RequestCancellation, RequestFailure, TranscriptionOptions,
    },
    voice_capture::{
        resources::{Assets, DescriptorArtifact, Runtime},
        Cleanup, Mode, StartupCancellation, StartupFailure,
    },
    voice_consent::ConsentOwner,
    voice_transport::BoundSpeech,
    voice_worker::{
        CaptureFactory, Owner, PreparedStream, RecorderEvent, RecorderPlan, RecorderWorker,
    },
    Environment,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::{DirBuilderExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};
fn listener() -> (TcpListener, String) {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    socket.set_nonblocking(true).unwrap();
    let base = format!("http://{}", socket.local_addr().unwrap());
    (socket, base)
}
fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((socket, _)) => {
                socket
                    .set_read_timeout(Some(Duration::from_secs(15)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                return socket;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("{error}"),
        }
    }
}
fn http(listener: &TcpListener, route: &str) -> (TcpStream, Vec<u8>) {
    let mut socket = accept(listener);
    let mut header = vec![];
    while !header.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        header.push(byte[0]);
        assert!(header.len() < 65536);
    }
    let header = String::from_utf8(header).unwrap();
    assert!(header.starts_with(route));
    assert!(header
        .to_ascii_lowercase()
        .contains("authorization: bearer private-worker-write\r\n"));
    let size = header
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>().unwrap())
        .unwrap_or(0);
    assert!(size < 1_000_000);
    let mut body = vec![0; size];
    socket.read_exact(&mut body).unwrap();
    (socket, body)
}
fn respond(socket: &mut TcpStream, status: &str, body: Value) {
    let body = body.to_string();
    write!(socket,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
}
fn authorize(listener: &TcpListener) {
    let (mut socket, body) = http(listener, "GET /api/auth/session ");
    assert!(body.is_empty());
    respond(
        &mut socket,
        "200 OK",
        json!({"authenticated":true,"permissions":["orchestration:operate"]}),
    );
}
struct Upgrade;
impl tungstenite::handshake::server::Callback for Upgrade {
    fn on_request(
        self,
        request: &tungstenite::handshake::server::Request,
        response: tungstenite::handshake::server::Response,
    ) -> Result<
        tungstenite::handshake::server::Response,
        tungstenite::handshake::server::ErrorResponse,
    > {
        assert_eq!(
            request.uri().to_string(),
            "/ws/voice?wsTicket=private-worker-ticket"
        );
        assert!(request.headers().get("authorization").is_none());
        Ok(response)
    }
}
fn media(listener: &TcpListener) -> tungstenite::WebSocket<TcpStream> {
    authorize(listener);
    let (mut request, _) = http(listener, "POST /api/auth/websocket-ticket/media ");
    respond(
        &mut request,
        "200 OK",
        json!({"ticket":"private-worker-ticket"}),
    );
    let mut socket = tungstenite::accept_hdr(accept(listener), Upgrade).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(socket.read().unwrap().to_text().unwrap()).unwrap()["type"],
        "start"
    );
    socket
}
fn send(socket: &mut tungstenite::WebSocket<TcpStream>, value: Value) {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .unwrap();
}
fn environment(base: &str) -> Environment {
    Environment {
        id: "chosen".into(),
        name: String::new(),
        base_url: base.into(),
        token_file: PathBuf::from("/state/read-token"),
        write_token_file: Some(PathBuf::from("/state/write-token")),
        bearer: None,
        connection_error: None,
        connection: None,
    }
}
fn bind(consent: &ConsentOwner, base: &str) -> Arc<BoundSpeech> {
    let choice: Value = serde_json::from_str(&consent.snapshot().unwrap().choice_json).unwrap();
    let grant = consent.bind_audio(&choice["id"].to_string()).unwrap();
    Arc::new(
        BoundSpeech::bind(grant, |id| {
            assert_eq!(id, "chosen");
            Ok(environment(base))
        })
        .unwrap(),
    )
}
fn receive(events: &mpsc::Receiver<RecorderEvent>) -> RecorderEvent {
    events
        .recv_timeout(Duration::from_secs(12))
        .expect("Actual recorder event")
}
fn permission(events: &mpsc::Receiver<RecorderEvent>) -> String {
    loop {
        match receive(events) {
            RecorderEvent::Permission { request_id } => return request_id,
            RecorderEvent::Failed { error, .. } => panic!("{error}"),
            _ => {}
        }
    }
}
fn prepared(events: &mpsc::Receiver<RecorderEvent>) {
    loop {
        match receive(events) {
            RecorderEvent::Prepared => return,
            RecorderEvent::Failed { error, .. } => panic!("{error}"),
            _ => {}
        }
    }
}
fn stopped(
    events: &mpsc::Receiver<RecorderEvent>,
) -> (Option<omarchy_t3_client::voice_worker::Recording>, usize) {
    loop {
        match receive(events) {
            RecorderEvent::Stopped {
                recording,
                pcm_bytes,
                ..
            } => return (recording, pcm_bytes),
            RecorderEvent::Failed { error, .. } => panic!("{error}"),
            _ => {}
        }
    }
}
fn children() -> BTreeSet<u32> {
    let mut children = BTreeSet::new();
    for entry in fs::read_dir("/proc/self/task").unwrap() {
        let file = entry.unwrap().path().join("children");
        if let Ok(text) = fs::read_to_string(file) {
            children.extend(text.split_whitespace().map(|id| id.parse::<u32>().unwrap()));
        }
    }
    children
}
fn main() {
    assert!(Path::new("/run/.containerenv").exists());
    for (name, value) in [
        ("read-token", "private-worker-read"),
        ("write-token", "private-worker-write"),
    ] {
        let path = Path::new("/state").join(name);
        fs::write(&path, value).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fs::create_dir_all("/tmp/.X11-unix").unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create("/state/runtime")
        .unwrap();
    std::env::set_var("XDG_RUNTIME_DIR", "/state/runtime");
    std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/state/no-bus");
    let runtime = Arc::new(Runtime::admit(Path::new("/electron")).unwrap());
    let path = PathBuf::from("/source/electron-private-fd.node");
    let descriptor = DescriptorArtifact {
        bytes: fs::metadata(&path).unwrap().len(),
        sha256: format!("{:x}", Sha256::digest(fs::read(&path).unwrap())),
        path,
    };
    let assets =
        Arc::new(Assets::capture(Path::new("/state/capture-assets"), &descriptor).unwrap());
    let consent_assets =
        Arc::new(Assets::consent(Path::new("/state/consent-assets"), &descriptor).unwrap());
    fs::DirBuilder::new()
        .mode(0o700)
        .create("/state/grants")
        .unwrap();
    let consent = ConsentOwner::launch(
        runtime.clone(),
        consent_assets,
        Path::new("/state/consent"),
        Path::new("/state/grants"),
    )
    .unwrap();
    consent
        .choose_audio(r#"{"kind":"environment","environmentId":"chosen"}"#)
        .unwrap();
    let display = Arc::new(Display::launch(Path::new("/usr/bin/Xvfb")).unwrap());
    let factory = |name: &str| {
        CaptureFactory::new(
            runtime.clone(),
            assets.clone(),
            Path::new("/state").join(name),
            display.clone(),
            String::new(),
        )
        .with_synthetic_device()
        .unwrap()
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let options = TranscriptionOptions::default();

    // The peer withholds ready until the REAL microphone prompt exists. A host
    // that incorrectly waits for stream readiness before capture deadlocks here.
    let (socket, base) = listener();
    let (mic_seen, mic_wait) = mpsc::channel();
    let (pcm_seen, pcm_wait) = mpsc::channel();
    let released = Arc::new(AtomicBool::new(false));
    let server_released = released.clone();
    let server = thread::spawn(move || {
        let mut stream = media(&socket);
        mic_wait.recv_timeout(Duration::from_secs(12)).unwrap();
        send(&mut stream, json!({"type":"ready"}));
        let mut bytes = 0;
        let mut revision = 0;
        loop {
            let frame = stream.read().unwrap();
            if frame.is_binary() {
                let count = frame.len();
                assert!(count > 0 && count <= 64000 && count.is_multiple_of(4));
                assert!(frame
                    .into_data()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|bytes| f32::from_le_bytes(*bytes).is_finite()));
                bytes += count;
                revision += 1;
                if revision == 1 {
                    pcm_seen.send(()).unwrap();
                }
                send(
                    &mut stream,
                    json!({"type":"update","revision":revision,"text":null}),
                );
            } else {
                assert_eq!(
                    serde_json::from_str::<Value>(frame.to_text().unwrap()).unwrap(),
                    json!({"type":"finish"})
                );
                assert!(server_released.load(Ordering::Acquire));
                break;
            }
        }
        send(
            &mut stream,
            json!({"type":"finished","text":"live worker transcript"}),
        );
        bytes
    });
    let transport = bind(&consent, &base);
    let owner: Owner = Arc::new(|| true);
    let prepared_stream = rt
        .block_on(PreparedStream::prepare(
            transport.clone(),
            options.clone(),
            owner.clone(),
            RequestCancellation::default(),
        ))
        .unwrap();
    let stream = prepared_stream.connect(|_| {}).unwrap();
    let (tx, events) = mpsc::channel();
    let recorder = RecorderWorker::spawn(
        RecorderPlan {
            transport: transport.clone(),
            options: options.clone(),
            owner: owner.clone(),
            mode: Mode::Streaming,
            streaming: Some(stream.feeder()),
        },
        factory("live"),
        StartupCancellation::default(),
        move |event| {
            tx.send(event).unwrap();
        },
    )
    .unwrap();
    let request = permission(&events);
    mic_seen.send(()).unwrap();
    assert!(RecorderWorker::spawn(
        RecorderPlan {
            transport: transport.clone(),
            options: options.clone(),
            owner: owner.clone(),
            mode: Mode::Streaming,
            streaming: Some(stream.feeder())
        },
        factory("duplicate"),
        StartupCancellation::default(),
        |_| {}
    )
    .is_err());
    assert!(!Path::new("/state/duplicate").exists());
    let foreign = bind(&consent, &base);
    assert!(RecorderWorker::spawn(
        RecorderPlan {
            transport: foreign,
            options: options.clone(),
            owner: owner.clone(),
            mode: Mode::Streaming,
            streaming: Some(stream.feeder())
        },
        factory("foreign"),
        StartupCancellation::default(),
        |_| {}
    )
    .is_err());
    assert!(!Path::new("/state/foreign").exists());
    recorder.permission(11, request, true).unwrap();
    prepared(&events);
    recorder.record(12, Duration::from_secs(300)).unwrap();
    pcm_wait.recv_timeout(Duration::from_secs(8)).unwrap();
    recorder.stop(13).unwrap();
    let (recording, live_bytes) = stopped(&events);
    // Native 64k is a maximum frame size, not a minimum: the real worklet
    // normally emits a 32k packet before the user requests stop.
    assert!(recording.is_none() && live_bytes > 0);
    assert_eq!(recorder.close(), Cleanup::Confirmed);
    released.store(true, Ordering::Release);
    stream.finish().unwrap();
    assert_eq!(stream.join().unwrap(), "live worker transcript");
    assert_eq!(server.join().unwrap(), live_bytes);

    for denied in [true, false] {
        let (socket, base) = listener();
        let server = thread::spawn(move || {
            let mut stream = media(&socket);
            send(&mut stream, json!({"type":"ready"}));
            if let Ok(frame) = stream.read() {
                assert!(frame.is_close());
            }
        });
        let transport = bind(&consent, &base);
        let owner: Owner = Arc::new(|| true);
        let stream = rt
            .block_on(PreparedStream::prepare(
                transport.clone(),
                options.clone(),
                owner.clone(),
                RequestCancellation::default(),
            ))
            .unwrap()
            .connect(|_| {})
            .unwrap();
        let (tx, events) = mpsc::channel();
        let recorder = RecorderWorker::spawn(
            RecorderPlan {
                transport,
                options: options.clone(),
                owner,
                mode: Mode::Streaming,
                streaming: Some(stream.feeder()),
            },
            factory(if denied {
                "denied"
            } else {
                "cancel-permission"
            }),
            StartupCancellation::default(),
            move |event| {
                tx.send(event).unwrap();
            },
        )
        .unwrap();
        let request = permission(&events);
        if denied {
            recorder.permission(21, request, false).unwrap();
            loop {
                if let RecorderEvent::Failed { error, .. } = receive(&events) {
                    assert_eq!(error, "capture_permission_denied");
                    break;
                }
            }
        } else {
            recorder.abort();
        }
        assert_eq!(recorder.close(), Cleanup::Confirmed);
        assert_eq!(stream.join(), Err(LiveFailure::Cancelled));
        server.join().unwrap();
    }

    let (socket, base) = listener();
    let transport = bind(&consent, &base);
    let current = Arc::new(AtomicBool::new(true));
    let owned = current.clone();
    let owner: Owner = Arc::new(move || owned.load(Ordering::Acquire));
    let (tx, events) = mpsc::channel();
    let recorder = RecorderWorker::spawn(
        RecorderPlan {
            transport: transport.clone(),
            options: options.clone(),
            owner: owner.clone(),
            mode: Mode::Buffered,
            streaming: None,
        },
        factory("buffered"),
        StartupCancellation::default(),
        move |event| {
            tx.send(event).unwrap();
        },
    )
    .unwrap();
    let request = permission(&events);
    recorder.permission(31, request, true).unwrap();
    prepared(&events);
    recorder.record(32, Duration::from_millis(350)).unwrap();
    loop {
        match receive(&events) {
            RecorderEvent::DurationLimit => break,
            RecorderEvent::Failed { error, .. } => panic!("{error}"),
            _ => {}
        }
    }
    recorder.stop(33).unwrap();
    let (recording, buffered_bytes) = stopped(&events);
    let recording = recording.unwrap();
    assert!(recording.id().starts_with("o3-voice-recording:") && buffered_bytes > 0);
    assert_eq!(recorder.close(), Cleanup::Confirmed);
    let expected_options = options.clone();
    let server = thread::spawn(move || {
        let mut first = None;
        for retry in [false, true] {
            authorize(&socket);
            let (mut request, body) = http(&socket, "POST /api/voice/transcribe ");
            let (actual, pcm) = decode_pcm_request(&body).unwrap();
            assert_eq!(actual, expected_options);
            assert_eq!(pcm.len(), buffered_bytes);
            let digest = format!("{:x}", Sha256::digest(pcm));
            if let Some(first) = &first {
                assert_eq!(first, &digest);
            } else {
                first = Some(digest);
            }
            respond(
                &mut request,
                if retry {
                    "200 OK"
                } else {
                    "503 Service Unavailable"
                },
                if retry {
                    json!({"text":"retained worker transcript"})
                } else {
                    json!({"message":"synthetic inference failure"})
                },
            );
        }
        socket
    });
    assert!(rt
        .block_on(recording.transcribe(&RequestCancellation::default()))
        .is_err());
    assert_eq!(
        rt.block_on(recording.transcribe(&RequestCancellation::default()))
            .unwrap(),
        "retained worker transcript"
    );
    let socket = server.join().unwrap();
    current.store(false, Ordering::Release);
    assert_eq!(
        rt.block_on(recording.transcribe(&RequestCancellation::default())),
        Err(RequestFailure::ConsentRevoked)
    );
    assert!(socket.accept().is_err());
    drop(recording);

    let (socket, base) = listener();
    let transport = bind(&consent, &base);
    let owner: Owner = Arc::new(|| true);
    let startup = StartupCancellation::default();
    startup.cancel();
    let error = RecorderWorker::spawn(
        RecorderPlan {
            transport: transport.clone(),
            options: options.clone(),
            owner: owner.clone(),
            mode: Mode::Buffered,
            streaming: None,
        },
        factory("pre-cancelled"),
        startup,
        |_| {},
    )
    .err()
    .unwrap();
    assert_eq!(
        error
            .get_ref()
            .unwrap()
            .downcast_ref::<StartupFailure>()
            .unwrap()
            .cleanup,
        Cleanup::Confirmed
    );
    assert!(!Path::new("/state/pre-cancelled").exists());
    assert!(socket.accept().is_err());

    // A rejected artifact starts no child. Even a host callback panic cannot
    // turn that known fact into sticky physical-cleanup uncertainty.
    let bad_assets =
        Arc::new(Assets::capture(Path::new("/state/rejected-assets"), &descriptor).unwrap());
    let changed = Path::new("/state/rejected-assets/electron-capture-renderer.mjs");
    fs::set_permissions(changed, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(changed, b"invalid private fixture artifact").unwrap();
    let rejected = CaptureFactory::new(
        runtime.clone(),
        bad_assets,
        PathBuf::from("/state/rejected-capture"),
        display.clone(),
        String::new(),
    )
    .with_synthetic_device()
    .unwrap();
    let before = children();
    let recorder = RecorderWorker::spawn(
        RecorderPlan {
            transport: transport.clone(),
            options: options.clone(),
            owner: owner.clone(),
            mode: Mode::Buffered,
            streaming: None,
        },
        rejected,
        StartupCancellation::default(),
        |_| {
            panic!("Intentional pre-spawn callback failure");
        },
    )
    .unwrap();
    assert_eq!(recorder.close(), Cleanup::Confirmed);
    assert_eq!(children(), before);
    assert!(!Path::new("/state/rejected-capture").exists());

    // Kill only the exact new direct child owned by THIS worker, using pidfd.
    // Missing close ACK/nonzero exit stays Unknown and blocks all successor
    // launches in this process; no directory is erased or alternate retry used.
    let before = children();
    let (tx, events) = mpsc::channel();
    let recorder = RecorderWorker::spawn(
        RecorderPlan {
            transport: transport.clone(),
            options: options.clone(),
            owner: owner.clone(),
            mode: Mode::Buffered,
            streaming: None,
        },
        factory("unknown"),
        StartupCancellation::default(),
        move |event| {
            tx.send(event).unwrap();
        },
    )
    .unwrap();
    let _permission = permission(&events);
    let after = children();
    let new: Vec<_> = after.difference(&before).copied().collect();
    assert_eq!(new.len(), 1);
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, new[0], 0) };
    assert!(fd >= 0);
    let pidfd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                pidfd.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        },
        0
    );
    loop {
        if let RecorderEvent::Closed(cleanup) = receive(&events) {
            assert_eq!(cleanup, Cleanup::Unknown);
            break;
        }
    }
    assert_eq!(recorder.close(), Cleanup::Unknown);
    let error = RecorderWorker::spawn(
        RecorderPlan {
            transport,
            options,
            owner,
            mode: Mode::Buffered,
            streaming: None,
        },
        factory("successor"),
        StartupCancellation::default(),
        |_| {},
    )
    .err()
    .unwrap();
    assert_eq!(error.to_string(), "capture_cleanup_unknown");
    assert!(!Path::new("/state/successor").exists());
    assert_eq!(consent.close(), Cleanup::Confirmed);
    println!(
        "{}",
        json!({"cases":10,"parallelCaptureAndStreamPreparation":true,"singleUseFeeder":true,"foreignPreparationRefused":true,"liveBytes":live_bytes,"finishAfterPhysicalRelease":true,"permissionDeniedIdentity":true,"pendingPermissionAbortClosesBothOwners":true,"bufferedBytes":buffered_bytes,"explicitRetrySameRecording":true,"ownerLossNoRequest":true,"preSpawnCancelConfirmed":true,"noChildCallbackPanicConfirmed":true,"unknownRetirementSticky":true,"scope":"Actual host workers/consent/capture/SDK with synthetic microphone and private peers; Qt adapter and public activation remain pending"})
    );
}
