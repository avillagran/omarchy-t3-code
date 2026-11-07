use omarchy_t3_client::{environments, file_metadata_runtime, rpc_sessions, thread_details};
use serde_json::{json, Value};
use std::{collections::HashSet, fs, os::unix::fs::PermissionsExt, sync::atomic::Ordering};
#[path = "support/file_metadata_peer.rs"]
mod peer;
#[test]
fn real_terminal_consumers_share_admitted_metadata_and_keep_editor_authorization_separate() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let root = std::env::temp_dir().join(format!("terminal-metadata-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let token = root.join("token");
    fs::write(&token, "synthetic-terminal-only").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let peer = peer::Peer::new();
    let config = root.join("environments.json");
    fs::write(&config,json!({"environments":[{"id":"fixture","base_url":peer.base,"token_file":token,"write_token_file":token}]}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", config);
    let environment = environments().unwrap().remove(0);
    let runtime = file_metadata_runtime::global();
    omarchy_t3_client::write::execute(&json!({"op":"file-metadata-retain","environment":"fixture","thread":"thread","leaseId":"lease"})).unwrap();
    peer.operate.store(false, Ordering::Release);
    let command = json!({"op":"thread-details-read","environment":"fixture","thread":"thread","target":"/work/main.rs:12:7","cwd":"/work","path":"/work/main.rs","leaseId":"lease","owner":"surface","editor":"zed"});
    let first = omarchy_t3_client::write::execute(&command).unwrap();
    assert_eq!(first["terminalLink"]["path"], "/work/main.rs");
    assert_eq!(first["terminalLink"]["kind"], "file");
    assert_eq!(
        thread_details::open(&environment, &command).unwrap_err(),
        "permission_denied"
    );
    assert!(!peer
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|call| call["tag"] == "shell.openInEditor"));
    peer.operate.store(true, Ordering::Release);
    let before = peer.metadata_calls().len();
    let second = thread_details::read(&environment, &command).unwrap();
    assert_eq!(
        first["terminalLink"]["sessionId"],
        second["terminalLink"]["sessionId"]
    );
    assert_eq!(
        peer.metadata_calls().len(),
        before,
        "real repeated hover uses retained socket cache"
    );
    let opened = thread_details::open(&environment, &command).unwrap();
    assert_eq!(opened["target"], command["target"]);
    assert_eq!(
        peer.metadata_calls().len(),
        before,
        "click uses same native cached query"
    );
    let calls = peer.calls.lock().unwrap();
    let open = calls
        .iter()
        .rev()
        .find(|call| call["tag"] == "shell.openInEditor")
        .unwrap();
    assert_eq!(
        open["payload"],
        json!({"cwd":"/work/main.rs:12:7","editor":"zed"})
    );
    drop(calls);
    let before = peer.metadata_calls().len();
    let document = omarchy_t3_client::documents::read(
        &environment,
        &json!({"thread":"thread","cwd":"/work","path":"read-through-document.ts"}),
    )
    .unwrap();
    assert_eq!(document["contents"], "fixture");
    let mut seeded_command = command.clone();
    seeded_command["target"] = json!("/work/read-through-document.ts");
    seeded_command["path"] = json!("/work/read-through-document.ts");
    assert_eq!(
        thread_details::read(&environment, &seeded_command).unwrap()["terminalLink"]["kind"],
        "file"
    );
    assert_eq!(
        peer.metadata_calls().len(),
        before,
        "actual workspace document read seeds terminal metadata on same admitted socket"
    );
    let suggestions = omarchy_t3_client::suggestions::query(
        &environment,
        &json!({"thread":"thread","kind":"path","query":"seed"}),
    )
    .unwrap();
    assert_eq!(suggestions["items"][0]["label"], "seeded.ts");
    seeded_command["target"] = json!("/work/seeded.ts");
    seeded_command["path"] = json!("/work/seeded.ts");
    assert_eq!(
        thread_details::read(&environment, &seeded_command).unwrap()["terminalLink"]["kind"],
        "file"
    );
    assert_eq!(
        peer.metadata_calls().len(),
        before,
        "actual composer search seeds terminal metadata without a redundant stat"
    );
    let fixture: Value =
        serde_json::from_str(include_str!("../native/terminal/link-fixtures.json")).unwrap();
    assert_eq!(fixture["metadataCases"].as_array().unwrap().len(), 22);
    let mut known = HashSet::new();
    known.insert("/work/main.rs".to_owned());
    for case in fixture["metadataCases"].as_array().unwrap() {
        let before = peer.metadata_calls().len();
        let result = thread_details::read(
            &environment,
            &json!({"thread":"thread","target":case["target"],"leaseId":"lease","owner":"surface"}),
        )
        .unwrap();
        assert_eq!(result["terminalLink"]["path"], case["expected"], "{case}");
        if let Some(path) = case["expected"].as_str() {
            assert_eq!(
                peer.metadata_calls().len(),
                before + usize::from(known.insert(path.into())),
                "admitted paths use native cache or exactly one stat: {case}"
            )
        } else {
            assert_eq!(
                peer.metadata_calls().len(),
                before,
                "rejected network/relative/oversized path never stats"
            )
        }
    }
    peer.present.store(false, Ordering::Release);
    runtime
        .invalidate("fixture", &["/work/main.rs".into()])
        .unwrap();
    let before = peer
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| call["tag"] == "shell.openInEditor")
        .count();
    assert_eq!(
        thread_details::open(&environment, &command).unwrap_err(),
        "terminal_link_unavailable"
    );
    assert_eq!(
        peer.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call["tag"] == "shell.openInEditor")
            .count(),
        before,
        "actual invalidation fences editor opening after disappearance"
    );
    peer.present.store(true, Ordering::Release);
    let miss = thread_details::read(&environment, &command).unwrap();
    assert!(
        miss["terminalLink"]["kind"].is_null(),
        "native negative remains cached until invalidation/TTL"
    );
    runtime
        .invalidate("fixture", &["/work/main.rs".into()])
        .unwrap();
    assert_eq!(
        thread_details::read(&environment, &command).unwrap()["terminalLink"]["kind"],
        "file"
    );
    peer.allow.store(false, Ordering::Release);
    let before = peer.metadata_calls().len();
    assert_eq!(
        thread_details::read(&environment, &command).unwrap_err(),
        "permission_denied"
    );
    assert_eq!(
        peer.metadata_calls().len(),
        before,
        "scope denial precedes even retained metadata access"
    );
    peer.allow.store(true, Ordering::Release);
    peer.hold.store(true, Ordering::Release);
    let mut pending = command.clone();
    pending["target"] = json!("/work/pending");
    pending["path"] = json!("/work/pending");
    let read = std::thread::spawn(move || omarchy_t3_client::write::execute(&pending));
    peer.wait(|calls| {
        calls.iter().any(|call| {
            call["tag"] == "filesystem.getMetadata"
                && call["payload"]["paths"] == json!(["/work/pending"])
        })
    });
    omarchy_t3_client::write::execute(&json!({"op":"file-metadata-release","leaseId":"lease"}))
        .unwrap();
    assert_eq!(
        read.join().unwrap().unwrap_err(),
        "metadata_cancelled",
        "actual release dispatch cancels pending real read"
    );
    peer.wait(|calls| calls.iter().any(|call| call["_tag"] == "Interrupt"));
    peer.hold.store(false, Ordering::Release);
    rpc_sessions::close_all();
}
