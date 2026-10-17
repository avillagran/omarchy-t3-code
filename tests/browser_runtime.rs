use omarchy_t3_client::browser_runtime as runtime;
use serde_json::json;
use sha2::{Digest, Sha256};
#[test]
fn native_partition_identity_preserves_default_namespace_and_isolates_environment_profile_pairs() {
    let default = runtime::partition("environment", "default").unwrap();
    assert_eq!(
        default["partition"],
        format!(
            "persist:t3code-preview-{}",
            &format!("{:x}", Sha256::digest("environment"))[..20]
        )
    );
    assert_eq!(default["persistent"], true);
    let user = runtime::partition("environment", "work").unwrap();
    let scope = json!(["environment", "work"]).to_string();
    assert_eq!(
        user["partition"],
        format!(
            "persist:t3code-preview-profile-{}",
            &format!("{:x}", Sha256::digest(scope.as_bytes()))[..20]
        )
    );
    assert_ne!(
        runtime::partition("a", "b::c").unwrap()["partition"],
        runtime::partition("a::b", "c").unwrap()["partition"]
    );
    assert_ne!(
        runtime::partition("one", "work").unwrap()["partition"],
        runtime::partition("two", "work").unwrap()["partition"]
    );
    let private = runtime::partition("environment", "incognito").unwrap();
    assert_eq!(private["persistent"], false);
    assert!(private["partition"]
        .as_str()
        .unwrap()
        .starts_with("t3code-preview-ephemeral-profile-"));
    assert!(runtime::partition("environment", "bad\nkey").is_err());
}
#[test]
fn persistent_and_incognito_storage_are_private_and_do_not_reuse_document_or_electron_paths() {
    use std::{
        fs,
        os::unix::fs::{symlink, PermissionsExt},
    };
    let root = std::env::temp_dir().join(format!("ot3-browser-runtime-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let storage = root.join("profiles");
    let frames = root.join("frames");
    let value=runtime::prepare(json!({"environmentId":"environment","profileId":"work","profileDirectory":"/wrong","partition":"wrong","credential":"PRIVATE"}),&storage,&frames).unwrap();
    assert!(!value.to_string().contains("PRIVATE"));
    let path = std::path::Path::new(value["profileDirectory"].as_str().unwrap());
    assert!(path.starts_with(&storage));
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&frames).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let absent = root.join("incognito-storage");
    let value = runtime::prepare(
        json!({"environmentId":"environment","profileId":"incognito"}),
        &absent,
        &root.join("private-frames"),
    )
    .unwrap();
    assert!(value["profileDirectory"].is_null());
    assert!(!absent.exists());
    let linked = root.join("linked");
    symlink(&storage, &linked).unwrap();
    assert_eq!(
        runtime::prepare(
            json!({"environmentId":"environment","profileId":"work"}),
            &linked,
            &frames
        )
        .unwrap_err(),
        "browser_storage_unavailable"
    );
    fs::remove_dir_all(root).unwrap();
}
