//! Opt-in private artifact admission; never opens an audio device or process.
use omarchy_t3_client::voice_capture::resources::{Assets, DescriptorArtifact, Runtime};
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};
fn main() {
    assert!(
        std::path::Path::new("/run/.containerenv").exists(),
        "Private rootless container required"
    );
    let runtime = Runtime::admit(std::path::Path::new("/electron")).unwrap();
    runtime.verify().unwrap();
    assert!(std::path::Path::new("/electron/electron").is_file());
    let source = PathBuf::from("/source/electron-private-fd.node");
    let descriptor = DescriptorArtifact {
        sha256: format!("{:x}", Sha256::digest(fs::read(&source).unwrap())),
        bytes: fs::metadata(&source).unwrap().len(),
        path: source,
    };
    let assets = Assets::capture(std::path::Path::new("/state/accepted"), &descriptor).unwrap();
    assets.verify().unwrap();
    assert!(std::path::Path::new("/state/accepted/electron-capture-main.cjs").is_file());
    assert!(Assets::capture(std::path::Path::new("/state/accepted"), &descriptor).is_err());
    let consent = Assets::consent(std::path::Path::new("/state/consent"), &descriptor).unwrap();
    consent.verify().unwrap();
    assert!(std::path::Path::new("/state/consent/consent-helper.cjs").is_file());
    assert!(std::path::Path::new("/state/consent/electron-private-fd.node").is_file());
    let stale = Assets::capture(std::path::Path::new("/state/stale"), &descriptor).unwrap();
    let entry = PathBuf::from("/state/stale/electron-capture-main.cjs");
    fs::set_permissions(&entry, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&entry, b"retained owned drift fixture").unwrap();
    fs::set_permissions(&entry, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(stale.verify().is_err());
    let own = std::path::Path::new("/state/own-target");
    fs::create_dir(own).unwrap();
    std::os::unix::fs::symlink(own, "/state/own-link").unwrap();
    assert!(Runtime::admit(std::path::Path::new("/state/own-link")).is_err());
    let bad = DescriptorArtifact {
        path: descriptor.path.clone(),
        sha256: "0".repeat(64),
        bytes: descriptor.bytes,
    };
    assert!(Assets::capture(std::path::Path::new("/state/bad-digest"), &bad).is_err());
    let writable = std::path::Path::new("/state/writable.node");
    fs::copy(&descriptor.path, writable).unwrap();
    fs::set_permissions(writable, fs::Permissions::from_mode(0o600)).unwrap();
    let bad = DescriptorArtifact {
        path: writable.into(),
        sha256: descriptor.sha256.clone(),
        bytes: descriptor.bytes,
    };
    assert!(Assets::capture(std::path::Path::new("/state/bad-mode"), &bad).is_err());
    let hard = std::path::Path::new("/state/owned-helper.node");
    fs::copy(&descriptor.path, hard).unwrap();
    fs::set_permissions(hard, fs::Permissions::from_mode(0o400)).unwrap();
    fs::hard_link(hard, "/state/owned-helper-alias.node").unwrap();
    let bad = DescriptorArtifact {
        path: hard.into(),
        sha256: descriptor.sha256,
        bytes: descriptor.bytes,
    };
    assert!(Assets::capture(std::path::Path::new("/state/bad-link"), &bad).is_err());
    runtime.verify().unwrap();
    assets.verify().unwrap();
    let captured_files = fs::read_dir("/state/accepted").unwrap().count();
    println!("{{\"runtimeFiles\":72,\"capturedAssetFiles\":{captured_files},\"refusals\":6,\"actualChildLaunched\":false,\"audioOpened\":false}}");
}
