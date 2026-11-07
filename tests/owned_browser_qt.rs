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
    struct Closing(Arc<AtomicBool>);
    impl Drop for Closing {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let _closing_on_drop = Closing(closing.clone());
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
    let completed = |value: &Value| {
        value["_tag"] == "Chunk"
            && value["requestId"] == "events"
            && value["values"].as_array().is_some_and(|events| {
                events.iter().any(|event| {
                    event["snapshot"]["navStatus"]["_tag"] == "Success"
                        && event["snapshot"]["navStatus"]["url"] == target
                })
            })
    };
    let mut authoritative_loaded = false;
    loop {
        let value = frame(&mut socket);
        authoritative_loaded |= completed(&value);
        if value["_tag"] == "Exit" && value["requestId"] == "navigate" {
            assert_eq!(
                value["exit"]["_tag"], "Success",
                "actual navigation failed: {value}"
            );
            assert_eq!(value["exit"]["value"]["tabId"], tab);
            break;
        }
    }
    loaded_receiver
        .recv_timeout(Duration::from_secs(3))
        .expect("actual Qt page did not load the owned HTML/title");
    let settled = Instant::now() + Duration::from_secs(5);
    while !authoritative_loaded {
        assert!(
            Instant::now() < settled,
            "native navigation did not publish actual loaded state"
        );
        authoritative_loaded |= completed(&frame(&mut socket));
    }
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

#[test]
fn actual_product_pool_shares_agent_and_visible_pages_without_background_selection() {
    use omarchy_t3_client::{
        browser_guest::{Config as GuestConfig, Pool},
        Environment,
    };
    use std::sync::{mpsc, Mutex};
    // This path is an immutable copy of the actual Cargo CLI, not a fixture shell.
    assert!(PathBuf::from(env!("CARGO_BIN_EXE_omarchy-t3-client")).is_absolute());
    let root = std::env::temp_dir().join(format!(
        "op-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    ));
    fs::create_dir(&root).unwrap();
    let (owner, browser) = Owner::launch_with_browser(&Config {
        executable: PathBuf::from("/fixture/o3-server"),
        state_root: root.join("state"),
    })
    .unwrap();
    let connection = owner.fresh_connection().unwrap();
    let environment = Environment {
        id: "owned".into(),
        name: "Private owned server".into(),
        base_url: owner.origin().into(),
        token_file: PathBuf::new(),
        write_token_file: None,
        bearer: None,
        connection_error: None,
        connection: Some(Arc::new(connection.clone())),
    };
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
    let initial = omarchy_t3_client::preview_transport::read(&environment, "thread").unwrap();
    let origin = initial["originKey"].as_str().unwrap().to_owned();
    let (frames_sender, frames) = mpsc::channel();
    let (native_sender, native_receiver) = mpsc::sync_channel(4096);
    let mut pool = Pool::new(
        GuestConfig::isolated(
            PathBuf::from("/fixture/app/bin/omarchy-t3-client"),
            &root.join("qt"),
        )
        .unwrap(),
        move |event| {
            frames_sender.send(event).unwrap();
        },
    );
    pool.bind_private_source(&environment.id, &origin, move |event| {
        native_sender.try_send(event).unwrap();
    })
    .unwrap();
    let pool = Arc::new(Mutex::new(pool));
    let closing = Arc::new(AtomicBool::new(false));
    let control_pool = pool.clone();
    let control_origin = origin.clone();
    let control_closing = closing.clone();
    let mut control = browser.control;
    let controls = thread::spawn(move || {
        let mut decoder = ipc::ControlDecoder::default();
        let mut buffer = [0; 65536];
        loop {
            let count = match control.read(&mut buffer) {
                Ok(count) => count,
                Err(error) => {
                    assert!(
                        control_closing.load(Ordering::Acquire),
                        "unexpected control read: {error}"
                    );
                    break;
                }
            };
            if count == 0 {
                break;
            }
            let lines = decoder.feed(&buffer[..count]);
            assert!(!lines.cancelled);
            for line in lines.values {
                if let Some(command) = ipc::decode_command_line(&line) {
                    let result = control_pool.lock().unwrap().private_command(
                        "owned",
                        &control_origin,
                        &command,
                    );
                    assert!(
                        result.is_ok() || control_closing.load(Ordering::Acquire),
                        "native private command failed: {result:?}"
                    );
                }
            }
        }
    });
    let event_closing = closing.clone();
    let mut events = browser.events;
    let native = thread::spawn(move || {
        while !event_closing.load(Ordering::Acquire) {
            match native_receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    let bytes = ipc::encode_event(&event).unwrap();
                    if let Err(error) = events.write_all(&bytes) {
                        assert!(
                            event_closing.load(Ordering::Acquire),
                            "unexpected native event write: {error}"
                        );
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let http_closed = Arc::new(AtomicBool::new(false));
    let stop_http = http_closed.clone();
    let http = thread::spawn(move || {
        while !stop_http.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    socket
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut request = [0; 4096];
                    let count = socket.read(&mut request).unwrap_or(0);
                    let first =
                        String::from_utf8_lossy(&request[..count]).starts_with("GET /first ");
                    let color = if first { "#123456" } else { "#aa2211" };
                    let body=format!("<!doctype html><style>html,body{{margin:0;background:{color}}}</style><title>Private product page</title><body>owned</body>");
                    let _=write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("private HTTP fixture: {error}"),
            }
        }
    });
    let mut tabs = Vec::new();
    for (request, path) in [("first", "first"), ("second", "second")] {
        send(
            &mut socket,
            request,
            "preview.open",
            json!({"threadId":"thread","runtime":"server","url":format!("http://{address}/{path}")}),
        );
        let opened = loop {
            let value = frame(&mut socket);
            assert_ne!(
                value["requestId"], request,
                "open completed without its authoritative guest"
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
        let tab = opened["tabId"].as_str().unwrap().to_owned();
        let admission = omarchy_t3_client::preview_transport::read(&environment, "thread").unwrap();
        assert_eq!(admission["canOperate"], true);
        assert_eq!(admission["result"]["serverEpoch"], opened["serverEpoch"]);
        let mut snapshot = admission["result"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|snapshot| snapshot["tabId"] == tab)
            .unwrap()
            .clone();
        if !snapshot["profileId"].is_string() {
            snapshot["profileId"] = json!("default");
        }
        pool.lock()
            .unwrap()
            .ensure_private("owned", &origin, &snapshot, (640, 480), &json!(1))
            .unwrap();
        loop {
            let value = frame(&mut socket);
            if value["_tag"] == "Exit" && value["requestId"] == request {
                assert_eq!(
                    value["exit"]["_tag"], "Success",
                    "actual product attach failed: {value}"
                );
                break;
            }
        }
        let lease = pool
            .lock()
            .unwrap()
            .private_lease("owned", "default", "thread", &tab)
            .expect("actual product target proof was not admitted");
        if request == "first" {
            pool.lock()
                .unwrap()
                .show(
                    "owned",
                    &origin,
                    &snapshot,
                    "visible-first",
                    (640, 480),
                    &json!(1),
                )
                .unwrap();
        }
        tabs.push((tab, snapshot, lease));
    }
    assert_ne!(tabs[0].2.target(), tabs[1].2.target());
    assert_eq!(
        tabs[0].2.inspector().source.guest,
        tabs[1].2.inspector().source.guest,
        "agent and visible tabs must share the actual profile guest"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut blue = false;
    while Instant::now() < deadline {
        let Ok(event) = frames.recv_timeout(Duration::from_millis(200)) else {
            continue;
        };
        if event["kind"] != "frame" {
            continue;
        }
        assert_eq!(event["requestId"], "visible-first");
        assert_eq!(event["tabId"], tabs[0].0);
        let path = reqwest::Url::parse(event["url"].as_str().unwrap())
            .unwrap()
            .to_file_path()
            .unwrap();
        let output = Command::new("/fixture/qt-image")
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success());
        let image: Value = serde_json::from_slice(&output.stdout).unwrap();
        if image["color"] == "#123456" {
            blue = true;
            break;
        }
    }
    assert!(
        blue,
        "background agent page replaced the selected page's actual pixels"
    );
    let before = pool
        .lock()
        .unwrap()
        .private_lease("owned", "default", "thread", &tabs[0].0)
        .unwrap();
    pool.lock().unwrap().hide();
    pool.lock()
        .unwrap()
        .show(
            "owned",
            &origin,
            &tabs[0].1,
            "visible-again",
            (640, 480),
            &json!(1),
        )
        .unwrap();
    let after = pool
        .lock()
        .unwrap()
        .private_lease("owned", "default", "thread", &tabs[0].0)
        .unwrap();
    assert_eq!(before.id(), after.id());
    assert_eq!(before.target(), after.target());
    pool.lock().unwrap().close_all();
    closing.store(true, Ordering::Release);
    owner.stop().unwrap();
    controls.join().unwrap();
    native.join().unwrap();
    http_closed.store(true, Ordering::Release);
    http.join().unwrap();
}

#[test]
fn actual_runtime_registers_global_observer_and_drives_the_product_manager() {
    use omarchy_t3_client::{
        browser_guest, browser_sessions, owned_runtime, preview_transport, protocol,
    };
    let root = std::env::temp_dir().join(format!(
        "or-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    ));
    fs::create_dir(&root).unwrap();
    let (events, output) = std::sync::mpsc::channel();
    let manager = browser_sessions::Manager::start_with_removal(
        browser_guest::Config::isolated(
            PathBuf::from("/fixture/app/bin/omarchy-t3-client"),
            &root.join("qt"),
        )
        .unwrap(),
        |id| {
            owned_runtime::current_environment()?
                .filter(|environment| environment.id == id)
                .ok_or("unknown_environment".into())
        },
        || Ok(json!({})),
        move |event| {
            let _ = events.send(event);
        },
        |_path| panic!("This proof must not remove a profile tree"),
    );
    let mut runtime = owned_runtime::Runtime::launch(
        &Config {
            executable: PathBuf::from("/fixture/o3-server"),
            state_root: root.join("state"),
        },
        manager.clone(),
    )
    .unwrap();
    let environment = owned_runtime::current_environment().unwrap().unwrap();
    let connection = environment.connection(true).unwrap();
    // No test-side observer or hand-written FD adapter: the actual Runtime must
    // receive opened before the server can complete this native open call.
    let opened = connection
        .rpc(
            "preview.open",
            json!({"threadId":"controller-thread","runtime":"server"}),
        )
        .unwrap();
    let tab = opened["tabId"]
        .as_str()
        .or_else(|| opened["snapshot"]["tabId"].as_str())
        .unwrap();
    let current = preview_transport::read(&environment, "controller-thread").unwrap();
    assert!(current["result"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|session| session["tabId"] == tab));
    manager.request(json!({"op":"browser-show","environment":environment.id,"thread":"controller-thread","tabId":tab,"originKey":current["originKey"],"serverEpoch":current["result"]["serverEpoch"],"requestId":"controller-visible","width":600,"height":400}), true);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(
            Instant::now() < deadline,
            "Actual manager did not show the controller-owned page"
        );
        let Ok(event) = output.recv_timeout(Duration::from_millis(100)) else {
            continue;
        };
        assert_ne!(event["kind"], "error", "Manager error: {event}");
        if event["kind"] == "shown" {
            break;
        }
    }
    protocol::api::preview::close(&connection, "controller-thread", Some(tab)).unwrap();
    runtime.stop().unwrap();
    assert!(owned_runtime::current_environment().unwrap().is_none());
    // Releasing an already stopped runtime cannot unbind its successor.
    let mut successor = owned_runtime::Runtime::launch(
        &Config {
            executable: PathBuf::from("/fixture/o3-server"),
            state_root: root.join("next"),
        },
        manager.clone(),
    )
    .unwrap();
    drop(runtime);
    let current = owned_runtime::current_environment().unwrap().unwrap();
    assert!(current
        .connection(true)
        .unwrap()
        .rpc("server.getConfig", json!({}))
        .is_ok());
    successor.stop().unwrap();
    manager.shutdown().unwrap();
}
