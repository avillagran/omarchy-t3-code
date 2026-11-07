//! Actual Rust consent owner and original producer in an isolated metadata realm.
use omarchy_t3_client::{
    voice_capture::{
        resources::{Assets, DescriptorArtifact, Runtime},
        Cleanup, StartupFailure,
    },
    voice_consent::{ConsentOwner, Snapshot},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::{DirBuilderExt, PermissionsExt},
    },
    path::Path,
    sync::Arc,
};
fn choice(snapshot: Snapshot) -> Value {
    serde_json::from_str(&snapshot.choice_json).unwrap()
}
fn observed(owner: &ConsentOwner) -> OwnedFd {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, owner.process_id().unwrap(), 0) };
    assert!(fd >= 0);
    unsafe { OwnedFd::from_raw_fd(fd as i32) }
}
fn exited(fd: &OwnedFd) {
    let mut descriptor = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    assert_eq!(unsafe { libc::poll(&mut descriptor, 1, 1000) }, 1);
}
fn main() {
    assert!(Path::new("/run/.containerenv").exists());
    let runtime = Arc::new(Runtime::admit(Path::new("/electron")).unwrap());
    let path = Path::new("/source/electron-private-fd.node");
    let descriptor = DescriptorArtifact {
        path: path.into(),
        bytes: fs::metadata(path).unwrap().len(),
        sha256: format!("{:x}", Sha256::digest(fs::read(path).unwrap())),
    };
    let assets = Arc::new(Assets::consent(Path::new("/state/assets"), &descriptor).unwrap());
    let namespace = Path::new("/state/grants");
    fs::DirBuilder::new().mode(0o700).create(namespace).unwrap();
    let first = ConsentOwner::launch(
        runtime.clone(),
        assets.clone(),
        Path::new("/state/first"),
        namespace,
    )
    .unwrap();
    let second = ConsentOwner::launch(
        runtime.clone(),
        assets.clone(),
        Path::new("/state/second"),
        namespace,
    )
    .unwrap();
    let first_exit = observed(&first);
    let second_exit = observed(&second);
    let environment = json!({"kind":"environment","environmentId":"actual-env"}).to_string();
    let cloud =
        json!({"kind":"cloud","environmentId":"actual-env","provider":"openai"}).to_string();
    assert_eq!(first.snapshot().unwrap().choice_json, "null");
    let selected = choice(first.choose_audio(&environment).unwrap());
    let guard = first.bind_audio(&selected["id"].to_string()).unwrap();
    assert!(guard.is_current());
    let fresh = choice(second.choose_audio(&environment).unwrap());
    assert_ne!(fresh["id"], selected["id"]);
    assert!(!guard.is_current());
    let current = first.bind_audio(&fresh["id"].to_string()).unwrap();
    assert!(current.is_current());
    let cleanup = first.choose_cleanup(true).unwrap().cleanup_json;
    second.choose_audio("null").unwrap();
    assert!(!current.is_current());
    assert_eq!(first.snapshot().unwrap().cleanup_json, cleanup);
    let prior = choice(first.choose_audio(&environment).unwrap());
    let prior_guard = first.bind_audio(&prior["id"].to_string()).unwrap();
    let late = second.pending_cloud(&cloud).unwrap();
    assert!(prior_guard.is_current());
    let other = choice(first.choose_audio(&environment).unwrap());
    assert_eq!(choice(late.acknowledge().unwrap()), other);
    let next = second.pending_cloud(&cloud).unwrap();
    next.acknowledge().unwrap();
    assert_eq!(
        choice(first.snapshot().unwrap())["destination"]["provider"],
        "openai"
    );
    assert!(first.change_generation().unwrap() > 0);
    // Retain a legacy UTF16 fixture. Neither Rust's metadata boundary nor the
    // real native bind narrows the original grant identity/EnvironmentId type.
    let key = "t3code:voice-audio-grant:v1";
    let key_bytes: Vec<u8> = key.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let value = r#"{"id":"legacy\ud800","destination":{"kind":"environment","environmentId":"native\udfff"}}"#;
    let encoded: Vec<u8> = value.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut bytes = b"OVS1".to_vec();
    bytes.extend_from_slice(&(key_bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&key_bytes);
    bytes.extend_from_slice(&encoded);
    let entry = namespace.join(format!("{:x}.consent", Sha256::digest(&key_bytes)));
    let temporary = namespace.join("retained-legacy-fixture.pending");
    fs::write(&temporary, bytes).unwrap();
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).unwrap();
    fs::rename(temporary, entry).unwrap();
    let raw = first.snapshot().unwrap().choice_json;
    assert!(raw.contains(r"\ud800") && raw.contains(r"\udfff"));
    let legacy = first.bind_audio(r#""legacy\ud800""#).unwrap();
    assert!(legacy.is_current());
    fs::rename(namespace, "/state/retained-grants").unwrap();
    fs::DirBuilder::new().mode(0o700).create(namespace).unwrap();
    assert!(!legacy.is_current());
    assert!(first.snapshot().is_err());
    assert_eq!(first.close(), Cleanup::Unknown);
    assert_eq!(first.close(), Cleanup::Unknown);
    exited(&first_exit);
    // A fresh owner on the replacement namespace cannot restore an old guard.
    let replacement = ConsentOwner::launch(
        runtime.clone(),
        assets.clone(),
        Path::new("/state/replacement"),
        namespace,
    )
    .unwrap();
    let replacement_exit = observed(&replacement);
    replacement.choose_audio(&environment).unwrap();
    assert!(!legacy.is_current());
    assert_eq!(replacement.close(), Cleanup::Confirmed);
    exited(&replacement_exit);
    // The second owner has not observed replacement yet; its retirement is
    // still explicit and cannot make any old guard current.
    assert!(!prior_guard.is_current());
    assert_eq!(second.close(), Cleanup::Confirmed);
    exited(&second_exit);
    let dropped = ConsentOwner::launch(
        runtime.clone(),
        assets.clone(),
        Path::new("/state/drop"),
        namespace,
    )
    .unwrap();
    let dropped_exit = observed(&dropped);
    let selected = choice(dropped.choose_audio(&environment).unwrap());
    let dropped_guard = dropped.bind_audio(&selected["id"].to_string()).unwrap();
    assert!(dropped_guard.is_current());
    drop(dropped);
    exited(&dropped_exit);
    assert!(!dropped_guard.is_current());
    let killed = ConsentOwner::launch(
        runtime.clone(),
        assets.clone(),
        Path::new("/state/killed"),
        namespace,
    )
    .unwrap();
    let killed_exit = observed(&killed);
    let selected = choice(killed.choose_audio(&environment).unwrap());
    let killed_guard = killed.bind_audio(&selected["id"].to_string()).unwrap();
    assert!(killed_guard.is_current());
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                killed_exit.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        },
        0
    );
    exited(&killed_exit);
    assert!(!killed_guard.is_current());
    assert_eq!(killed.close(), Cleanup::Unknown);
    let stopped = ConsentOwner::launch(
        runtime.clone(),
        assets.clone(),
        Path::new("/state/stopped"),
        namespace,
    )
    .unwrap();
    let stopped_exit = observed(&stopped);
    assert_eq!(
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                stopped_exit.as_raw_fd(),
                libc::SIGSTOP,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        },
        0
    );
    let started = std::time::Instant::now();
    assert_eq!(stopped.close(), Cleanup::Unknown);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(5500),
        "Close must share one five-second deadline"
    );
    exited(&stopped_exit);
    let bad = Path::new("/state/invalid.node");
    let data = b"retained invalid helper";
    fs::write(bad, data).unwrap();
    fs::set_permissions(bad, fs::Permissions::from_mode(0o400)).unwrap();
    let bad_descriptor = DescriptorArtifact {
        path: bad.into(),
        bytes: data.len() as u64,
        sha256: format!("{:x}", Sha256::digest(data)),
    };
    let bad_assets =
        Arc::new(Assets::consent(Path::new("/state/bad-assets"), &bad_descriptor).unwrap());
    let failure = match ConsentOwner::launch(
        runtime.clone(),
        bad_assets,
        Path::new("/state/invalid-startup"),
        namespace,
    ) {
        Ok(_) => panic!("Invalid addon admitted"),
        Err(error) => error,
    };
    assert_eq!(
        failure
            .get_ref()
            .unwrap()
            .downcast_ref::<StartupFailure>()
            .unwrap()
            .cleanup,
        Cleanup::Unknown
    );
    runtime.verify().unwrap();
    assets.verify().unwrap();
    println!(
        "{}",
        json!({"cases":7,"crossProcessFreshGrant":true,"sameDestinationRevokes":true,"independentCleanup":true,"lateCloudAckCannotRestore":true,"rawUtf16InnerJson":true,"namespaceReplacementDenies":true,"dropExits":true,"deadHelperDenies":true,"stoppedHelperSharesFiveSecondCloseBudget":true,"invalidStartupUnknown":true,"scope":"Actual Rust consent owner; SDK/audio/composer consumers not activated"})
    );
}
