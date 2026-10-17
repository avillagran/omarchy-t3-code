use omarchy_t3_client::{backend_service, refresh_connections};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt};
#[test]
fn service_start_respects_running_owner_and_profile() {
    let root = std::env::temp_dir().join(format!("t3-service-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join(".t3/userdata")).unwrap();
    fs::create_dir_all(root.join(".t3/bin")).unwrap();
    std::env::set_var("HOME", &root);
    std::env::set_var("T3CODE_HOME", root.join(".t3"));
    let runtime = root.join(".t3/userdata/server-runtime.json");
    fs::write(&runtime, json!({"pid":std::process::id()}).to_string()).unwrap();
    assert_eq!(
        backend_service::start().unwrap()["status"],
        "already-running"
    );
    let owner = backend_service::status()["owner"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(matches!(owner.as_str(), "gui" | "unknown"));
    let blocked = if owner == "gui" {
        "backend_owned_by_gui"
    } else {
        "backend_owner_unknown"
    };
    assert_eq!(backend_service::control("stop").unwrap_err(), blocked);
    assert_eq!(backend_service::control("restart").unwrap_err(), blocked);
    fs::remove_file(&runtime).unwrap();
    std::env::set_var("T3_TEST_PID", std::process::id().to_string());
    let cli = root.join(".t3/bin/t3");
    fs::write(&cli, r#"#!/bin/sh
if [ "$1" = __ssh-helper ]; then
  if [ "$2" = runtime-port ]; then printf '%s 1234\n' "$T3_TEST_PID"; exit 0; fi
  [ "$2" = wait-ready ] && [ "$3" = 1234 ] && [ "$4" = 15000 ] && [ -z "$T3_TEST_NOT_READY" ]; exit $?
fi
[ "$1" = service ] && [ "$2" = install ] && [ "$3" = --base-dir ] && [ "$4" = "$HOME/.t3" ] || exit 1
printf '{"pid":%s}' "$T3_TEST_PID" > "$HOME/.t3/userdata/server-runtime.json"
"#).unwrap();
    fs::set_permissions(&cli, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(backend_service::start().unwrap()["status"], "started");
    fs::remove_file(&runtime).unwrap();
    fs::create_dir_all(root.join(".config/systemd/user")).unwrap();
    fs::write(
        root.join(".config/systemd/user/t3code.service"),
        "Environment=T3CODE_HOME=/other/profile\n",
    )
    .unwrap();
    assert_eq!(
        backend_service::start().unwrap_err(),
        "service_profile_mismatch"
    ); // A matching native unit is started through systemd; never reinstalled/repointed.
    let log = root.join("systemctl.log");
    std::env::set_var("T3_TEST_SYSTEMCTL_LOG", &log);
    fs::create_dir_all(root.join("test-bin")).unwrap();
    let systemctl = root.join("test-bin/systemctl");
    fs::write(
        &systemctl,
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$T3_TEST_SYSTEMCTL_LOG"
printf '{"pid":%s}' "$T3_TEST_PID" > "$HOME/.t3/userdata/server-runtime.json"
"#,
    )
    .unwrap();
    fs::set_permissions(&systemctl, fs::Permissions::from_mode(0o700)).unwrap();
    let old_path = std::env::var("PATH").unwrap();
    std::env::set_var(
        "PATH",
        format!("{}:{}", root.join("test-bin").display(), old_path),
    );
    fs::write(
        root.join(".config/systemd/user/t3code.service"),
        format!(
            "Environment=T3CODE_HOME=\"{}\"\n",
            root.join(".t3").display()
        ),
    )
    .unwrap();
    assert_eq!(backend_service::start().unwrap()["status"], "started");
    assert_eq!(
        fs::read_to_string(log).unwrap().trim(),
        "--user restart t3code.service"
    );
    // Successful systemd activation is insufficient when native readiness fails.
    fs::remove_file(&runtime).unwrap();
    std::env::set_var("T3_TEST_NOT_READY", "1");
    assert_eq!(
        backend_service::start().unwrap_err(),
        "service_start_failed"
    );
    std::env::remove_var("T3_TEST_NOT_READY");
    fs::remove_file(&runtime).unwrap();
    fs::write(&systemctl, "#!/bin/sh\nexit 0\n").unwrap();
    assert_eq!(
        backend_service::start().unwrap_err(),
        "service_start_failed"
    );
    std::env::set_var("PATH", old_path);
    refresh_connections();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ownership_distinguishes_gui_service_and_verified_manual_runtime() {
    assert_eq!(backend_service::classify(false, true, false), "gui");
    assert_eq!(backend_service::classify(true, true, false), "headless");
    assert_eq!(
        backend_service::classify(false, false, true),
        "headless-manual"
    );
    assert_eq!(backend_service::classify(false, false, false), "unknown");
}
