//! Link this private fixture to the actual current client library inside the sandbox.
use omarchy_t3_client::shell_state::{apply, enriched, mount_search, reset};
use serde_json::{json, Value};

fn ids(env: &str, shell: &Value) -> Vec<String> {
    enriched(env, shell.clone())["threads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect()
}
fn main() {
    let env = "private-search-mounts";
    reset(env);
    let shell = json!({"snapshotSequence":10,"threads":[{"id":"current","title":"authoritative"}],
        "olderSettled":{"asOfSequence":10}});
    let page = |id: &str, sequence: u64| {
        json!([{"environmentId":env,"read":{
        "snapshotSequence":sequence,"threads":[{"id":id,"title":"search"}]}}])
    };
    mount_search("sidebar", "shown-one", &page("older", 10)).unwrap();
    assert_eq!(ids(env, &shell), ["current", "older"]);
    mount_search("palette", "shown-one", &page("older", 10)).unwrap();
    mount_search("sidebar", "", &json!([])).unwrap();
    assert_eq!(ids(env, &shell), ["current", "older"]);
    mount_search("palette", "shown-two", &page("replacement", 10)).unwrap();
    assert_eq!(ids(env, &shell), ["current", "replacement"]);
    mount_search("sidebar", "shown-one", &page("older", 10)).unwrap();
    mount_search("palette", "", &json!([])).unwrap();
    assert_eq!(ids(env, &shell), ["current", "older"]);
    mount_search("sidebar", "", &json!([])).unwrap();
    assert_eq!(ids(env, &shell), ["current"]);
    mount_search("sidebar", "shell-wins", &page("current", 20)).unwrap();
    assert_eq!(
        enriched(env, shell.clone())["threads"][0]["title"],
        "authoritative"
    );
    mount_search("sidebar", "stale", &page("older", 9)).unwrap();
    assert_eq!(ids(env, &shell), ["current"]);
    mount_search("sidebar", "removed", &page("older", 10)).unwrap();
    apply(
        env,
        &json!({"kind":"snapshot","snapshot":{"snapshotSequence":10,"projects":[],"threads":[]}}),
    );
    apply(
        env,
        &json!({"kind":"thread.removed","sequence":11,"threadId":"older"}),
    );
    assert_eq!(ids(env, &shell), ["current"]);
    assert!(mount_search("other", "", &json!([])).is_err());
    assert!(mount_search(
        "sidebar",
        "invalid",
        &json!([{"environmentId":env,"read":{
        "snapshotSequence":11,"threads":[{"id":"../escape"}]}}])
    )
    .is_err());
    // A malformed replacement leaves the current owner intact.
    let fresh = json!({"snapshotSequence":11,"threads":[],"olderSettled":{"asOfSequence":11}});
    mount_search("sidebar", "fresh", &page("newer", 11)).unwrap();
    assert!(mount_search("sidebar", "bad", &json!([{"environmentId":env,"read":{}}])).is_err());
    assert_eq!(ids(env, &fresh), ["newer"]);
    mount_search("sidebar", "", &json!([])).unwrap();
    assert!(ids(env, &fresh).is_empty());
    println!("THREAD_SEARCH_MOUNTS_PASS: shared ownership, replacement, release, shell precedence, cut, removals and atomic refusal");
}
