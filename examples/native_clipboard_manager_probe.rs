//! Actual owned server/Manager/native permission consumer, synthetic websites only.
use omarchy_t3_client::{
    browser_guest, browser_sessions, gui_clipboard, owned_backend, owned_runtime,
    preview_transport, protocol,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};
fn output(value: Value) {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{value}").unwrap();
    stdout.flush().unwrap();
}
fn stage(value: &str) {
    output(json!({"type":"stage","stage":value}));
}
fn main() {
    let gui = Arc::new(Mutex::new(
        gui_clipboard::start().expect("actual GUI registration"),
    ));
    stage("admitted");
    let root = std::env::temp_dir().join(format!(
        "nm-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    ));
    std::fs::create_dir(&root).unwrap();
    let mut config = browser_guest::Config::isolated(
        PathBuf::from("/fixture/app/bin/omarchy-t3-client"),
        &root.join("browser"),
    )
    .unwrap()
    .with_owned_native_electron(
        std::path::Path::new("/electron"),
        std::path::Path::new("/fixture/electron"),
        std::path::Path::new("/usr/bin/Xvfb"),
    )
    .unwrap()
    .with_gui_clipboard(Some(gui));
    let exercise_ime = std::env::var_os("O3_NATIVE_GUI_IME").is_some();
    if exercise_ime {
        config = config.with_mapped_native_host().unwrap();
    }
    let removals = Arc::new(AtomicUsize::new(0));
    let attempted = removals.clone();
    let shown = Arc::new(AtomicBool::new(false));
    let visible = shown.clone();
    let manager = browser_sessions::Manager::start_with_removal(
        config,
        |id| {
            owned_runtime::current_environment()?
                .filter(|env| env.id == id)
                .ok_or("unknown_environment".into())
        },
        || Ok(json!({})),
        move |event| {
            if event["kind"] == "shown" && event["requestId"] == "manager-visible" {
                visible.store(true, Ordering::SeqCst);
            }
            output(json!({"type":"browser-surface","data":event}));
        },
        move |_| {
            attempted.fetch_add(1, Ordering::SeqCst);
            Err(std::io::Error::other(
                "unexpected storage removal in read-permission proof",
            ))
        },
    );
    let (commands, controls) = mpsc::channel();
    let input_manager = manager.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let row: Value = serde_json::from_str(&line.unwrap()).unwrap();
            if row["op"] == "browser-permission" {
                input_manager.request(row, false);
            } else if row["op"] == "browser-input" {
                input_manager.request(row, true);
            } else if commands.send(row).is_err() {
                break;
            }
        }
    });
    let mut runtime = owned_runtime::Runtime::launch(
        &owned_backend::Config {
            executable: PathBuf::from("/server/o3-server"),
            state_root: root.join("s"),
        },
        manager.clone(),
    )
    .unwrap();
    let environment = owned_runtime::current_environment().unwrap().unwrap();
    let connection = environment.connection(true).unwrap();
    let opened = protocol::api::preview::open(
        &connection,
        "controller-thread",
        &protocol::api::preview::PreviewOpen {
            runtime: Some("server".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let tab = opened["tabId"]
        .as_str()
        .or_else(|| opened["snapshot"]["tabId"].as_str())
        .unwrap()
        .to_owned();
    let current = preview_transport::read(&environment, "controller-thread").unwrap();
    assert_ne!(
        current["result"]["headless"], true,
        "only the adopted desktop path is admitted"
    );
    manager.request(json!({"op":"browser-show","environment":environment.id,"thread":"controller-thread","tabId":tab,"originKey":current["originKey"],"serverEpoch":current["result"]["serverEpoch"],"requestId":"manager-visible","width":640,"height":480}),true);
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while !shown.load(Ordering::SeqCst) {
        assert!(
            std::time::Instant::now() < deadline,
            "actual Manager must show the adopted native page"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let (reports, results) = mpsc::channel();
    let ime_page =
        exercise_ime.then(|| std::fs::read_to_string("/fixture/native-ime-page.html").unwrap());
    let http = std::thread::spawn(move || {
        while !stopped.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    socket
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut data = Vec::new();
                    let mut header = None;
                    let mut length = 0;
                    loop {
                        let mut block = [0; 4096];
                        match socket.read(&mut block) {
                            Ok(0) | Err(_) => break,
                            Ok(count) => data.extend_from_slice(&block[..count]),
                        }
                        if data.len() > 65536 {
                            break;
                        }
                        if header.is_none() {
                            if let Some(end) =
                                data.windows(4).position(|value| value == b"\r\n\r\n")
                            {
                                let text = String::from_utf8_lossy(&data[..end]);
                                length = text
                                    .lines()
                                    .find_map(|line| {
                                        line.split_once(':')
                                            .filter(|(key, _)| {
                                                key.eq_ignore_ascii_case("content-length")
                                            })
                                            .and_then(|(_, value)| {
                                                value.trim().parse::<usize>().ok()
                                            })
                                    })
                                    .unwrap_or(0);
                                if length > 32768 {
                                    break;
                                }
                                header = Some(end + 4);
                            }
                        }
                        if header.is_some_and(|end| data.len() >= end + length) {
                            break;
                        }
                    }
                    let post = data.starts_with(b"POST /result ");
                    if post {
                        if let Some(end) = header {
                            if data.len() >= end + length {
                                if let Ok(value) =
                                    serde_json::from_slice::<Value>(&data[end..end + length])
                                {
                                    let _ = reports.send(value);
                                }
                            }
                        }
                    }
                    let body = if post {
                        "ok"
                    } else if data.starts_with(b"GET /read-4 ") {
                        r#"<html><body><script>setTimeout(async()=>{let result;try{await navigator.clipboard.write([new ClipboardItem({'web application/x-owned-custom':new Blob([new Uint8Array([0,1,2,127,128,255])],{type:'application/x-owned-custom'}),'text/plain':new Blob(['owned custom fallback'],{type:'text/plain'})})]);result={outcome:'allowed',text:'custom-written'}}catch(error){result={outcome:'denied',error:error.name}};fetch('/result',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(result)})},300)</script></body></html>"#
                    } else if data.starts_with(b"GET /read-6 ") {
                        ime_page.as_deref().unwrap()
                    } else if data.starts_with(b"GET /read-5 ") {
                        r#"<html><body><script>setTimeout(async()=>{let result;try{const items=await navigator.clipboard.read();let body;for(const item of items)if(item.types.includes('web application/x-owned-custom'))body=await item.getType('web application/x-owned-custom');if(!body)throw Error('Missing custom type');const bytes=new Uint8Array(await body.arrayBuffer());const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))).map(value=>value.toString(16).padStart(2,'0')).join('');result={outcome:'allowed',text:String(bytes.length)+':'+digest}}catch(error){result={outcome:'denied',error:error.name}};fetch('/result',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(result)})},300)</script></body></html>"#
                    } else {
                        r#"<html><body><script>setTimeout(async()=>{let result;try{const text=await navigator.clipboard.readText();result={outcome:'allowed',text}}catch(error){result={outcome:'denied',error:error.name}};fetch('/result',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(result)})},300)</script></body></html>"#
                    };
                    let _=write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(_) => break,
            }
        }
    });
    stage("owner-ready");
    let mut phases = vec![
        ("start", "127.0.0.1", false, "", "blocked-one"),
        ("next-block", "127.0.0.1", false, "", "blocked-cached"),
        (
            "next-allow",
            "localhost",
            true,
            "native permission allowed one",
            "allowed-one",
        ),
        (
            "next-fresh",
            "localhost",
            true,
            "native permission allowed two",
            "allowed-cached",
        ),
        (
            "next-custom",
            "localhost",
            true,
            "custom-written",
            "custom-written",
        ),
        (
            "read-custom",
            "localhost",
            true,
            "200000:1bd168fa4e29a8a1af90173db63749b296c7f417c23487dd03ebf21d6ed663a6",
            "custom-read",
        ),
    ];
    if exercise_ime {
        phases.push(("next-ime", "localhost", true, "日本", "ime-complete"));
    }
    for (index, (instruction, host, allowed, expected, done)) in phases.into_iter().enumerate() {
        let command = controls.recv_timeout(Duration::from_secs(30)).unwrap();
        assert_eq!(command["op"], instruction);
        let connection = owned_runtime::current_environment()
            .unwrap()
            .unwrap()
            .connection(true)
            .unwrap();
        protocol::api::preview::navigate(
            &connection,
            "controller-thread",
            &tab,
            &format!("http://{host}:{port}/read-{index}"),
            None,
        )
        .unwrap();
        if instruction == "next-ime" {
            let ready = results.recv_timeout(Duration::from_secs(25)).unwrap();
            assert_eq!(ready["outcome"], "ime-page-ready");
            stage("ime-ready");
        }
        let report = loop {
            let report = results.recv_timeout(Duration::from_secs(25)).unwrap();
            if instruction == "next-ime" && report["outcome"] == "ime-observation" {
                output(
                    json!({"type":"ime-observation","focus":report["focus"],"target":report["target"],"x":report["x"],"y":report["y"],"bounds":report["bounds"],"valueUnits":report["text"].as_str().map(|text|text.encode_utf16().count()),"events":report["events"]}),
                );
                continue;
            }
            break report;
        };
        assert_eq!(
            report["outcome"],
            if allowed { "allowed" } else { "denied" }
        );
        if allowed {
            assert_eq!(report["text"], expected);
        } else {
            assert_eq!(report["error"], "NotAllowedError");
        }
        if instruction == "next-ime" {
            let events = report["events"].as_array().unwrap();
            assert!(events
                .iter()
                .any(|event| event["type"] == "compositionstart" && event["trusted"] == true));
            assert!(events.iter().any(|event| event["type"] == "compositionend"));
        }
        stage(done);
    }
    assert_eq!(
        controls.recv_timeout(Duration::from_secs(30)).unwrap()["op"],
        "stop"
    );
    runtime.stop().unwrap();
    manager.shutdown().unwrap();
    stop.store(true, Ordering::SeqCst);
    http.join().unwrap();
    assert_eq!(removals.load(Ordering::SeqCst), 0);
    stage("complete");
}
