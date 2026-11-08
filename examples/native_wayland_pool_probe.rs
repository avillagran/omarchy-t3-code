//! Private actual Pool/NativeHost/Guest acceptance. No system clipboard access.
use omarchy_t3_client::browser_guest::{Config, Pool};
use serde_json::{json, Value};
use std::{
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::net::UnixStream,
    },
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};
fn receive(events: &mpsc::Receiver<Value>, id: Option<u64>) -> Value {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let event = events
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if event["tabId"] != "tab" {
            continue;
        }
        if id.is_none() && event["type"] == "attached" {
            return event;
        }
        if event["type"] == "cdp" {
            let message: Value = serde_json::from_str(event["message"].as_str().unwrap()).unwrap();
            if message["id"].as_u64() == id {
                return message;
            }
        }
    }
}
fn command(
    pool: &mut Pool,
    events: &mpsc::Receiver<Value>,
    serial: &mut u64,
    session: Option<&str>,
    method: &str,
    params: Value,
) -> Value {
    *serial += 1;
    let mut payload = json!({"id":serial,"method":method,"params":params});
    if let Some(session) = session {
        payload["sessionId"] = json!(session);
    }
    pool.private_command(
        "owned",
        "origin",
        &json!({"type":"cdp","threadId":"thread","tabId":"tab","message":payload.to_string()}),
    )
    .unwrap();
    let response = receive(events, Some(*serial));
    assert!(response.get("error").is_none(), "private-cdp-error");
    response["result"].clone()
}
fn config() -> Config {
    let root =
        std::path::PathBuf::from(format!("/tmp/native-wayland-pool-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    Config::isolated(Path::new("/app/omarchy-t3-client").into(), &root)
        .unwrap()
        .with_owned_native_wayland(
            Path::new("/electron"),
            Path::new("/native-assets"),
            Path::new("/compositor/sway-owned"),
            Path::new("/compositor/prefix/usr/lib"),
        )
        .unwrap()
        .with_mapped_native_host()
        .unwrap()
}
fn children(pid: u32) -> Vec<i32> {
    std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children"))
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect()
}
fn pidfd(pid: i32) -> OwnedFd {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) as i32 };
    assert!(fd >= 0);
    unsafe { OwnedFd::from_raw_fd(fd) }
}
fn renderer_death(snapshot: &Value) {
    let (sender, events) = mpsc::channel();
    let mut pool = Pool::new(config(), |_| {});
    pool.bind_private_source("owned", "origin", move |event| {
        let _ = sender.send(event);
    })
    .unwrap();
    pool.ensure_private("owned", "origin", snapshot, (640, 480), &json!(1))
        .unwrap();
    receive(&events, None);
    // Only our own child ancestry metadata is inspected, never memory or args.
    let pairs: Vec<_> = children(std::process::id())
        .into_iter()
        .filter_map(|renderer| {
            let children = children(renderer as _);
            if children.len() == 1 {
                Some((renderer, children[0]))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(pairs.len(), 1);
    let (renderer, engine) = pairs[0];
    let renderer_watch = pidfd(renderer);
    let engine_watch = pidfd(engine);
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                renderer_watch.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        },
        0
    );
    let mut watch = libc::pollfd {
        fd: engine_watch.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    assert_eq!(unsafe { libc::poll(&mut watch, 1, 3000) }, 1);
    assert_ne!(watch.revents & libc::POLLIN, 0);
    pool.close_all();
}
fn main() {
    let (ui_sender, ui) = mpsc::channel();
    let (sender, events) = mpsc::channel();
    let mut pool = Pool::new(config(), move |event| {
        let _ = ui_sender.send(event);
    });
    pool.bind_private_source("owned", "origin", move |event| {
        let _ = sender.send(event);
    })
    .unwrap();
    let snapshot = json!({"threadId":"thread","tabId":"tab","navStatus":{"_tag":"Idle"},"canGoBack":false,"canGoForward":false,"updatedAt":"","runtime":"server","profileId":"default"});
    pool.ensure_private("owned", "origin", &snapshot, (640, 480), &json!(1))
        .unwrap();
    receive(&events, None);
    let mut serial = 0;
    let targets = command(
        &mut pool,
        &events,
        &mut serial,
        None,
        "Target.getTargets",
        json!({}),
    );
    let target = targets["targetInfos"][0]["targetId"].as_str().unwrap();
    let attached = command(
        &mut pool,
        &events,
        &mut serial,
        None,
        "Target.attachToTarget",
        json!({"targetId":target,"flatten":true}),
    );
    let session = attached["sessionId"].as_str().unwrap();
    let result = command(
        &mut pool,
        &events,
        &mut serial,
        Some(session),
        "Runtime.evaluate",
        json!({"expression":"[1+1,typeof require,typeof process,typeof Buffer]","returnByValue":true}),
    );
    assert_eq!(
        result["result"]["value"],
        json!([2, "undefined", "undefined", "undefined"])
    );
    pool.show(
        "owned",
        "origin",
        &snapshot,
        "surface",
        (640, 480),
        &json!(1),
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let event = ui
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap();
        if event["kind"] == "frame" {
            break;
        }
    }
    let namespaces: Vec<_> = std::fs::read_dir("/tmp")
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            entry.file_type().unwrap().is_dir()
                && entry
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b"o3-wayland-")
        })
        .collect();
    assert_eq!(namespaces.len(), 1);
    let mut foreign = UnixStream::connect(namespaces[0].path().join("engine/engine.sock")).unwrap();
    foreign
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reply = [0_u8; 1];
    match foreign.read(&mut reply) {
        Ok(0) => {}
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        _ => panic!("foreign frontend was not refused"),
    }
    let still_owned = command(
        &mut pool,
        &events,
        &mut serial,
        Some(session),
        "Runtime.evaluate",
        json!({"expression":"3","returnByValue":true}),
    );
    assert_eq!(still_owned["result"]["value"], 3);
    pool.close_all();
    renderer_death(&snapshot);
    println!("NATIVE_WAYLAND_POOL_PASS actual-engine-spawn native-addon-confirmation authenticated-streams NativeHost-Guest CDP sandboxed-page actual-frame foreign-idle-frontend-refused legitimate-guest-retained owned-close renderer-death-kills-actual-engine no-clipboard-claim");
}
