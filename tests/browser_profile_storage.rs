use omarchy_t3_client::browser_runtime::{clear_owned_storage, lock_profile, partition};
use std::{fs, os::unix::fs::symlink};
#[test]
fn cleanup_requires_the_exact_live_partition_lock_and_rejects_symlink_storage() {
    let root = std::env::temp_dir().join(format!("ot3-profile-lock-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let storage = root.join("profiles");
    fs::create_dir_all(&storage).unwrap();
    let name = partition("local", "work").unwrap();
    let path = storage.join(
        name["partition"]
            .as_str()
            .unwrap()
            .trim_start_matches("persist:"),
    );
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("site-data"), "fixture").unwrap();
    let other = lock_profile(&storage, "local", "default").unwrap();
    assert_eq!(
        clear_owned_storage(&storage, "local", "work", &other).unwrap_err(),
        "browser_profile_storage_unsafe"
    );
    assert!(path.exists());
    let held = lock_profile(&storage, "local", "work").unwrap();
    assert!(matches!(lock_profile(&storage,"local","work"),Err(e) if e=="browser_profile_busy"));
    clear_owned_storage(&storage, "local", "work", &held).unwrap();
    assert!(!path.exists());
    drop(held);
    let held = lock_profile(&storage, "local", "work").unwrap();
    let outside = root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("keep"), "keep").unwrap();
    symlink(&outside, &path).unwrap();
    assert_eq!(
        clear_owned_storage(&storage, "local", "work", &held).unwrap_err(),
        "browser_profile_storage_unsafe"
    );
    assert!(outside.join("keep").exists());
    drop(held);
    drop(other);
    fs::remove_dir_all(root).unwrap();
}
