//! Local shared-profile handover never discards old webhook work or duplicate protection.
use omarchy_t3_client::backend_service;
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::{Child, Command},
    time::{Duration, Instant},
};

struct Peer(Child);
impl Drop for Peer {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
fn executable(path: &std::path::Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
#[test]
fn legacy_and_unknown_readiness_stop_nothing_and_the_shutdown_window_is_rechecked() {
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let root = std::env::temp_dir().join(format!("o3-handover-{}", uuid::Uuid::new_v4()));
    let profile = root.join("profile");
    fs::create_dir_all(profile.join("userdata")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join(".config/omarchy-t3-code")).unwrap();
    fs::create_dir_all(root.join(".config/systemd/user")).unwrap();
    for (key, path) in [
        ("HOME", root.clone()),
        ("XDG_CONFIG_HOME", root.join(".config")),
        ("XDG_STATE_HOME", root.join("state")),
        ("XDG_DATA_HOME", root.join("data")),
    ] {
        std::env::set_var(key, path);
    }
    std::env::set_current_dir(&root).unwrap();
    let node = Command::new("node")
        .args(["-p", "process.execPath"])
        .output()
        .unwrap();
    assert!(node.status.success());
    let node = fs::canonicalize(String::from_utf8(node.stdout).unwrap().trim()).unwrap();
    let report = root.join("report.json");
    let log = root.join("effects.log");
    let runtime = profile.join("userdata/server-runtime.json");
    std::env::set_var("TEST_HANDOVER_PROFILE", &profile);
    std::env::set_var("TEST_HANDOVER_REPORT", &report);
    std::env::set_var("TEST_HANDOVER_LOG", &log);
    std::env::set_var("TEST_HANDOVER_ROOT", &root);
    std::env::set_var("TEST_HANDOVER_PID", std::process::id().to_string());
    fs::write(&log, "").unwrap();
    fs::write(root.join("shared-profile-readiness"),r#"
const fs=require('node:fs');
if(JSON.stringify(process.argv.slice(2))!==JSON.stringify(['--base-dir',process.env.TEST_HANDOVER_PROFILE]))process.exit(2);
process.stdout.write(fs.readFileSync(process.env.TEST_HANDOVER_REPORT));
"#).unwrap();
    fs::write(root.join("__ssh-helper"),r#"
const fs=require('node:fs');
if(process.argv[2]==='runtime-port'){const value=JSON.parse(fs.readFileSync(process.argv[3]));console.log(value.pid+' 1234');}
else if(process.argv[2]!=='wait-ready')process.exit(2);
"#).unwrap();
    executable(
        &root.join("bin/systemctl"),
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$TEST_HANDOVER_LOG"
[ "$2" = reset-failed ] && exit 0
[ "$2" = restart ] || exit 2
if [ "$3" = t3code.service ]; then version=o3-fixture; else version=t3-fixture; fi
printf '{"pid":%s,"serverVersion":"%s"}' "$TEST_HANDOVER_PID" "$version" > "$TEST_HANDOVER_PROFILE/userdata/server-runtime.json"
"#,
    );
    let old_path = std::env::var("PATH").unwrap();
    std::env::set_var("PATH", format!("{}:{old_path}", root.join("bin").display()));
    for unit in ["t3code.service", "o3-t3-server.service"] {
        fs::write(
            root.join(".config/systemd/user").join(unit),
            format!("[Service]\nEnvironment=T3CODE_HOME={}\n", profile.display()),
        )
        .unwrap();
    }
    fs::write(root.join(".config/omarchy-t3-code/backends.json"),json!({"version":1,"default":"o3","servers":[{"id":"o3","unit":"t3code.service","profile":profile,"cli":node},{"id":"t3","unit":"o3-t3-server.service","profile":profile,"cli":node}]}).to_string()).unwrap();
    let mut peer=Peer(Command::new(&node).args(["-e",r#"
const fs=require('node:fs'),p=process.env.TEST_HANDOVER_PROFILE,r=process.env.TEST_HANDOVER_ROOT;
process.on('SIGTERM',()=>{if(fs.existsSync(r+'/inject-on-stop'))fs.writeFileSync(process.env.TEST_HANDOVER_REPORT,JSON.stringify({version:1,legacyWebhookReady:false,pendingLegacyDeliveries:0,retainedLegacyReplayClaims:1}));process.exit(0)});
fs.writeFileSync(p+'/userdata/server-runtime.json',JSON.stringify({pid:process.pid,serverVersion:'o3-fixture'}));
fs.writeFileSync(r+'/peer-ready','ready');setInterval(()=>{},1000);
"#]).env("T3CODE_HOME",&profile).spawn().unwrap());
    let until = Instant::now() + Duration::from_secs(5);
    while !root.join("peer-ready").is_file() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(backend_service::status()["owner"], "headless-manual");
    for (pending, replay) in [(1, 0), (0, 1)] {
        fs::write(&report,json!({"version":1,"legacyWebhookReady":false,"pendingLegacyDeliveries":pending,"retainedLegacyReplayClaims":replay}).to_string()).unwrap();
        assert_eq!(
            backend_service::switch_to("t3").unwrap_err(),
            "backend_legacy_webhook_handover_blocked"
        );
        assert!(peer.0.try_wait().unwrap().is_none());
        assert_eq!(fs::read_to_string(&log).unwrap(), "");
    }
    fs::write(&report,json!({"version":1,"legacyWebhookReady":true,"pendingLegacyDeliveries":0,"retainedLegacyReplayClaims":1}).to_string()).unwrap();
    assert_eq!(
        backend_service::switch_to("t3").unwrap_err(),
        "backend_handover_readiness_unavailable"
    );
    assert!(peer.0.try_wait().unwrap().is_none());
    // Simulate an older O3 admitting work after the preflight but before it stops.
    fs::write(&report,json!({"version":1,"legacyWebhookReady":true,"pendingLegacyDeliveries":0,"retainedLegacyReplayClaims":0}).to_string()).unwrap();
    fs::write(root.join("inject-on-stop"), "").unwrap();
    assert_eq!(
        backend_service::switch_to("t3").unwrap_err(),
        "backend_legacy_webhook_handover_blocked"
    );
    peer.0.wait().unwrap();
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "",
        "the rival server was never started"
    );
    assert_eq!(
        backend_service::start_as("t3").unwrap_err(),
        "backend_legacy_webhook_handover_blocked"
    );
    assert_eq!(backend_service::status()["backend"], "o3");
    // O3 remains available to preserve/reconcile its old data.
    assert_eq!(
        backend_service::start_as("o3").unwrap()["status"],
        "started"
    );
    fs::remove_file(&runtime).unwrap();
    fs::write(&report,json!({"version":1,"legacyWebhookReady":true,"pendingLegacyDeliveries":0,"retainedLegacyReplayClaims":0}).to_string()).unwrap();
    assert_eq!(
        backend_service::start_as("t3").unwrap()["status"],
        "started"
    );
    assert_eq!(backend_service::status()["backend"], "t3");
}
