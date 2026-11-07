//! Synthetic actual Pool/engine/GUI clipboard acceptance; private container only.
use omarchy_t3_client::{
    browser_guest::{Config, Pool},
    gui_clipboard,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
fn stage(value: &str) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}
fn receive(events: &mpsc::Receiver<Value>, id: Option<u64>) -> Value {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let value = events
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if id.is_none() && value["type"] == "attached" {
            return value;
        }
        if value["type"] == "cdp" {
            let frame: Value = serde_json::from_str(value["message"].as_str().unwrap()).unwrap();
            if frame["id"].as_u64() == id {
                return frame;
            }
        }
    }
}
fn command(
    pool: &mut Pool,
    events: &mpsc::Receiver<Value>,
    id: &mut u64,
    session: Option<&str>,
    method: &str,
    params: Value,
) -> Value {
    *id += 1;
    let mut frame = json!({"id":id,"method":method,"params":params});
    if let Some(session) = session {
        frame["sessionId"] = json!(session);
    }
    pool.private_command(
        "owned",
        "origin",
        &json!({"type":"cdp","threadId":"thread","tabId":"tab","message":frame.to_string()}),
    )
    .unwrap();
    let result = receive(events, Some(*id));
    assert!(result.get("error").is_none(), "{result}");
    result["result"].clone()
}
fn evaluate(
    pool: &mut Pool,
    events: &mpsc::Receiver<Value>,
    id: &mut u64,
    session: &str,
    expression: &str,
) -> Value {
    let result = command(
        pool,
        events,
        id,
        Some(session),
        "Runtime.evaluate",
        json!({"expression":expression,"returnByValue":true,"awaitPromise":true}),
    );
    assert!(result.get("exceptionDetails").is_none(), "{result}");
    result["result"]["value"].clone()
}
fn main() {
    let gui = Arc::new(Mutex::new(
        gui_clipboard::start().expect("actual GUI parent"),
    ));
    stage("admitted");
    let root = std::path::PathBuf::from(format!("/tmp/native-gui-pool-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let config = Config::isolated(
        std::path::PathBuf::from("/fixture/app/bin/omarchy-t3-client"),
        &root,
    )
    .unwrap()
    .with_owned_native_electron(
        std::path::Path::new("/electron"),
        std::path::Path::new("/fixture/electron"),
        std::path::Path::new("/usr/bin/Xvfb"),
    )
    .unwrap()
    .with_gui_clipboard(Some(gui));
    let (output, ui) = mpsc::channel();
    let (native, events) = mpsc::channel();
    let mut pool = Pool::new(config, move |event| {
        let _ = output.send(event);
    });
    pool.bind_private_source("owned", "origin", move |event| {
        let _ = native.send(event);
    })
    .unwrap();
    let snapshot = json!({"threadId":"thread","tabId":"tab","navStatus":{"_tag":"Idle"},"canGoBack":false,"canGoForward":false,"updatedAt":"","runtime":"server","profileId":"default"});
    pool.ensure_private("owned", "origin", &snapshot, (640, 480), &json!(1))
        .unwrap();
    receive(&events, None);
    let mut id = 0;
    let targets = command(
        &mut pool,
        &events,
        &mut id,
        None,
        "Target.getTargets",
        json!({}),
    );
    let target = targets["targetInfos"][0]["targetId"].as_str().unwrap();
    let attached = command(
        &mut pool,
        &events,
        &mut id,
        None,
        "Target.attachToTarget",
        json!({"targetId":target,"flatten":true}),
    );
    let session = attached["sessionId"].as_str().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));
    let done = stop.clone();
    let server = std::thread::spawn(move || {
        while !done.load(Ordering::SeqCst) {
            if let Ok((mut socket, _)) = listener.accept() {
                socket
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut bytes = [0; 4096];
                let _ = socket.read(&mut bytes);
                let _=socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: 26\r\n\r\n<html><body></body></html>");
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    });
    command(
        &mut pool,
        &events,
        &mut id,
        Some(session),
        "Page.navigate",
        json!({"url":format!("http://127.0.0.1:{port}/")}),
    );
    for _ in 0..200 {
        if evaluate(
            &mut pool,
            &events,
            &mut id,
            session,
            "document.readyState==='complete'&&location.protocol==='http:'",
        ) == true
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        evaluate(
            &mut pool,
            &events,
            &mut id,
            session,
            "[typeof require,typeof process,typeof Buffer]"
        ),
        json!(["undefined", "undefined", "undefined"])
    );
    evaluate(&mut pool,&events,&mut id,session,"document.body.innerHTML=\"<textarea id='target' style='position:fixed;left:10px;top:10px;width:200px;height:60px'></textarea><button id='write' style='position:fixed;left:250px;top:10px;width:150px;height:60px'>Write</button>\";write.onclick=async event=>{window.trusted=event.isTrusted&&navigator.userActivation.isActive;try{await navigator.clipboard.writeText('native product current clipboard');window.writeResult='ok'}catch(error){window.writeResult=error.name}};0");
    pool.show(
        "owned",
        "origin",
        &snapshot,
        "surface",
        (640, 480),
        &json!(1),
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while ui
        .recv_timeout(until.saturating_duration_since(Instant::now()))
        .unwrap()["kind"]
        != "frame"
    {}
    stage("pool-ready");
    let mut input = std::io::stdin().lock().lines();
    assert_eq!(input.next().unwrap().unwrap(), "paste");
    for event in [
        json!({"type":"press","x":30,"y":30,"button":1,"buttons":1}),
        json!({"type":"release","x":30,"y":30,"button":1,"buttons":0}),
        json!({"type":"key","key":86,"text":"v","modifiers":0x04000000}),
        json!({"type":"keyup","key":86,"text":"v","modifiers":0x04000000}),
    ] {
        pool.input("surface", &event).unwrap();
    }
    let mut value = Value::Null;
    for _ in 0..200 {
        value = evaluate(&mut pool, &events, &mut id, session, "target.value");
        if value == "native external GUI offer" {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(value, "native external GUI offer");
    stage("paste-ok");
    assert_eq!(input.next().unwrap().unwrap(), "write");
    for event in [
        json!({"type":"press","x":280,"y":30,"button":1,"buttons":1}),
        json!({"type":"release","x":280,"y":30,"button":1,"buttons":0}),
    ] {
        pool.input("surface", &event).unwrap();
    }
    for _ in 0..200 {
        value = evaluate(
            &mut pool,
            &events,
            &mut id,
            session,
            "[window.writeResult,window.trusted]",
        );
        if value == json!(["ok", true]) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(value, json!(["ok", true]));
    stage("write-ok");
    assert_eq!(input.next().unwrap().unwrap(), "close");
    pool.close_profile("owned", "default").unwrap();
    assert_eq!(pool.process_count(), 0);
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    stage("pool-closed");
    assert_eq!(input.next().unwrap().unwrap(), "quit");
}
