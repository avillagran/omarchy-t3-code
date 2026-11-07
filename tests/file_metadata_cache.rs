//! State transitions from the 12 unmodified native 4288 metadata tests; their
//! actual Effect runtime suite runs separately with test-native-file-metadata-container.
use omarchy_t3_client::file_metadata_cache::{
    batches, Cache, Completion, Metadata, ThreadRef, CACHE_LIMIT, STALE_MS,
};
fn file(bytes: u64) -> Option<Metadata> {
    Some(Metadata {
        kind: "file".into(),
        byte_length: Some(bytes),
        mime_type: None,
    })
}
fn directory() -> Option<Metadata> {
    Some(Metadata {
        kind: "directory".into(),
        byte_length: None,
        mime_type: None,
    })
}
fn setup() -> Cache {
    let mut cache = Cache::default();
    cache.open_session("physical-one", "env");
    cache
}
fn mount(cache: &mut Cache) -> ThreadRef {
    let r = ThreadRef {
        environment: "env".into(),
        thread: "thread".into(),
    };
    cache.retain_thread(r.clone());
    r
}
fn answer(
    cache: &mut Cache,
    id: &str,
    r: Option<&ThreadRef>,
    path: &str,
    value: Option<Metadata>,
    now: u64,
) {
    let request = cache.begin(id, r, path).unwrap();
    assert_eq!(cache.complete(request, value, now), Completion::Remembered)
}
#[test]
fn mounted_misses_expire_but_known_thread_entries_survive_global_ttl() {
    let mut c = setup();
    let r = mount(&mut c);
    answer(&mut c, "physical-one", Some(&r), "/new", None, 0);
    answer(&mut c, "physical-one", Some(&r), "/known", file(5), 0);
    assert_eq!(
        c.known("physical-one", Some(&r), "/new", STALE_MS - 1),
        Some(None)
    );
    assert_eq!(c.known("physical-one", Some(&r), "/new", STALE_MS), None);
    assert_eq!(
        c.known("physical-one", Some(&r), "/known", STALE_MS + 1),
        Some(file(5))
    );
    answer(
        &mut c,
        "physical-one",
        Some(&r),
        "/new",
        directory(),
        STALE_MS + 1,
    );
    assert_eq!(
        c.known("physical-one", Some(&r), "/new", 3 * STALE_MS),
        Some(directory())
    );
}
#[test]
fn numeric_suffixes_in_literal_seed_names_do_not_seed_different_paths() {
    let mut c = setup();
    let guard = c.seed_guard("physical-one").unwrap();
    c.seed_entries(
        &guard,
        &[("/workspace/report:123".into(), "directory".into())],
        0,
    );
    assert_eq!(c.known("physical-one", None, "/workspace/report", 0), None);
    assert_eq!(
        c.known("physical-one", None, "/workspace/report:123", 0),
        Some(directory())
    );
    c.seed_file(&guard, "/workspace/read:12:3", 100, 0);
    assert_eq!(
        c.known("physical-one", None, "/workspace/read:12:3", 0),
        Some(file(100))
    );
    assert_eq!(c.known("physical-one", None, "/workspace/read", 0), None);
}
#[test]
fn in_flight_invalidation_retries_only_the_changed_path() {
    let mut c = setup();
    let r = mount(&mut c);
    let changed = c.begin("physical-one", Some(&r), "/file").unwrap();
    let unchanged = c.begin("physical-one", None, "/unchanged").unwrap();
    c.invalidate("physical-one", "/file");
    assert_eq!(c.complete(changed, file(5), 0), Completion::Retry);
    assert_eq!(
        c.complete(unchanged, directory(), 0),
        Completion::Remembered
    );
    assert_eq!(c.known("physical-one", Some(&r), "/file", 0), None);
    assert_eq!(
        c.known("physical-one", None, "/unchanged", 0),
        Some(directory())
    );
    answer(&mut c, "physical-one", Some(&r), "/file", file(10), 0);
    assert_eq!(
        c.known("physical-one", Some(&r), "/file", 2 * STALE_MS),
        Some(file(10))
    );
}
#[test]
fn offscreen_invalidation_performs_no_request_until_a_new_read() {
    let mut c = setup();
    let r = mount(&mut c);
    answer(&mut c, "physical-one", Some(&r), "/file", None, 0);
    c.invalidate("physical-one", "/file");
    assert_eq!(c.entry_count("physical-one"), 0);
    assert_eq!(c.known("physical-one", Some(&r), "/file", 1), None);
    answer(&mut c, "physical-one", Some(&r), "/file", directory(), 1);
    assert_eq!(
        c.known("physical-one", Some(&r), "/file", 1),
        Some(directory())
    );
}
#[test]
fn invalidating_one_retained_path_keeps_unrelated_answers() {
    let mut c = setup();
    let r = mount(&mut c);
    answer(&mut c, "physical-one", Some(&r), "/file", file(5), 0);
    answer(&mut c, "physical-one", Some(&r), "/unchanged", None, 0);
    c.invalidate("physical-one", "/file");
    assert_eq!(c.known("physical-one", Some(&r), "/file", 1), None);
    assert_eq!(
        c.known("physical-one", Some(&r), "/unchanged", 1),
        Some(None)
    );
    answer(&mut c, "physical-one", Some(&r), "/file", file(10), 1);
    assert_eq!(
        c.known("physical-one", Some(&r), "/file", 1),
        Some(file(10))
    );
}
#[test]
fn replacement_socket_discards_retained_values_and_late_old_responses() {
    let mut c = setup();
    let r = mount(&mut c);
    answer(&mut c, "physical-one", Some(&r), "/file", file(5), 0);
    let old = c.begin("physical-one", Some(&r), "/late").unwrap();
    c.close_session("physical-one");
    c.open_session("physical-two", "env");
    assert_eq!(c.known("physical-two", Some(&r), "/file", 1), None);
    assert_eq!(c.complete(old, file(9), 1), Completion::SessionClosed);
    answer(&mut c, "physical-two", Some(&r), "/file", directory(), 1);
    assert_eq!(
        c.known("physical-two", Some(&r), "/file", 1),
        Some(directory())
    );
}
#[test]
fn batch_plans_and_caches_are_separate_for_actual_environment_sockets() {
    let mut c = setup();
    c.open_session("physical-other", "other");
    let guard = c.seed_guard("physical-one").unwrap();
    c.seed_entries(&guard, &[("/known".into(), "directory".into())], 0);
    assert_eq!(c.known("physical-other", None, "/known", 0), None);
    let requests = [
        c.begin("physical-one", None, "/new").unwrap(),
        c.begin("physical-other", None, "/new").unwrap(),
    ];
    let planned = batches(&requests);
    assert_eq!(planned.len(), 2);
    assert_ne!(planned[0].session, planned[1].session);
    assert_eq!(planned[0].paths, vec!["/new"]);
    assert_eq!(planned[1].paths, vec!["/new"]);
}
#[test]
fn failed_batch_is_not_cached_and_all_paths_can_retry_after_access_is_granted() {
    let mut c = setup();
    let a = c.begin("physical-one", None, "/a").unwrap();
    let b = c.begin("physical-one", None, "/b").unwrap();
    c.abandon(a);
    c.abandon(b);
    assert_eq!(c.known("physical-one", None, "/a", 0), None);
    assert_eq!(c.known("physical-one", None, "/b", 0), None);
    answer(&mut c, "physical-one", None, "/a", directory(), 0);
    assert_eq!(c.known("physical-one", None, "/a", 0), Some(directory()));
}
#[test]
fn read_size_seeding_and_windows_normalization_share_a_cache_key() {
    let mut c = setup();
    let guard = c.seed_guard("physical-one").unwrap();
    c.seed_file(&guard, r"C:\Workspace\file", 100, 0);
    assert_eq!(
        c.known("physical-one", None, "c:/workspace/file/", 0),
        Some(file(100))
    );
}
#[test]
fn one_batch_deduplicates_wire_paths_and_reuses_its_completed_answers() {
    let mut c = setup();
    let requests = [
        c.begin("physical-one", None, "/file").unwrap(),
        c.begin("physical-one", None, "/folder.ts").unwrap(),
        c.begin("physical-one", None, "/file").unwrap(),
    ];
    let planned = batches(&requests);
    assert_eq!(planned.len(), 1);
    assert_eq!(planned[0].paths, vec!["/file", "/folder.ts"]);
    assert_eq!(planned[0].requests, vec![0, 1, 2]);
    for (index, request) in requests.into_iter().enumerate() {
        assert_eq!(
            c.complete(request, if index == 1 { directory() } else { file(5) }, 0),
            Completion::Remembered
        )
    }
    assert_eq!(c.known("physical-one", None, "/file", 0), Some(file(5)));
}
#[test]
fn search_and_tree_kinds_seed_queries_without_a_fetch() {
    let mut c = setup();
    let guard = c.seed_guard("physical-one").unwrap();
    c.seed_entries(
        &guard,
        &[("/workspace/folder.ts".into(), "directory".into())],
        0,
    );
    assert_eq!(
        c.known("physical-one", None, "/workspace/folder.ts", 0),
        Some(directory())
    );
}
#[test]
fn large_mounts_use_native_64_path_batches_and_cache_misses() {
    let mut c = setup();
    let requests = (0..130)
        .map(|i| {
            c.begin("physical-one", None, &format!("/workspace/{i}"))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let planned = batches(&requests);
    assert_eq!(
        planned.iter().map(|b| b.paths.len()).collect::<Vec<_>>(),
        vec![64, 64, 2]
    );
    assert_eq!(planned.iter().map(|b| b.paths.len()).sum::<usize>(), 130);
    for request in requests {
        assert_eq!(c.complete(request, None, 0), Completion::Remembered)
    }
    assert_eq!(c.known("physical-one", None, "/workspace/0", 1), Some(None));
}
#[test]
fn global_budget_evicts_oldest_insertions_while_thread_retention_survives() {
    let mut c = setup();
    let r = mount(&mut c);
    answer(&mut c, "physical-one", Some(&r), "/retained", file(5), 0);
    for i in 0..CACHE_LIMIT + 1 {
        answer(&mut c, "physical-one", None, &format!("/file/{i}"), None, 0)
    }
    assert_eq!(c.entry_count("physical-one"), CACHE_LIMIT);
    assert_eq!(c.known("physical-one", None, "/retained", 1), None);
    assert_eq!(
        c.known("physical-one", Some(&r), "/retained", 2 * STALE_MS),
        Some(file(5))
    );
    c.release_thread(&r);
    assert_eq!(
        c.known("physical-one", Some(&r), "/retained", 2 * STALE_MS),
        None
    );
}
#[test]
fn stale_reads_cannot_seed_after_invalidation_or_socket_replacement() {
    let mut c = setup();
    let guard = c.seed_guard("physical-one").unwrap();
    c.invalidate("physical-one", "/other");
    c.seed_file(&guard, "/file", 100, 0);
    assert_eq!(c.known("physical-one", None, "/file", 0), None);
    let guard = c.seed_guard("physical-one").unwrap();
    c.close_session("physical-one");
    c.open_session("physical-two", "env");
    c.seed_entries(&guard, &[("/file".into(), "directory".into())], 0);
    assert_eq!(c.known("physical-two", None, "/file", 0), None);
}
#[test]
fn partial_seed_updates_retained_values_without_erasing_richer_fields() {
    let mut c = setup();
    let r = mount(&mut c);
    let mut rich = file(5).unwrap();
    rich.mime_type = Some("text/plain".into());
    answer(
        &mut c,
        "physical-one",
        Some(&r),
        "/file",
        Some(rich.clone()),
        0,
    );
    let guard = c.seed_guard("physical-one").unwrap();
    c.seed_file(&guard, "/file", 10, 2 * STALE_MS);
    rich.byte_length = Some(10);
    assert_eq!(
        c.known("physical-one", Some(&r), "/file", 2 * STALE_MS),
        Some(Some(rich))
    );
}
#[test]
fn pending_old_mount_does_not_insert_retention_into_a_recreated_thread() {
    let mut c = setup();
    let r = mount(&mut c);
    let request = c.begin("physical-one", Some(&r), "/file").unwrap();
    c.release_thread(&r);
    c.retain_thread(r.clone());
    assert_eq!(c.complete(request, file(5), 0), Completion::Remembered);
    assert_eq!(
        c.known("physical-one", Some(&r), "/file", 2 * STALE_MS),
        None
    );
}
#[test]
fn multiple_mounted_readers_release_only_their_own_retention() {
    let mut c = setup();
    let r = mount(&mut c);
    c.retain_thread(r.clone());
    answer(&mut c, "physical-one", Some(&r), "/file", file(5), 0);
    c.release_thread(&r);
    assert_eq!(
        c.known("physical-one", Some(&r), "/file", 2 * STALE_MS),
        Some(file(5))
    );
    c.release_thread(&r);
    assert_eq!(
        c.known("physical-one", Some(&r), "/file", 2 * STALE_MS),
        None
    );
}
