use serde_json::{json, Value};
use t3_protocol::{pull_requests::sidebar_badge, sidebar};
fn link(n: u64, head: &str, base: &str, state: &str) -> Value {
    json!({"host":"github.com","repository":"example/project","number":n,"url":format!("https://github.com/example/project/pull/{n}"),"source":"manual","linkedAt":format!("2026-10-16T01:{n:02}:00Z"),"snapshot":{"state":state,"headBranch":head,"baseBranch":base,"isDraft":false},"stack":null})
}
#[test]
fn legacy_and_explicit_unlink_preserve_native_semantics() {
    let legacy = json!({"projectId":"p","repository":"example/project","number":314,"url":"https://github.com/example/project/pull/314"});
    let b = sidebar_badge(&json!({"projectId":"p","linkedPullRequest":legacy}));
    assert_eq!(b["text"], "314");
    assert_eq!(b["state"], "unknown");
    assert_eq!(b["current"]["host"], "github.com");
    assert_eq!(b["current"]["projectId"], "p");
    assert!(sidebar_badge(&json!({"pullRequests":[],"linkedPullRequest":legacy})).is_null());
    assert_eq!(
        sidebar_badge(&json!({"pullRequests":[],"branchPullRequest":legacy}))["text"],
        "314"
    );
    assert!(sidebar_badge(
        &json!({"linkedPullRequest":{"number":1,"repository":"a/b","url":"javascript:alert(1)"}})
    )
    .is_null());
    let m = sidebar::metadata(
        &json!({"projectId":"p","pullRequests":[link(1,"a","main","merged")]}),
        0,
    );
    assert_eq!(m["pullRequestBadge"]["state"], "merged");
    assert_eq!(m["pullRequests"][0]["number"], 1);
}
#[test]
fn chains_are_stacks_but_forks_cycles_and_unrelated_links_are_counts() {
    let a = link(1, "a", "main", "open");
    let b = link(2, "b", "a", "open");
    let c = link(3, "c", "a", "open");
    let badge = sidebar_badge(&json!({"pullRequests":[a,b]}));
    assert_eq!(badge["kind"], "stack");
    assert_eq!(badge["text"], "2");
    assert_eq!(badge["current"]["number"], 2);
    let badge = sidebar_badge(&json!({"pullRequests":[a,b,c]}));
    assert_eq!(badge["kind"], "multiple");
    assert_eq!(badge["text"], "+3");
    assert_eq!(
        sidebar_badge(&json!({"pullRequests":[link(1,"a","b","open"),link(2,"b","a","open")]}))
            ["kind"],
        "multiple"
    );
    assert_eq!(
        sidebar_badge(
            &json!({"pullRequests":[link(1,"a","main","merged"),link(2,"b","main","merged")]})
        )["state"],
        "merged"
    );
    let mut draft = a.clone();
    draft["snapshot"]["isDraft"] = json!(true);
    assert_eq!(
        sidebar_badge(&json!({"pullRequests":[draft]}))["state"],
        "draft"
    );
    let mut hidden = a.clone();
    hidden["source"] = json!("stack-dismissed");
    assert!(sidebar_badge(&json!({"pullRequests":[hidden]})).is_null());
}
#[test]
fn native_stack_order_and_cross_host_identity_are_preserved() {
    let mut a = link(1, "a", "main", "open");
    let mut b = link(2, "b", "unrelated", "open");
    let stack = json!({"kind":"native","id":"stack","layers":[{"number":2},{"number":1}]});
    a["stack"] = stack.clone();
    b["stack"] = stack;
    assert_eq!(
        sidebar_badge(&json!({"pullRequests":[a,b]}))["current"]["number"],
        1
    );
    let mut elsewhere = a.clone();
    elsewhere["host"] = json!("forge.example");
    elsewhere["url"] = json!("https://forge.example/example/project/pull/1");
    assert_eq!(
        sidebar_badge(&json!({"pullRequests":[a,elsewhere]}))["kind"],
        "multiple"
    );
    assert_eq!(
        sidebar_badge(&json!({"pullRequests":[a,a]}))["kind"],
        "single"
    );
}
