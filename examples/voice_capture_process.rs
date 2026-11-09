//! Actual Rust owner/private renderer acceptance with a synthetic audio device.
use omarchy_t3_client::{
    browser_native_display::Display,
    voice_capture::{
        resources::{Assets, DescriptorArtifact, Runtime},
        Capture, Cleanup, Event, Mode,
    },
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
fn event(owner: &mut Capture) -> Event {
    let end = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(Instant::now() < end);
        if let Some(event) = owner.next(Duration::from_millis(100)).unwrap() {
            return event;
        }
    }
}
fn result(owner: &mut Capture, id: u64, bytes: &mut usize) {
    loop {
        match event(owner) {
            Event::Pcm(pcm) => *bytes += pcm.len(),
            Event::Result {
                id: received,
                ok,
                error,
            } if received == id => {
                assert!(ok, "{error:?}");
                return;
            }
            _ => {}
        }
    }
}
fn main() {
    assert!(Path::new("/run/.containerenv").exists());
    fs::create_dir_all("/tmp/.X11-unix").unwrap();
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .mode(0o700)
        .create("/state/runtime")
        .unwrap();
    std::env::set_var("XDG_RUNTIME_DIR", "/state/runtime");
    std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/state/no-bus");
    let runtime = Arc::new(Runtime::admit(Path::new("/electron")).unwrap());
    let descriptor = PathBuf::from("/source/electron-private-fd.node");
    let receipt = DescriptorArtifact {
        sha256: format!("{:x}", Sha256::digest(fs::read(&descriptor).unwrap())),
        bytes: fs::metadata(&descriptor).unwrap().len(),
        path: descriptor,
    };
    let assets = Arc::new(Assets::capture(Path::new("/state/assets"), &receipt).unwrap());
    let display = Arc::new(Display::launch(Path::new("/usr/bin/Xvfb")).unwrap());
    let mut rows = vec![];
    for case in [
        "streaming",
        "buffered",
        "denied",
        "cancel-permission",
        "cancel-recording",
        "cancel-buffered-stop",
        "drop",
    ] {
        let mode = if case == "buffered" || case == "cancel-buffered-stop" {
            Mode::Buffered
        } else {
            Mode::Streaming
        };
        let mut owner = Capture::synthetic(
            runtime.clone(),
            assets.clone(),
            &Path::new("/state").join(case),
            display.clone(),
            mode,
            "",
        )
        .unwrap();
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, owner.process_id(), 0) };
        assert!(raw >= 0);
        let observed = unsafe { OwnedFd::from_raw_fd(raw as i32) };
        let mut bytes = 0;
        if case == "drop" {
            drop(owner)
        } else {
            let permission = loop {
                if let Event::Permission { request_id } = event(&mut owner) {
                    break request_id;
                }
            };
            if case == "cancel-permission" {
                let cancel = owner.cancellation();
                std::thread::spawn(move || cancel.cancel()).join().unwrap();
            } else {
                assert!(owner.permission("foreign-permission", true).is_err());
                let decision = owner.permission(&permission, case != "denied").unwrap();
                result(&mut owner, decision, &mut bytes);
                if case == "denied" {
                    loop {
                        if let Event::Result { id: 1, ok, error } = event(&mut owner) {
                            assert!(!ok);
                            assert_eq!(error.as_deref(), Some("capture_permission_denied"));
                            break;
                        }
                    }
                } else {
                    result(&mut owner, 1, &mut bytes);
                    let record = owner.record().unwrap();
                    result(&mut owner, record, &mut bytes);
                    if mode == Mode::Buffered {
                        let end = Instant::now() + Duration::from_millis(1300);
                        while Instant::now() < end {
                            assert!(!matches!(
                                owner.next(Duration::from_millis(100)).unwrap(),
                                Some(Event::Pcm(_))
                            ))
                        }
                    } else {
                        while bytes < 96000 {
                            if let Event::Pcm(pcm) = event(&mut owner) {
                                bytes += pcm.len()
                            }
                        }
                    }
                    if case == "cancel-buffered-stop" {
                        owner.stop().unwrap();
                        // Witness a real buffered frame before cancellation;
                        // the remaining frames and stop result are already in
                        // flight and must be discarded without losing close.
                        loop {
                            if let Event::Pcm(pcm) = event(&mut owner) {
                                assert_eq!(pcm.len(), 32000);
                                bytes += pcm.len();
                                break;
                            }
                        }
                        let cancel = owner.cancellation();
                        std::thread::spawn(move || cancel.cancel()).join().unwrap();
                    } else if case == "cancel-recording" {
                        let cancel = owner.cancellation();
                        std::thread::spawn(move || cancel.cancel()).join().unwrap();
                    } else {
                        let stop = owner.stop().unwrap();
                        result(&mut owner, stop, &mut bytes);
                        assert!(bytes > 0)
                    }
                }
            }
            assert_eq!(owner.close(), Cleanup::Confirmed);
            assert_eq!(owner.close(), Cleanup::Confirmed);
        }
        let mut wait = libc::pollfd {
            fd: observed.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(unsafe { libc::poll(&mut wait, 1, 1000) }, 1);
        rows.push(serde_json::json!({"case":case,"pcmBytes":bytes,"leaderExitObserved":true}));
    }
    runtime.verify().unwrap();
    assets.verify().unwrap();
    use std::os::unix::fs::PermissionsExt;
    let bad = Path::new("/state/owned-invalid-helper.node");
    let bytes = b"retained invalid native addon fixture";
    fs::write(bad, bytes).unwrap();
    fs::set_permissions(bad, fs::Permissions::from_mode(0o400)).unwrap();
    let receipt = DescriptorArtifact {
        path: bad.into(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
        bytes: bytes.len() as u64,
    };
    let invalid = Arc::new(Assets::capture(Path::new("/state/invalid-assets"), &receipt).unwrap());
    let failure = match Capture::synthetic(
        runtime,
        invalid,
        Path::new("/state/invalid-startup"),
        display,
        Mode::Streaming,
        "",
    ) {
        Ok(_) => panic!("Invalid helper admitted"),
        Err(error) => error,
    };
    assert_eq!(
        failure
            .get_ref()
            .unwrap()
            .downcast_ref::<omarchy_t3_client::voice_capture::StartupFailure>()
            .unwrap()
            .cleanup,
        Cleanup::Unknown
    );
    rows.push(serde_json::json!({"case":"failed-startup","pcmBytes":0,"cleanup":"unknown"}));
    println!(
        "{}",
        serde_json::json!({"cases":rows,"actualRustSupervisor":true,"actualNativeProcess":true,"syntheticDeviceOnly":true,"sdkOrConsentActivated":false})
    );
}
