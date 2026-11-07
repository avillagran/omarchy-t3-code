//! Actual product Pool/renderer/native Host protocol; private container only.
//! Fixtures are retained. No recursive deletion or real user profiles.
use omarchy_t3_client::browser_guest::{Config, Pool};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
fn receive(receiver: &mpsc::Receiver<Value>, mut accept: impl FnMut(&Value) -> bool) -> Value {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let row = receiver
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .expect("owned browser event deadline");
        if accept(&row) {
            return row;
        }
    }
}
fn cdp(
    pool: &mut Pool,
    events: &mpsc::Receiver<Value>,
    id: u64,
    method: &str,
    params: Value,
    session: Option<&str>,
) -> Value {
    let mut message = json!({"id":id,"method":method,"params":params});
    if let Some(session) = session {
        message["sessionId"] = json!(session)
    }
    pool.private_command(
        "owned",
        "origin",
        &json!({"type":"cdp","threadId":"thread","tabId":"tab","message":message.to_string()}),
    )
    .unwrap();
    let event = receive(events, |row| {
        row["type"] == "cdp"
            && serde_json::from_str::<Value>(row["message"].as_str().unwrap_or(""))
                .is_ok_and(|message| message["id"] == id)
    });
    serde_json::from_str(event["message"].as_str().unwrap()).unwrap()
}
#[test]
fn product_pool_admits_native_guest_and_shares_visible_frames_with_cdp() {
    let root = PathBuf::from(format!("/tmp/native-pool-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let config = Config::isolated(PathBuf::from("/fixture/app/bin/omarchy-t3-client"), &root)
        .unwrap()
        .with_owned_native_electron(
            std::path::Path::new("/electron"),
            std::path::Path::new("/fixture/electron"),
            std::path::Path::new("/usr/bin/Xvfb"),
        )
        .unwrap();
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
    receive(&events, |event| {
        event["type"] == "attached" && event["tabId"] == "tab"
    });
    assert!(pool
        .private_lease("owned", "default", "thread", "tab")
        .is_some());
    let targets = cdp(&mut pool, &events, 1, "Target.getTargets", json!({}), None);
    assert!(targets.get("error").is_none(), "{targets}");
    let target = targets["result"]["targetInfos"][0]["targetId"]
        .as_str()
        .unwrap();
    let attached = cdp(
        &mut pool,
        &events,
        2,
        "Target.attachToTarget",
        json!({"targetId":target,"flatten":true}),
        None,
    );
    let session = attached["result"]["sessionId"].as_str().unwrap();
    let globals = cdp(
        &mut pool,
        &events,
        3,
        "Runtime.evaluate",
        json!({"expression":"[typeof require,typeof process,typeof Buffer]","returnByValue":true}),
        Some(session),
    );
    assert_eq!(
        globals["result"]["result"]["value"],
        json!(["undefined", "undefined", "undefined"])
    );
    let painted = cdp(
        &mut pool,
        &events,
        30,
        "Runtime.evaluate",
        json!({"expression":"document.body.style.backgroundColor='#123456';document.body.style.margin='0';0","returnByValue":true}),
        Some(session),
    );
    assert!(painted.get("error").is_none());
    pool.show(
        "owned",
        "origin",
        &snapshot,
        "surface",
        (640, 480),
        &json!(1),
    )
    .unwrap();
    let frame = receive(&ui, |event| event["kind"] == "frame");
    assert_eq!(frame["requestId"], "surface");
    assert_eq!(frame["width"], 640);
    assert_eq!(frame["height"], 480);
    let file = omarchy_t3_client::Url::parse(frame["url"].as_str().unwrap())
        .unwrap()
        .to_file_path()
        .unwrap();
    let image = image::open(file).unwrap().into_rgba8();
    assert_eq!(
        image.get_pixel(320, 240).0,
        [18, 52, 86, 255],
        "actual guest pixels reach the product frame file"
    );
    let prepared = cdp(
        &mut pool,
        &events,
        40,
        "Runtime.evaluate",
        json!({"expression":r#"document.body.innerHTML="<input id='field' style='position:absolute;left:10px;top:10px;width:200px;height:40px'><button id='button' style='position:absolute;left:250px;top:10px;width:100px;height:40px'>Count</button>";document.body.dataset.clicks='0';document.querySelector('#button').onclick=()=>document.body.dataset.clicks=String(Number(document.body.dataset.clicks)+1);0"#,"returnByValue":true}),
        Some(session),
    );
    assert!(prepared.get("error").is_none());
    assert!(pool
        .input(
            "stale-surface",
            &json!({"type":"press","x":30,"y":30,"button":1,"buttons":1})
        )
        .is_err());
    for event in [
        json!({"type":"press","x":30,"y":30,"button":1,"buttons":1}),
        json!({"type":"release","x":30,"y":30,"button":1,"buttons":0}),
        json!({"type":"key","key":65,"text":"a","modifiers":0}),
        json!({"type":"keyup","key":65,"text":"a","modifiers":0}),
        json!({"type":"text","text":"漢字🧪"}),
        json!({"type":"composition","preedit":"にほん","selectionStart":3,"selectionEnd":3}),
        json!({"type":"text","text":"日本"}),
    ] {
        pool.input("surface", &event).unwrap();
    }
    let mut current = Value::Null;
    for id in 41..90 {
        current = cdp(
            &mut pool,
            &events,
            id,
            "Runtime.evaluate",
            json!({"expression":"document.querySelector('#field').value","returnByValue":true}),
            Some(session),
        );
        if current["result"]["result"]["value"] == "a漢字🧪日本" {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        current["result"]["result"]["value"], "a漢字🧪日本",
        "actual pointer focus, key text, Unicode commit and composition replacement"
    );
    for event in [
        json!({"type":"key","key":65,"text":"a","modifiers":0x04000000}),
        json!({"type":"keyup","key":65,"text":"a","modifiers":0x04000000}),
        json!({"type":"text","text":"replaced"}),
        json!({"type":"press","x":270,"y":30,"button":1,"buttons":1}),
        json!({"type":"release","x":270,"y":30,"button":1,"buttons":0}),
    ] {
        pool.input("surface", &event).unwrap();
    }
    for id in 90..140 {
        current = cdp(
            &mut pool,
            &events,
            id,
            "Runtime.evaluate",
            json!({"expression":"[document.querySelector('#field').value,document.body.dataset.clicks]","returnByValue":true}),
            Some(session),
        );
        if current["result"]["result"]["value"] == json!(["replaced", "1"]) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        current["result"]["result"]["value"],
        json!(["replaced", "1"]),
        "actual control shortcut and one pointer click"
    );
    let foreign = cdp(
        &mut pool,
        &events,
        4,
        "Target.getTargetInfo",
        json!({"targetId":"foreign-target"}),
        Some(session),
    );
    assert!(foreign.get("error").is_some());
    assert_eq!(pool.process_count(), 1);
    let display_children = fs::read_to_string("/proc/thread-self/children")
        .unwrap()
        .split_whitespace()
        .filter_map(|pid| pid.parse::<i32>().ok())
        .filter(|pid| {
            fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|name| name.trim() == "Xvfb")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        display_children.len(),
        1,
        "the actual Pool owns exactly one private display"
    );
    pool.close_profile("owned", "default")
        .expect("native profile must flush with a successful owned exit");
    assert_eq!(pool.process_count(), 0);
    assert!(
        display_children
            .iter()
            .all(|pid| unsafe { libc::kill(*pid, 0) } != 0),
        "the cohort reaps its owned display"
    );
}
