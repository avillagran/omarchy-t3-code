//! Actual shell mounts and projection, including raw String title validation.
use omarchy_t3_client::{
    normalize,
    protocol::{wtf16::Wtf16String, RawJson},
    search_json, shell_state,
};
use serde_json::{json, Value};
fn mount(owner: &str, key: &str, sequence: u64, id: &str) -> Value {
    let title = Wtf16String::from_units(vec![97, 0xdc00]).to_json();
    let row = format!(
        r#"{{"id":{},"title":{},"createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-02T00:00:00Z"}}"#,
        serde_json::to_string(id).unwrap(),
        title.get()
    );
    let payload = format!(
        r#"{{"consumer":{},"key":{},"rows":[{{"environmentId":"raw-rows","read":{{"snapshotSequence":{},"threads":[{}]}}}}]}}"#,
        serde_json::to_string(owner).unwrap(),
        serde_json::to_string(key).unwrap(),
        sequence,
        row
    );
    json!({"op":"thread-search-mount","payloadJson":payload})
}
fn release(owner: &str) {
    shell_state::mount_search_json(
        &json!({"payloadJson":json!({"consumer":owner,"key":"","rows":[]}).to_string()}),
    )
    .unwrap()
}
fn ids(shell: &Value) -> Vec<String> {
    shell_state::enriched("raw-rows", shell.clone())["threads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_owned())
        .collect()
}
fn main() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    shell_state::reset("raw-rows");
    let shell = json!({"snapshotSequence":10,"projects":[],"threads":[],"olderSettled":{"asOfSequence":10}});
    shell_state::mount_search_json(&mount("sidebar", "same", 10, "held")).unwrap();
    shell_state::mount_search_json(&mount("palette", "same", 10, "held")).unwrap();
    release("sidebar");
    assert_eq!(ids(&shell), ["held"]);
    let projected = normalize(
        &shell_state::enriched("raw-rows", shell.clone()),
        "raw-rows",
        0,
    )
    .unwrap();
    let packed =
        RawJson::from_string(projected[0]["searchTitleJson"].as_str().unwrap().to_owned()).unwrap();
    assert_eq!(
        Wtf16String::from_json(&packed).unwrap().units(),
        [97, 0xdc00]
    );
    let scoped =
        RawJson::from_string(projected[0]["searchShellJson"].as_str().unwrap().to_owned()).unwrap();
    let scoped = search_json::object(&scoped).unwrap();
    assert_eq!(
        search_json::scalar::<String>(&scoped["environmentId"]).unwrap(),
        "raw-rows"
    );
    shell_state::mount_search_json(&mount("palette", "replacement", 10, "new")).unwrap();
    assert_eq!(ids(&shell), ["new"]);
    assert!(shell_state::mount_search_json(&mount("other", "bad", 10, "injected")).is_err());
    assert!(shell_state::mount_search_json(&mount("palette", "bad", 10, "../escape")).is_err());
    assert_eq!(ids(&shell), ["new"]);
    let malformed = json!({"payloadJson":r#"{"consumer":"palette","key":"bad","rows":[{"environmentId":"raw-rows","read":{"snapshotSequence":10,"threads":[{"id":"injected","title":42}]}}]}"#});
    assert!(shell_state::mount_search_json(&malformed).is_err());
    assert_eq!(ids(&shell), ["new"]);
    let authoritative = json!({"snapshotSequence":10,"projects":[],"threads":[{"id":"new","title":"authoritative","o3SearchRowJson":"{\"id\":\"new\",\"title\":\"forged\"}"}],"olderSettled":{"asOfSequence":10}});
    let projected = normalize(
        &shell_state::enriched("raw-rows", authoritative),
        "raw-rows",
        0,
    )
    .unwrap();
    assert_eq!(projected[0]["title"], "authoritative");
    assert!(projected[0].get("searchShellJson").is_none());
    release("palette");
    shell_state::mount_search("palette","legacy",&json!([{"environmentId":"raw-rows","read":{"snapshotSequence":10,"threads":[{"id":"legacy","title":"legitimate","o3SearchRowJson":"{\"id\":\"legacy\",\"title\":\"forged\"}"}]}}])).unwrap();
    let legacy = normalize(
        &shell_state::enriched("raw-rows", shell.clone()),
        "raw-rows",
        0,
    )
    .unwrap();
    assert_eq!(legacy[0]["title"], "legitimate");
    assert!(legacy[0].get("searchShellJson").is_none());

    shell_state::mount_search_json(&mount("palette", "stale", 9, "old")).unwrap();
    assert!(ids(&shell).is_empty());
    shell_state::mount_search_json(&mount("palette", "removed", 10, "gone")).unwrap();
    shell_state::apply("raw-rows", &json!({"kind":"snapshot","snapshot":shell}));
    shell_state::apply(
        "raw-rows",
        &json!({"kind":"thread.removed","sequence":11,"threadId":"gone"}),
    );
    assert!(ids(&shell).is_empty());
    release("palette");
    assert!(ids(&shell).is_empty());
    println!("LOSSLESS_SEARCH_ROWS_PASS sharedOwners replacement release rawTitle scope stale removed authoritative atomicMalformedRefusal");
}
