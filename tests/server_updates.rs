use omarchy_t3_client::server_updates as updates;
use serde_json::json;
use std::cmp::Ordering::*;

#[test]
fn versions_follow_t3_exact_semver_and_channels() {
    for v in ["0.0.46-nightly.20261013.3042", "0.1.3", "1.2.3-rc.1+build.5"] {
        assert!(updates::exact(v), "{v}");
    }
    for v in ["0.1", "01.2.3", "0.1.3-", "v0.1.3", "0.1.3-01", "0.1.3 ; rm -rf ~", "0.1.3-a'b"] {
        assert!(!updates::exact(v), "{v}");
    }
    assert_eq!(updates::channel("0.1.3-nightly.20261023.3566"), "nightly");
    assert_eq!(updates::channel("0.1.3-preview.20261023.1"), "preview");
    assert_eq!(updates::channel("0.1.3"), "stable");
    assert_eq!(updates::channel("0.1.3-rc.1"), "stable");
    assert_eq!(updates::compare("0.1.3-nightly.20261023.3566", "0.0.46-nightly.20261013.3042"), Greater);
    assert_eq!(updates::compare("0.1.3-nightly.20261023.3566", "0.1.3-nightly.20261024.3604"), Less);
    assert_eq!(updates::compare("0.1.3", "0.1.3-nightly.20261024.3604"), Greater);
    assert_eq!(updates::compare("0.1.3+a", "0.1.3+b"), Equal);
}

#[test]
fn newest_release_skips_drafts_and_other_channels() {
    let releases = json!([
        {"tag_name":"v0.1.4-nightly.20261025.3700","draft":true},
        {"tag_name":"v0.1.4-preview.20261025.1"},
        {"tag_name":"v0.1.3-nightly.20261024.3604"},
        {"tag_name":"v0.1.2"},
    ]);
    assert_eq!(updates::newest(&releases, "nightly").as_deref(), Some("0.1.3-nightly.20261024.3604"));
    assert_eq!(updates::newest(&releases, "stable").as_deref(), Some("0.1.2"));
    assert_eq!(updates::newest(&json!([]), "stable"), None);
    let a = updates::advisory("0.0.46-nightly.20261013.3042", Some("0.1.3-nightly.20261024.3604"));
    assert_eq!(a["latest"], "0.1.3-nightly.20261024.3604");
    assert_eq!(a["channel"], "nightly");
    assert!(updates::advisory("0.1.3-nightly.20261024.3604", Some("0.1.3-nightly.20261024.3604"))["latest"].is_null());
    // Never a downgrade.
    assert!(updates::advisory("0.1.4-nightly.20261030.3800", Some("0.1.3-nightly.20261024.3604"))["latest"].is_null());
}

#[test]
fn automatic_updates_wait_for_a_quiet_server_and_never_loop_on_failure() {
    let now = 1_800_000_000_000i64;
    let snapshot = json!({"threads":[
        {"environmentId":"local","working":false,"updatedAt":now - updates::QUIET_MS - 1},
        {"environmentId":"node","working":true,"updatedAt":now - updates::QUIET_MS - 1},
        {"environmentId":"recent","working":false,"updatedAt":now - 1000},
        {"environmentId":"asking","working":false,"pending":true,"updatedAt":0},
    ]});
    assert!(updates::quiet(&snapshot, "local", now));
    assert!(updates::quiet(&snapshot, "empty", now));
    for busy in ["node", "recent", "asking"] {
        assert!(!updates::quiet(&snapshot, busy, now), "{busy}");
    }
    let row = json!({"latest":"0.1.3-nightly.20261024.3604","canUpdate":true});
    let none = serde_json::Value::Null;
    assert!(updates::due("auto", &row, &none, true, now));
    assert!(!updates::due("notify", &row, &none, true, now));
    assert!(!updates::due("auto", &row, &none, false, now));
    assert!(!updates::due("auto", &json!({"latest":null,"canUpdate":true}), &none, true, now));
    assert!(!updates::due("auto", &json!({"latest":"0.1.3","canUpdate":false}), &none, true, now));
    let failed = json!({"target":"0.1.3-nightly.20261024.3604","at":now - 1000,"status":"failed"});
    assert!(!updates::due("auto", &row, &failed, true, now));
    assert!(updates::due("auto", &row, &failed, true, now + updates::RETRY_MS));
    // A newer release than the one that failed is tried right away.
    assert!(updates::due("auto", &json!({"latest":"0.1.4-nightly.20261025.3700","canUpdate":true}), &failed, true, now));
}
