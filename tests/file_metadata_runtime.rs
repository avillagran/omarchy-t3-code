use omarchy_t3_client::{
    file_metadata_cache::ThreadRef, file_metadata_runtime::Runtime, rpc_sessions,
};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
#[path = "support/file_metadata_peer.rs"]
mod peer;

#[test]
fn actual_socket_batches_retains_seeds_retries_and_cancels_scoped_waiters() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let root = std::env::temp_dir().join(format!("metadata-runtime-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let token = root.join("token");
    fs::write(&token, "synthetic-only").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let peer = peer::Peer::new();
    peer.operate.store(false, Ordering::Release);
    let config = root.join("environments.json");
    fs::write(&config,json!({"environments":[{"id":"fixture","base_url":peer.base,"token_file":token,"write_token_file":token}]}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", &config);
    let now = Arc::new(AtomicU64::new(0));
    let clock = now.clone();
    let runtime = Arc::new(Runtime::new(Arc::new(move || {
        clock.load(Ordering::Acquire)
    })));
    runtime
        .retain(
            "lease",
            ThreadRef {
                environment: "fixture".into(),
                thread: "thread".into(),
            },
        )
        .unwrap();
    let paths = (0..130)
        .map(|n| format!("/work/file-{n}"))
        .collect::<Vec<_>>();
    let queries = runtime
        .query("fixture", &paths, Some("lease"), "pane")
        .unwrap();
    for query in queries {
        assert_eq!(query.wait().unwrap().value.unwrap().kind, "file")
    }
    let calls = peer.metadata_calls();
    assert_eq!(
        calls
            .iter()
            .map(|call| call["payload"]["paths"].as_array().unwrap().len())
            .sum::<usize>(),
        130
    );
    assert!(calls
        .iter()
        .all(|call| call["payload"]["paths"].as_array().unwrap().len() <= 64));
    peer.present.store(false, Ordering::Release);
    let mut created = runtime
        .query(
            "fixture",
            &["/work/created.ts".into()],
            Some("lease"),
            "pane",
        )
        .unwrap();
    assert!(created.remove(0).wait().unwrap().value.is_none());
    peer.present.store(true, Ordering::Release);
    let before = peer.metadata_calls().len();
    let mut created = runtime
        .query(
            "fixture",
            &["/work/created.ts".into()],
            Some("lease"),
            "pane",
        )
        .unwrap();
    assert!(created.remove(0).wait().unwrap().value.is_none());
    assert_eq!(
        peer.metadata_calls().len(),
        before,
        "creation does not bypass native negative-cache lifetime"
    );
    runtime
        .invalidate("fixture", &["/work/created.ts".into()])
        .unwrap();
    let mut created = runtime
        .query(
            "fixture",
            &["/work/created.ts".into()],
            Some("lease"),
            "pane",
        )
        .unwrap();
    assert!(created.remove(0).wait().unwrap().value.is_some());
    let one = rpc_sessions::session("fixture", false).unwrap();
    let mut queries = runtime
        .query("fixture", &["/work/missing".into()], Some("lease"), "pane")
        .unwrap();
    assert!(queries.remove(0).wait().unwrap().value.is_none());
    let before = peer.metadata_calls().len();
    now.store(60_001, Ordering::Release);
    let mut queries = runtime
        .query(
            "fixture",
            &[paths[0].clone(), "/work/missing".into()],
            Some("lease"),
            "pane",
        )
        .unwrap();
    assert!(queries.remove(0).wait().unwrap().value.is_some());
    assert!(queries.remove(0).wait().unwrap().value.is_none());
    assert_eq!(
        peer.metadata_calls().len(),
        before + 1,
        "mounted positive survives TTL; miss retries"
    );
    assert_eq!(
        rpc_sessions::session("fixture", false).unwrap().id(),
        one.id(),
        "all metadata uses one actual admitted socket"
    );

    let function = &omarchy_t3_client::protocol::api::projects::SEARCH_ENTRIES;
    runtime
        .seeded_read(
            "fixture",
            function,
            &json!({"cwd":"/work","query":"seed","limit":40}),
            "/work",
            false,
        )
        .unwrap();
    let before = peer.metadata_calls().len();
    let mut queries = runtime
        .query(
            "fixture",
            &["/work/seeded.ts".into()],
            Some("lease"),
            "pane",
        )
        .unwrap();
    assert_eq!(
        queries.remove(0).wait().unwrap().value.unwrap().kind,
        "file"
    );
    assert_eq!(
        peer.metadata_calls().len(),
        before,
        "real project read seeds same actual socket"
    );

    peer.hold.store(true, Ordering::Release);
    let seeded = runtime.clone();
    let pending_read = std::thread::spawn(move || {
        seeded.seeded_read(
            "fixture",
            &omarchy_t3_client::protocol::api::projects::READ_FILE,
            &json!({"cwd":"/work","relativePath":"read-raced.ts"}),
            "/work",
            true,
        )
    });
    peer.wait(|calls| calls.iter().any(|call| call["tag"] == "projects.readFile"));
    runtime
        .invalidate("fixture", &["/work/unrelated".into()])
        .unwrap();
    peer.hold.store(false, Ordering::Release);
    pending_read.join().unwrap().unwrap();
    let before = peer.metadata_calls().len();
    let mut queries = runtime
        .query(
            "fixture",
            &["/work/read-raced.ts".into()],
            Some("lease"),
            "pane",
        )
        .unwrap();
    queries.remove(0).wait().unwrap();
    assert_eq!(
        peer.metadata_calls().len(),
        before + 1,
        "intervening invalidation fences real file-read seeding"
    );

    peer.hold.store(true, Ordering::Release);
    let before = peer.metadata_calls().len();
    let mut queries = runtime
        .query(
            "fixture",
            &["/work/dirty".into(), "/work/clean".into()],
            Some("lease"),
            "pane",
        )
        .unwrap();
    peer.wait(|calls| {
        calls
            .iter()
            .filter(|call| call["tag"] == "filesystem.getMetadata")
            .count()
            > before
    });
    runtime
        .invalidate("fixture", &["/work/dirty".into()])
        .unwrap();
    peer.hold.store(false, Ordering::Release);
    assert!(queries.remove(0).wait().unwrap().value.is_some());
    assert!(queries.remove(0).wait().unwrap().value.is_some());
    let calls = peer.metadata_calls();
    assert_eq!(calls.len(), before + 2);
    assert_eq!(
        calls.last().unwrap()["payload"]["paths"],
        json!(["/work/dirty"]),
        "only invalidated path retries"
    );

    peer.hold.store(true, Ordering::Release);
    let before = peer.metadata_calls().len();
    let mut distinct = runtime
        .query(
            "fixture",
            &["/work/cancel-one".into(), "/work/keep-one".into()],
            Some("lease"),
            "pane",
        )
        .unwrap();
    peer.wait(|calls| {
        calls
            .iter()
            .filter(|call| call["tag"] == "filesystem.getMetadata")
            .count()
            > before
    });
    let first = distinct.remove(0);
    first.cancel();
    assert_eq!(first.wait().err().unwrap(), "metadata_cancelled");
    assert!(
        !peer
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call["_tag"] == "Interrupt"),
        "one cancelled path cannot interrupt its batch peer"
    );
    peer.hold.store(false, Ordering::Release);
    assert!(distinct.remove(0).wait().unwrap().value.is_some());

    peer.hold.store(true, Ordering::Release);
    let before = peer.metadata_calls().len();
    let mut queries = runtime
        .query(
            "fixture",
            &["/work/shared".into(), "/work/shared".into()],
            Some("lease"),
            "pane",
        )
        .unwrap();
    peer.wait(|calls| {
        calls
            .iter()
            .filter(|call| call["tag"] == "filesystem.getMetadata")
            .count()
            > before
    });
    let first = queries.remove(0);
    first.cancel();
    assert_eq!(first.wait().err().unwrap(), "metadata_cancelled");
    assert!(
        !peer
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call["_tag"] == "Interrupt"),
        "one cancelled waiter cannot interrupt another waiter"
    );
    peer.hold.store(false, Ordering::Release);
    assert!(queries.remove(0).wait().unwrap().value.is_some());
    peer.hold.store(true, Ordering::Release);
    let before = peer.metadata_calls().len();
    let mut queries = runtime
        .query(
            "fixture",
            &["/work/cancel-all".into()],
            Some("lease"),
            "retiring-pane",
        )
        .unwrap();
    peer.wait(|calls| {
        calls
            .iter()
            .filter(|call| call["tag"] == "filesystem.getMetadata")
            .count()
            > before
    });
    runtime.cancel_owner("retiring-pane");
    assert_eq!(
        queries.remove(0).wait().err().unwrap(),
        "metadata_cancelled"
    );
    peer.wait(|calls| calls.iter().any(|call| call["_tag"] == "Interrupt"));
    peer.hold.store(false, Ordering::Release);
    assert!(one.is_open(), "batch cancellation preserves actual socket");
    assert_eq!(
        one.rpc("unrelated", json!({"ok":true}), Duration::from_secs(2))
            .unwrap(),
        json!({"ok":true})
    );

    peer.allow.store(false, Ordering::Release);
    let before = peer.metadata_calls().len();
    assert_eq!(
        runtime
            .query("fixture", &[paths[0].clone()], Some("lease"), "pane")
            .err()
            .unwrap(),
        "permission_denied"
    );
    assert_eq!(
        peer.metadata_calls().len(),
        before,
        "denied cache read never stats"
    );
    peer.allow.store(true, Ordering::Release);
    runtime
        .retain(
            "lease2",
            ThreadRef {
                environment: "fixture".into(),
                thread: "thread".into(),
            },
        )
        .unwrap();
    runtime.release("lease");
    let before = peer.metadata_calls().len();
    let mut queries = runtime
        .query(
            "fixture",
            &[paths[0].clone()],
            Some("lease2"),
            "second-pane",
        )
        .unwrap();
    queries.remove(0).wait().unwrap();
    assert_eq!(
        peer.metadata_calls().len(),
        before,
        "last mounted thread reader retains positives"
    );
    runtime.release("lease2");
    runtime
        .retain(
            "lease3",
            ThreadRef {
                environment: "fixture".into(),
                thread: "thread".into(),
            },
        )
        .unwrap();
    let mut queries = runtime
        .query("fixture", &[paths[0].clone()], Some("lease3"), "new-pane")
        .unwrap();
    queries.remove(0).wait().unwrap();
    assert_eq!(
        peer.metadata_calls().len(),
        before + 1,
        "full unmount/remount cannot retain expired old-thread positives"
    );
    runtime.release("lease3");
    assert_eq!(
        runtime
            .query("fixture", &[paths[0].clone()], Some("lease"), "pane")
            .err()
            .unwrap(),
        "metadata_lease_closed"
    );
    rpc_sessions::close_all();
}
