//! Full private CLI → browser descriptors → actual Qt/native Host proof.
//! Run only through tools/test-owned-browser-qt; fixtures are retained.
use omarchy_t3_client::{
    desktop_browser_ipc as ipc,
    owned_backend::{Config, Owner},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    os::{
        fd::AsRawFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};
type Socket = WebSocket<MaybeTlsStream<TcpStream>>;
fn send(socket: &mut Socket, id: &str, tag: &str, payload: Value) {
    socket
        .send(Message::Text(
            json!({"_tag":"Request","id":id,"tag":tag,"payload":payload})
                .to_string()
                .into(),
        ))
        .unwrap();
}
fn frame(socket: &mut Socket) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(Instant::now() < deadline, "private RPC deadline");
        match socket.read() {
            Ok(Message::Text(text)) => {
                let value: Value = serde_json::from_str(&text).unwrap();
                if value["_tag"] == "Chunk" {
                    socket
                        .send(Message::Text(
                            json!({"_tag":"Ack","requestId":value["requestId"]})
                                .to_string()
                                .into(),
                        ))
                        .unwrap();
                }
                return value;
            }
            Ok(Message::Ping(data)) => socket.send(Message::Pong(data)).unwrap(),
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            other => panic!("private RPC failed: {other:?}"),
        }
    }
}
struct Guest(Child);
impl Drop for Guest {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
#[test]
fn authoritative_preview_open_reaches_native_qt_over_actual_owned_descriptors() {
    let root = std::env::temp_dir().join(format!(
        "oq-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    ));
    fs::create_dir(&root).unwrap();
    let (owner, browser) = Owner::launch_with_browser(&Config {
        executable: PathBuf::from("/fixture/o3-server"),
        state_root: root.join("state"),
    })
    .unwrap();
    let connection = owner.fresh_connection().unwrap();
    let ticket = connection
        .request_json("POST", "/api/auth/websocket-ticket/rpc", None, false)
        .unwrap();
    let mut endpoint = reqwest::Url::parse(owner.origin()).unwrap();
    endpoint.set_scheme("ws").unwrap();
    endpoint.set_path("/ws");
    endpoint
        .query_pairs_mut()
        .append_pair("orchestrationProtocol", "2")
        .append_pair("wsTicket", ticket["ticket"].as_str().unwrap());
    let (mut socket, _) = tungstenite::connect(endpoint.as_str()).unwrap();
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
    }
    send(&mut socket, "events", "subscribePreviewEvents", json!({}));
    socket
        .send(Message::Text(json!({"_tag":"Ping"}).to_string().into()))
        .unwrap();
    while frame(&mut socket)["_tag"] != "Pong" {}
    send(
        &mut socket,
        "open",
        "preview.open",
        json!({"threadId":"thread","runtime":"server"}),
    );
    let opened = loop {
        let value = frame(&mut socket);
        assert_ne!(
            value["requestId"], "open",
            "open completed before authoritative guest creation"
        );
        if value["_tag"] == "Chunk" && value["requestId"] == "events" {
            if let Some(event) = value["values"]
                .as_array()
                .unwrap()
                .iter()
                .find(|event| event["type"] == "opened")
            {
                break event.clone();
            }
        }
    };
    let tab = opened["tabId"].as_str().unwrap();
    assert_eq!(opened["threadId"], "thread");
    let (parent, child) = UnixStream::pair().unwrap();
    let fd = unsafe { libc::fcntl(child.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
    assert!(fd >= 64);
    let mut command = Command::new("/fixture/qt-browser");
    command
        .args([
            "/fixture/guest.qml",
            &fd.to_string(),
            "thread",
            tab,
            "/fixture/bridge.js",
            "/fixture/host.js",
            "/fixture/platform.js",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .process_group(0);
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut guest = Guest(command.spawn().unwrap());
    let (loaded_sender, loaded_receiver) = std::sync::mpsc::channel();
    let status_output = guest.0.stdout.take().unwrap();
    let status = thread::spawn(move || {
        for line in BufReader::new(status_output).lines() {
            if line.unwrap() == "OWNED_PAGE_LOADED" {
                let _ = loaded_sender.send(());
            }
        }
    });
    unsafe { libc::close(fd) };
    drop(child);
    let mut output = parent.try_clone().unwrap();
    let mut control = browser.control;
    let commands = thread::spawn(move || {
        let mut decoder = ipc::ControlDecoder::default();
        let mut bytes = [0; 65536];
        loop {
            let count = control.read(&mut bytes).unwrap();
            if count == 0 {
                break;
            }
            let lines = decoder.feed(&bytes[..count]);
            assert!(!lines.cancelled);
            for line in lines.values {
                if let Some(value) = ipc::decode_command_line(&line) {
                    output.write_all(value.to_string().as_bytes()).unwrap();
                    output.write_all(b"\n").unwrap();
                }
            }
        }
        let _ = output.shutdown(Shutdown::Both);
    });
    let closing = Arc::new(AtomicBool::new(false));
    let events_closing = closing.clone();
    let mut events = browser.events;
    let incoming = thread::spawn(move || {
        let mut input = BufReader::new(parent);
        loop {
            let mut line = Vec::new();
            let count = match input
                .by_ref()
                .take(ipc::MAX_CONTROL_WIRE_BYTES as u64 + 1)
                .read_until(b'\n', &mut line)
            {
                Ok(count) => count,
                Err(error) => {
                    assert!(
                        events_closing.load(Ordering::Acquire),
                        "unexpected native event read: {error}"
                    );
                    break;
                }
            };
            if count == 0 {
                break;
            }
            assert!(line.len() <= ipc::MAX_CONTROL_WIRE_BYTES);
            let event: Value = serde_json::from_slice(&line).unwrap();
            let bytes = ipc::encode_event(&event).expect("actual native event schema");
            if let Err(error) = events.write_all(&bytes) {
                assert!(
                    events_closing.load(Ordering::Acquire),
                    "unexpected native event write: {error}"
                );
                break;
            }
        }
    });
    let opened_session = loop {
        let value = frame(&mut socket);
        if value["_tag"] == "Exit" && value["requestId"] == "open" {
            assert_eq!(
                value["exit"]["_tag"], "Success",
                "actual guest attachment failed: {value}"
            );
            break value["exit"]["value"].clone();
        }
    };
    assert_eq!(opened_session["tabId"], tab);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let stopped = Arc::new(AtomicBool::new(false));
    let stop = stopped.clone();
    let http = thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    socket
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut bytes = [0; 4096];
                    let _ = socket.read(&mut bytes);
                    let body="<!doctype html><title>Private owned Qt navigation</title><body>owned page</body>";
                    write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("private HTTP fixture: {error}"),
            }
        }
    });
    let target = format!("http://{address}/owned");
    send(
        &mut socket,
        "navigate",
        "preview.navigate",
        json!({"threadId":"thread","tabId":tab,"url":target}),
    );
    loop {
        let value = frame(&mut socket);
        if value["_tag"] == "Exit" && value["requestId"] == "navigate" {
            assert_eq!(
                value["exit"]["_tag"], "Success",
                "actual navigation failed: {value}"
            );
            assert_eq!(value["exit"]["value"]["navStatus"]["url"], target);
            break;
        }
    }
    loaded_receiver
        .recv_timeout(Duration::from_secs(3))
        .expect("actual Qt page did not load the owned HTML/title");
    closing.store(true, Ordering::Release);
    owner.stop().unwrap();
    commands.join().unwrap();
    incoming.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = guest.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Qt guest did not exit on private channel close"
        );
        thread::sleep(Duration::from_millis(20));
    }
    status.join().unwrap();
    stopped.store(true, Ordering::Release);
    http.join().unwrap();
}
