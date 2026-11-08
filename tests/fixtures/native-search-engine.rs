//! Exercise the actual compiled application owner against its admitted Electron.
use omarchy_t3_client::{
    native_search::{Engine, Manager},
    voice_capture::resources::Runtime,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

fn children() -> BTreeSet<u32> {
    fs::read_dir("/proc/self/task")
        .unwrap()
        .flat_map(|entry| {
            fs::read_to_string(entry.unwrap().path().join("children"))
                .unwrap()
                .split_whitespace()
                .map(|word| word.parse::<u32>().unwrap())
                .collect::<Vec<_>>()
        })
        .collect()
}
fn rows(title: &str) -> Value {
    json!([{"key":"private/thread","thread":{"id":"thread","environmentId":"private",
        "title":title,"createdAt":"2026-10-01T00:00:00.000Z","updatedAt":"2026-10-02T00:00:00.000Z"}}])
}
fn request(action: &str, id: &str) -> Value {
    json!({"op":"native-search","action":action,"requestId":id,
        "payloadJson":json!({"rows":rows("Café auth"),"query":"auth","key":"private/thread",
            "activeGroups":[],"threadSearchItems":[]}).to_string()})
}
fn run() -> Result<(), String> {
    assert!(Path::new("/run/.containerenv").is_file());
    assert!(children().is_empty());
    let runtime =
        Arc::new(Runtime::admit(Path::new("/electron")).map_err(|error| error.to_string())?);
    let mut engine = Engine::launch(runtime.clone())?;
    assert_eq!(children().len(), 1);
    assert_eq!(
        engine.call(json!({"op":"snapshot","rows":rows("Café auth")}))?,
        json!({"updated":true})
    );
    let item = json!({"value":"thread:private/thread","threadKey":"private/thread",
        "title":"Café auth","searchTerms":["Café auth"]});
    let ranked = engine.call(
        json!({"op":"palette","query":"cafe","activeGroups":[],"threadSearchItems":[item]}),
    )?;
    assert_eq!(
        ranked["groups"][0]["items"][0]["value"],
        "thread:private/thread"
    );
    assert!(engine.call(json!({"op":"snapshot","rows":{}})).is_err());
    assert_eq!(
        engine.call(json!({"op":"snapshot","rows":rows("Café auth")}))?,
        json!({"updated":true})
    );
    let packed = engine.call(json!({"op":"palette","payloadJson":r#"{"query":"x\udc00","activeGroups":[],"threadSearchItems":[],"messageMatches":[],"messageQuery":"x\udc00"}"#}))?;
    assert!(packed["resultJson"]
        .as_str()
        .unwrap()
        .contains(r#""query":"x\udc00""#));
    let title = "Native search ".repeat(100_000);
    assert_eq!(
        engine.call(json!({"op":"snapshot","rows":rows(&title)}))?,
        json!({"updated":true})
    );
    engine.close()?;
    assert!(children().is_empty());

    let (tx, rx) = mpsc::channel();
    let mut manager = Manager::new(tx)?;
    for index in 0..50 {
        manager.submit(request("palette", &format!("rank-{index}")))?;
    }
    manager.submit(request("open", "open-a"))?;
    manager.submit(request("open", "open-b"))?;
    let mut received = BTreeSet::new();
    let mut opens = Vec::new();
    for _ in 0..52 {
        let event = rx
            .recv_timeout(Duration::from_secs(20))
            .map_err(|error| error.to_string())?;
        let id = event["requestId"].as_str().unwrap();
        assert!(
            received.insert(id.to_owned()),
            "A waiter received duplicate terminal replies"
        );
        if id.starts_with("open-") {
            assert!(event["error"].is_null());
            let result: Value =
                serde_json::from_str(event["result"]["resultJson"].as_str().unwrap()).unwrap();
            opens.push((id.to_owned(), result["send"].as_bool().unwrap()));
        } else if !event["error"].is_null() {
            assert_eq!(event["error"], "native_search_superseded");
        }
    }
    assert_eq!(
        opens,
        [("open-a".to_owned(), true), ("open-b".to_owned(), false)]
    );
    manager.close()?;
    assert!(children().is_empty());

    let mut stopped = Engine::launch(runtime)?;
    let owned = children();
    assert_eq!(owned.len(), 1);
    let pid = *owned.first().unwrap();
    assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGSTOP) }, 0);
    let mut status = 0;
    assert_eq!(
        unsafe { libc::waitpid(pid as i32, &mut status, libc::WUNTRACED) },
        pid as i32
    );
    assert!(libc::WIFSTOPPED(status));
    let started = Instant::now();
    let failure = stopped.call(json!({"op":"snapshot","rows":rows(&title)}));
    assert_eq!(failure.unwrap_err(), "native_search_timeout");
    assert!(started.elapsed() >= Duration::from_millis(9500));
    assert!(started.elapsed() < Duration::from_secs(13));
    assert!(
        stopped.close().is_err(),
        "Forced failure must remain observable"
    );
    assert!(
        children().is_empty(),
        "The exact stopped child must be killed and reaped"
    );
    println!("NATIVE_SEARCH_ENGINE_PASS {{\"cases\":8,\"rankWaiters\":50,\"orderedOpens\":2,\"largePipeBytes\":{},\"timeoutChildReaped\":true,\"actualRootLibrary\":true}}", title.len());
    Ok(())
}
fn main() {
    run().unwrap();
}
