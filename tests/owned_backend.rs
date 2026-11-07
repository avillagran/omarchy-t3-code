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

#[test]
fn rotating_renewal_preserves_snapshots_and_uses_actual_rpc_socket_identities() {
    let root = std::env::temp_dir().join(format!(
        "oa-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    ));
    fs::create_dir(&root).unwrap();
    let config = Config {
        executable: PathBuf::from("/fixture/o3-server"),
        state_root: root.join("state"),
    };
    let owner = Owner::launch(&config).unwrap();
    let before = owner.connection();
    let socket = before
        .rpc_session(t3_protocol::SessionOptions::default())
        .unwrap();
    let identity = socket.id().to_owned();
    assert!(owner
        .fresh_connection()
        .unwrap()
        .same_session_admission(&before));
    assert!(socket.is_open());
    assert_eq!(socket.id(), identity);
    let renewed = owner.refresh_authorization().unwrap();
    assert!(!renewed.same_session_admission(&before));
    assert!(before.same_session_admission(&before.clone()));
    assert_eq!(socket.id(), identity);
    let next = renewed
        .rpc_session(t3_protocol::SessionOptions::default())
        .unwrap();
    assert_ne!(next.id(), identity);
    next.close();
    socket.close();
}

#[test]
fn explicit_shutdown_releases_private_profile_while_old_snapshots_are_retained() {
    let root = std::env::temp_dir().join(format!(
        "os-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    ));
    fs::create_dir(&root).unwrap();
    let config = Config {
        executable: PathBuf::from("/fixture/o3-server"),
        state_root: root.join("state"),
    };
    let old = Owner::launch(&config).unwrap();
    let snapshot = old.connection();
    old.stop().unwrap();
    assert!(old.fresh_connection().is_err());
    let mut replacement = Owner::launch(&config).unwrap();
    old.stop().unwrap();
    assert!(replacement.is_running().unwrap());
    assert!(snapshot.same_session_admission(&snapshot.clone()));
}

#[test]
fn opted_in_browser_descriptors_are_adopted_and_shutdown_interrupts_actual_parent_io() {
    use std::io::Read;
    use std::time::Duration;
    let root = std::env::temp_dir().join(format!(
        "of-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    ));
    fs::create_dir(&root).unwrap();
    let config = Config {
        executable: PathBuf::from("/fixture/o3-server"),
        state_root: root.join("state"),
    };
    let (owner, mut browser) = Owner::launch_with_browser(&config).unwrap();
    let descriptor = t3_protocol::api::call(
        &owner.fresh_connection().unwrap(),
        "server.getConfig",
        &serde_json::json!({}),
        Some(Duration::from_secs(2)),
    )
    .unwrap();
    assert_eq!(
        descriptor["environment"]["capabilities"]["serverBrowser"],
        true
    );
    browser
        .control
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut byte = [0];
    let error = browser.control.read(&mut byte).unwrap_err();
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    owner.stop().unwrap();
    assert_eq!(browser.control.read(&mut byte).unwrap(), 0);
    assert!(owner.fresh_connection().is_err());
}
