//! Actual consent realm to SDK HTTP, confined to private synthetic loopback.
use omarchy_t3_client::{
    browser_native_display::Display,
    protocol::speech::{
        decode_pcm_request, RequestCancellation, RequestFailure, TranscriptionOptions,
    },
    voice_capture::{
        resources::{Assets, DescriptorArtifact, Runtime},
        Capture, Cleanup, Event, Mode,
    },
    voice_consent::ConsentOwner,
    voice_transport::BoundSpeech,
    Environment,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};
fn listener() -> (TcpListener, String) {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", socket.local_addr().unwrap());
    socket.set_nonblocking(true).unwrap();
    (socket, base)
}
fn accept(listener: &TcpListener, route: &str) -> (TcpStream, Vec<u8>) {
    let end = Instant::now() + Duration::from_secs(3);
    let mut socket = loop {
        match listener.accept() {
            Ok((socket, _)) => break socket,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < end, "Synthetic voice request admission");
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("{error}"),
        }
    };
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut header = vec![];
    while !header.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        header.push(byte[0]);
        assert!(header.len() < 16384);
    }
    let header = String::from_utf8(header).unwrap();
    assert!(header.starts_with(route));
    let lower = header.to_ascii_lowercase();
    assert!(lower.contains("authorization: bearer synthetic-write-authority\r\n"));
    assert!(!lower.contains("synthetic-read-authority"));
    let length = header
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>().unwrap())
        .unwrap_or(0);
    assert!(length <= 1_048_576);
    let mut body = vec![0; length];
    socket.read_exact(&mut body).unwrap();
    (socket, body)
}
fn respond(socket: &mut TcpStream, body: Value) {
    let body = body.to_string();
    write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
}
fn authorize(listener: &TcpListener, allowed: bool) {
    let (mut socket, body) = accept(listener, "GET /api/auth/session ");
    assert!(body.is_empty());
    respond(
        &mut socket,
        json!({"authenticated":true,"permissions":if allowed {vec!["orchestration:operate"]}else{vec![]}}),
    );
}
fn environment(base: &str) -> Environment {
    Environment {
        id: "chosen-environment".into(),
        name: String::new(),
        base_url: base.into(),
        token_file: PathBuf::from("/state/read-token"),
        write_token_file: Some(PathBuf::from("/state/write-token")),
        bearer: None,
        connection_error: None,
        connection: None,
    }
}
fn selected(owner: &ConsentOwner, base: &str) -> BoundSpeech {
    let choice: Value = serde_json::from_str(&owner.snapshot().unwrap().choice_json).unwrap();
    let guard = owner.bind_audio(&choice["id"].to_string()).unwrap();
    BoundSpeech::bind(guard, |id| {
        assert_eq!(id, "chosen-environment");
        Ok(environment(base))
    })
    .unwrap()
}
fn capture_result(capture: &mut Capture, id: u64, pcm: &mut Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(
            Instant::now() < deadline,
            "Actual capture command completion"
        );
        match capture.next(Duration::from_millis(100)).unwrap() {
            Some(Event::Pcm(bytes)) => pcm.extend_from_slice(&bytes),
            Some(Event::Result {
                id: received,
                ok,
                error,
            }) if received == id => {
                assert!(ok, "{error:?}");
                return;
            }
            Some(Event::CaptureError) => panic!("Actual capture failed"),
            _ => {}
        }
    }
}
fn main() {
    assert!(Path::new("/run/.containerenv").exists());
    for (name, token) in [
        ("read-token", "synthetic-read-authority"),
        ("write-token", "synthetic-write-authority"),
    ] {
        let path = Path::new("/state").join(name);
        fs::write(&path, token).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let runtime = Arc::new(Runtime::admit(Path::new("/electron")).unwrap());
    let addon = Path::new("/source/electron-private-fd.node");
    let descriptor = DescriptorArtifact {
        path: addon.into(),
        bytes: fs::metadata(addon).unwrap().len(),
        sha256: format!("{:x}", Sha256::digest(fs::read(addon).unwrap())),
    };
    let assets = Arc::new(Assets::consent(Path::new("/state/assets"), &descriptor).unwrap());
    fs::DirBuilder::new()
        .mode(0o700)
        .create("/state/grants")
        .unwrap();
    let owner = ConsentOwner::launch(
        runtime.clone(),
        assets,
        Path::new("/state/consent"),
        Path::new("/state/grants"),
    )
    .unwrap();
    let local = json!({"kind":"environment","environmentId":"chosen-environment"}).to_string();
    owner.choose_audio(&local).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let options = TranscriptionOptions::default();
    let (socket, base) = listener();
    let server = thread::spawn(move || {
        authorize(&socket, true);
        let (mut request, body) = accept(&socket, "POST /api/voice/transcribe ");
        let (actual, pcm) = decode_pcm_request(&body).unwrap();
        assert_eq!(actual, TranscriptionOptions::default());
        assert_eq!(pcm, [0; 8]);
        respond(&mut request, json!({"text":"actual consented transcript"}));
        socket
    });
    let transport = selected(&owner, &base);
    assert_eq!(transport.environment_id(), "chosen-environment");
    assert!(!transport.is_cloud());
    assert_eq!(
        rt.block_on(transport.transcribe(
            &[0; 8],
            &options,
            &RequestCancellation::default(),
            || true
        ))
        .unwrap(),
        "actual consented transcript"
    );
    let socket = server.join().unwrap();
    owner.choose_audio(&local).unwrap();
    assert_eq!(
        rt.block_on(transport.transcribe(
            &[0; 8],
            &options,
            &RequestCancellation::default(),
            || true
        )),
        Err(RequestFailure::ConsentRevoked)
    );
    assert!(matches!(socket.accept(),Err(error)if error.kind()==std::io::ErrorKind::WouldBlock));

    let guard = owner
        .bind_audio(
            &serde_json::from_str::<Value>(&owner.snapshot().unwrap().choice_json).unwrap()["id"]
                .to_string(),
        )
        .unwrap();
    assert!(BoundSpeech::bind(guard, |_| {
        let mut foreign = environment(&base);
        foreign.id = "other-environment".into();
        Ok(foreign)
    })
    .is_err());
    let (socket, base) = listener();
    let server = thread::spawn(move || {
        authorize(&socket, false);
        socket
    });
    let transport = selected(&owner, &base);
    assert_eq!(
        rt.block_on(transport.transcribe(
            &[0; 8],
            &options,
            &RequestCancellation::default(),
            || true
        )),
        Err(RequestFailure::Transport("permission_denied".into()))
    );
    let socket = server.join().unwrap();
    assert!(matches!(socket.accept(),Err(error)if error.kind()==std::io::ErrorKind::WouldBlock));
    assert_eq!(
        rt.block_on(transport.transcribe(
            &[0; 8],
            &options,
            &RequestCancellation::default(),
            || false
        )),
        Err(RequestFailure::ConsentRevoked)
    );
    assert!(matches!(socket.accept(),Err(error)if error.kind()==std::io::ErrorKind::WouldBlock));

    let cloud=json!({"kind":"cloud","environmentId":"chosen-environment","provider":"openrouter","model":"author/model-v1"}).to_string();
    owner.pending_cloud(&cloud).unwrap().acknowledge().unwrap();
    let (socket, base) = listener();
    let server = thread::spawn(move || {
        authorize(&socket, true);
        let (mut request, _) = accept(&socket, "GET /api/voice/cloud/keys ");
        respond(
            &mut request,
            json!({"openai":false,"xai":false,"openrouter":true}),
        );
        authorize(&socket, true);
        let (mut request, body) = accept(
            &socket,
            "POST /api/voice/cloud/openrouter/transcribe?model=author%2Fmodel-v1 ",
        );
        assert_eq!(decode_pcm_request(&body).unwrap().1, [0; 8]);
        respond(&mut request, json!({"text":"selected cloud transcript"}));
    });
    let transport = selected(&owner, &base);
    assert!(transport.is_cloud());
    assert_eq!(
        rt.block_on(transport.cloud_keys(&RequestCancellation::default(), || true))
            .unwrap()
            .openrouter,
        Some(true)
    );
    assert_eq!(
        rt.block_on(transport.transcribe(
            &[0; 8],
            &options,
            &RequestCancellation::default(),
            || true
        ))
        .unwrap(),
        "selected cloud transcript"
    );
    server.join().unwrap();

    owner.choose_audio(&local).unwrap();
    let (socket, base) = listener();
    let (entered, ready) = mpsc::channel();
    let (closed, observed) = mpsc::channel();
    let server = thread::spawn(move || {
        authorize(&socket, true);
        let (mut request, _) = accept(&socket, "POST /api/voice/transcribe ");
        entered.send(()).unwrap();
        match request.read(&mut [0]) {
            Ok(0) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                ) => {}
            _ => panic!("Actual voice request must close"),
        };
        closed.send(()).unwrap();
    });
    let transport = selected(&owner, &base);
    let cancel = RequestCancellation::default();
    let stopper = cancel.clone();
    let interrupt = thread::spawn(move || {
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        stopper.cancel();
    });
    rt.block_on(async {
        assert_eq!(
            transport
                .transcribe(&[0; 8], &options, &cancel, || true)
                .await,
            Err(RequestFailure::Cancelled)
        );
        tokio::task::spawn_blocking(move || observed.recv_timeout(Duration::from_secs(3)).unwrap())
            .await
            .unwrap();
    });
    interrupt.join().unwrap();
    server.join().unwrap();

    // Exercise the real binary capture consumer as well as synthetic envelope
    // controls. The private browser supplies an explicit synthetic media device.
    let (socket, base) = listener();
    let (expected, recording) = mpsc::channel();
    let server = thread::spawn(move || {
        authorize(&socket, true);
        let (mut request, _) = accept(&socket, "GET /api/voice/status ");
        respond(
            &mut request,
            json!({"supported":true,"supportsStreaming":false,"supportsBufferedStreaming":false}),
        );
        // Start the inference admission witness after cold capture has retired;
        // model/display startup is not part of this socket phase's deadline.
        let (bytes, digest) = recording.recv_timeout(Duration::from_secs(15)).unwrap();
        authorize(&socket, true);
        let (mut request, body) = accept(&socket, "POST /api/voice/transcribe ");
        let (_, pcm) = decode_pcm_request(&body).unwrap();
        assert_eq!(pcm.len(), bytes);
        assert_eq!(format!("{:x}", Sha256::digest(pcm)), digest);
        respond(
            &mut request,
            json!({"text":"captured recording transcript"}),
        );
    });
    let transport = selected(&owner, &base);
    assert_eq!(
        rt.block_on(transport.status(&RequestCancellation::default(), || true))
            .unwrap()["supported"],
        true
    );
    fs::create_dir_all("/tmp/.X11-unix").unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create("/state/runtime")
        .unwrap();
    std::env::set_var("XDG_RUNTIME_DIR", "/state/runtime");
    std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/state/no-bus");
    let capture_assets =
        Arc::new(Assets::capture(Path::new("/state/capture-assets"), &descriptor).unwrap());
    let display = Arc::new(Display::launch(Path::new("/usr/bin/Xvfb")).unwrap());
    let mut capture = Capture::synthetic(
        runtime,
        capture_assets,
        Path::new("/state/capture"),
        display,
        Mode::Buffered,
        "",
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let permission = loop {
        assert!(Instant::now() < deadline);
        if let Some(Event::Permission { request_id }) =
            capture.next(Duration::from_millis(100)).unwrap()
        {
            break request_id;
        }
    };
    let decision = capture.permission(&permission, true).unwrap();
    let mut pcm = vec![];
    capture_result(&mut capture, decision, &mut pcm);
    capture_result(&mut capture, 1, &mut pcm);
    let record = capture.record().unwrap();
    capture_result(&mut capture, record, &mut pcm);
    let deadline = Instant::now() + Duration::from_millis(350);
    while Instant::now() < deadline {
        assert!(!matches!(
            capture.next(Duration::from_millis(25)).unwrap(),
            Some(Event::Pcm(_))
        ))
    }
    let stopped = capture.stop().unwrap();
    capture_result(&mut capture, stopped, &mut pcm);
    assert!(!pcm.is_empty());
    assert_eq!(pcm.len() % 4, 0);
    assert!(pcm.len() < 1_000_000);
    assert_eq!(capture.close(), Cleanup::Confirmed);
    let captured_bytes = pcm.len();
    expected
        .send((captured_bytes, format!("{:x}", Sha256::digest(&pcm))))
        .unwrap();
    assert_eq!(
        rt.block_on(transport.transcribe(&pcm, &options, &RequestCancellation::default(), || true))
            .unwrap(),
        "captured recording transcript"
    );
    use zeroize::Zeroize;
    pcm.zeroize();
    server.join().unwrap();
    assert_eq!(owner.close(), Cleanup::Confirmed);
    println!(
        "{}",
        json!({"cases":8,"actualConsentRealm":true,"sameWriteCredential":true,"actualBinaryPcm":true,"reselectionDeniesBeforeConnect":true,"foreignResolverDenied":true,"revokedAuthorityNoAudio":true,"ownerLossNoRequest":true,"boundOpenRouterModel":true,"cancelledInferenceSocketClosed":true,"actualChromiumCapturedBytes":captured_bytes,"captureCleanupConfirmed":true,"scope":"Actual consent/capture to SDK transport with synthetic device and server; cleanup provider, streaming and composer remain separate"})
    );
}
