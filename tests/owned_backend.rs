use omarchy_t3_client::owned_backend::{Config, Owner};
use std::{fs, path::PathBuf};
#[test]
fn actual_child_auth_rpc_profile_lock_and_shutdown_are_owned() {
    // This immutable CLI fixture is mounted only by the private-container runner.
    let executable = PathBuf::from("/fixture/o3-server");
    assert!(executable.is_file(), "use tools/test-owned-backend");
    let root = std::env::temp_dir().join(format!(
        "ob-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    ));
    fs::create_dir(&root).unwrap();
    let shared = root.join("shared-profile");
    fs::create_dir(&shared).unwrap();
    fs::write(shared.join("retained"), b"shared fixture unchanged").unwrap();
    let config = Config {
        executable,
        state_root: root.join("state"),
    };
    let mut owner = Owner::launch(&config).unwrap();
    assert!(owner.is_running().unwrap());
    assert_eq!(owner.profile(), config.profile());
    assert!(owner.origin().starts_with("http://127.0.0.1:"));
    assert!(matches!(Owner::launch(&config),Err(error) if error=="owned_backend_already_running"));
    let runtime: serde_json::Value = serde_json::from_slice(
        &fs::read(config.profile().join("userdata/server-runtime.json")).unwrap(),
    )
    .unwrap();
    let pid = runtime["pid"].as_i64().unwrap();
    drop(owner);
    assert_eq!(
        unsafe { libc::kill(pid as i32, 0) },
        -1,
        "owned child must be reaped"
    );
    assert_eq!(
        fs::read(shared.join("retained")).unwrap(),
        b"shared fixture unchanged"
    );
    assert!(
        config.profile().is_dir(),
        "shutdown retains the private profile"
    );
}
