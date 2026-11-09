//! Real Qt QProcess fixture entry: the production stdio parser and typed capture.
use omarchy_t3_client::{
    protocol::speech::decode_pcm_request,
    voice_capture::resources::{DescriptorArtifact, Runtime},
    voice_host::Resources,
    voice_watch, Environment,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
fn private(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
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
fn response(socket: &mut TcpStream, body: Value) {
    let body = body.to_string();
    write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
}
struct Peer {
    running: Arc<AtomicBool>,
    thread: thread::JoinHandle<(usize, usize)>,
    base: String,
}
impl Peer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let running = Arc::new(AtomicBool::new(true));
        let active = running.clone();
        let thread = thread::spawn(move || {
            let mut pcm_bytes = 0;
            let mut cleanup_calls = 0;
            while active.load(Ordering::Acquire) {
                let mut socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("{error}"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                    assert!(header.len() < 16384);
                }
                let header = String::from_utf8(header).unwrap();
                assert!(header
                    .to_ascii_lowercase()
                    .contains("authorization: bearer private-host-write\r\n"));
                assert!(!header.contains("private-host-read"));
                let size = header
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                    .unwrap_or(0);
                assert!(size < 1_048_576);
                let mut body = vec![0; size];
                socket.read_exact(&mut body).unwrap();
                if header.starts_with("GET /api/auth/session ") {
                    response(
                        &mut socket,
                        json!({"authenticated":true,"permissions":["orchestration:operate"]}),
                    );
                } else if header.starts_with("GET /api/voice/status ") {
                    response(
                        &mut socket,
                        json!({"supported":true,"state":"ready","modelId":"private-model","model":"Private synthetic model","size":1,"supportsStreaming":false,"supportsBufferedStreaming":false,"supportsTranslation":false,"acceleration":"cpu","modelUnloadTimeout":"never","gpuDevices":[]}),
                    );
                } else if header.starts_with("POST /api/voice/transcribe ") {
                    let (options, pcm) = decode_pcm_request(&body).unwrap();
                    assert!(!pcm.is_empty() && pcm.len().is_multiple_of(4));
                    assert!(pcm
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|bytes| f32::from_le_bytes(*bytes).is_finite()));
                    assert!(pcm
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|bytes| f32::from_le_bytes(*bytes).abs() > 0.0001));
                    assert_eq!(
                        options,
                        omarchy_t3_client::protocol::speech::TranscriptionOptions::default()
                    );
                    pcm_bytes += pcm.len();
                    response(&mut socket, json!({"text":"qt hosted transcript"}));
                } else if header.starts_with("POST /api/voice/post-process ") {
                    let body: Value = serde_json::from_slice(&body).unwrap();
                    assert_eq!(body["transcript"], "qt hosted transcript");
                    assert_eq!(
                        body["draft"],
                        json!({"text":"before ","selection":{"start":7,"end":7}})
                    );
                    cleanup_calls += 1;
                    response(&mut socket, json!({"text":"clean hosted transcript"}));
                } else {
                    panic!("Unexpected private speech route");
                }
            }
            (pcm_bytes, cleanup_calls)
        });
        Self {
            running,
            thread,
            base,
        }
    }
    fn close(self) -> (usize, usize) {
        self.running.store(false, Ordering::Release);
        self.thread.join().unwrap()
    }
}

fn main() {
    assert!(Path::new("/run/.containerenv").exists());
    assert_ne!(std::process::id(), 1);
    fs::create_dir_all("/tmp/.X11-unix").unwrap();
    private(Path::new("/state/runtime"));
    std::env::set_var("XDG_RUNTIME_DIR", "/state/runtime");
    std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/state/no-bus");
    for (name, value) in [
        ("read-token", "private-host-read"),
        ("write-token", "private-host-write"),
    ] {
        let path = Path::new("/state").join(name);
        fs::write(&path, value).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let runtime = Arc::new(Runtime::admit(Path::new("/electron")).unwrap());
    let path = PathBuf::from("/source/electron-private-fd.node");
    let addon = DescriptorArtifact {
        bytes: fs::metadata(&path).unwrap().len(),
        sha256: format!("{:x}", Sha256::digest(fs::read(&path).unwrap())),
        path,
    };
    private(Path::new("/state/profile"));
    private(Path::new("/state/namespace"));
    let resources = Resources::new(
        runtime,
        &addon,
        Path::new("/state/profile"),
        Path::new("/state/namespace"),
    )
    .unwrap()
    .with_synthetic_device()
    .unwrap();
    let peer = Peer::start();
    let base = peer.base.clone();
    // The draft environment is intentionally unavailable. Original vocabulary
    // defaults and disabled optional cleanup must not retarget audio to it.
    let result = voice_watch::serve(Some(resources), move |id| {
        if id == "chosen" {
            Ok(environment(&base))
        } else {
            Err("private_draft_unavailable".into())
        }
    });
    let (pcm_bytes, cleanup_calls) = peer.close();
    result.unwrap();
    assert!(pcm_bytes > 0);
    assert_eq!(cleanup_calls, 0);
    fs::write("/state/peer-receipt.json",json!({"eofShutdown":true,"actualSyntheticPcmBytes":pcm_bytes,"cleanupHttpCalls":cleanup_calls,"sameProductionWatchParser":true}).to_string()).unwrap();
}
