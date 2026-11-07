use base64::Engine;
use omarchy_t3_client::{backend_service, refresh_connections};
use serde_json::json;
use std::os::fd::AsRawFd;
use std::{fs, os::unix::fs::PermissionsExt};
#[test]
fn service_start_respects_running_owner_and_profile() {
    let root = std::env::temp_dir().join(format!("t3-service-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join(".t3/userdata")).unwrap();
    fs::create_dir_all(root.join(".t3/bin")).unwrap();
    std::env::set_var("HOME", &root);
    std::env::set_var("XDG_CONFIG_HOME", root.join(".config"));
    std::env::set_var("XDG_STATE_HOME", root.join(".local/state"));
    std::env::set_var("XDG_DATA_HOME", root.join(".local/share"));
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
        fs::read_to_string(&log).unwrap().trim(),
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
    // O3 may replace the T3 unit through a drop-in with a different profile.
    // A second native T3 unit keeps the original history available without
    // rewriting the trial, its CLI wrapper, or the client's own profile.
    let o3 = root.join("o3-profile");
    for profile in [&o3, &root.join(".t3")] {
        fs::create_dir_all(profile.join("userdata")).unwrap();
    }
    let dropins = root.join(".config/systemd/user/t3code.service.d");
    fs::create_dir_all(&dropins).unwrap();
    fs::write(
        dropins.join("100-o3.conf"),
        format!("[Service]\nEnvironment=T3CODE_HOME={}\n", o3.display()),
    )
    .unwrap();
    fs::write(
        root.join(".config/systemd/user/o3-t3-server.service"),
        format!(
            "[Service]\nEnvironment=T3CODE_HOME={}\n",
            root.join(".t3").display()
        ),
    )
    .unwrap();
    let config = root.join(".config/omarchy-t3-code/backends.json");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    let registry = json!({"version":1,"default":"o3","servers":[
        {"id":"o3","unit":"t3code.service","profile":o3,"cli":cli},
        {"id":"t3","unit":"o3-t3-server.service","profile":root.join(".t3"),"cli":cli}
    ]});
    fs::write(&config, registry.to_string()).unwrap();
    std::env::set_var("T3_TEST_ROOT", &root);
    fs::write(&systemctl, r#"#!/bin/sh
printf '%s\n' "$*" >> "$T3_TEST_SYSTEMCTL_LOG"
[ "$2" = reset-failed ] && exit 0
if [ "$3" = t3code.service ]; then profile="$T3_TEST_ROOT/o3-profile"; else profile="$T3_TEST_ROOT/.t3"; fi
if [ "$2" = stop ]; then rm -f "$profile/userdata/server-runtime.json"; exit 0; fi
printf '{"pid":%s}' "$T3_TEST_PID" > "$profile/userdata/server-runtime.json"
"#).unwrap();
    fs::write(&log, "").unwrap();
    let lock = fs::OpenOptions::new()
        .write(true)
        .open(root.join(".local/state/omarchy-t3-code/backend-start.lock"))
        .unwrap();
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert_eq!(
        backend_service::start_as("o3").unwrap_err(),
        "backend_start_busy"
    );
    drop(lock);
    assert_eq!(backend_service::status()["backend"], "o3");
    assert_eq!(
        backend_service::status()["canControl"],
        true,
        "effective drop-in profile wins over base unit"
    );
    assert_eq!(
        backend_service::status()["choices"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        backend_service::start_as("o3").unwrap()["status"],
        "started"
    );
    assert_eq!(backend_service::local_profile(), o3);
    assert!(backend_service::local_token().ends_with("backend-o3.token"));
    let token = backend_service::local_token();
    fs::create_dir_all(token.parent().unwrap()).unwrap();
    let payload = json!({"exp":chrono::Utc::now().timestamp_millis()+86400000,"scopes":["orchestration:read","orchestration:operate","filesystem:read","settings:write","terminal:read","terminal:operate"]});
    fs::write(
        &token,
        format!(
            "{}.synthetic-signature",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string())
        ),
    )
    .unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(
        backend_service::local_profile().join("userdata/server-runtime.json"),
        json!({"pid":std::process::id(),"origin":"http://127.0.0.1:13773"}).to_string(),
    )
    .unwrap();
    refresh_connections();
    let route = omarchy_t3_client::environments()
        .unwrap()
        .into_iter()
        .find(|e| e.id == "local")
        .unwrap();
    assert_eq!(route.base_url, "http://127.0.0.1:13773");
    assert_eq!(route.token_file, token);

    assert_eq!(
        backend_service::start_as("t3").unwrap_err(),
        "backend_already_running"
    );
    assert_eq!(
        backend_service::status()["backend"],
        "o3",
        "blocked choice does not change active profile"
    );
    fs::remove_file(o3.join("userdata/server-runtime.json")).unwrap();
    assert_eq!(
        backend_service::start_as("t3").unwrap()["status"],
        "started"
    );
    assert_eq!(backend_service::status()["backend"], "t3");
    assert_eq!(backend_service::local_profile(), root.join(".t3"));
    assert!(backend_service::local_token().ends_with("backend-t3.token"));
    let token = backend_service::local_token();
    fs::create_dir_all(token.parent().unwrap()).unwrap();
    let payload = json!({"exp":chrono::Utc::now().timestamp_millis()+86400000,"scopes":["orchestration:read","orchestration:operate","filesystem:read","settings:write","terminal:read","terminal:operate"]});
    fs::write(
        &token,
        format!(
            "{}.synthetic-signature",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string())
        ),
    )
    .unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(
        backend_service::local_profile().join("userdata/server-runtime.json"),
        json!({"pid":std::process::id(),"origin":"http://127.0.0.1:23773"}).to_string(),
    )
    .unwrap();
    refresh_connections();
    let route = omarchy_t3_client::environments()
        .unwrap()
        .into_iter()
        .find(|e| e.id == "local")
        .unwrap();
    assert_eq!(route.base_url, "http://127.0.0.1:23773");
    assert_eq!(route.token_file, token);

    assert_eq!(std::env::var_os("T3CODE_HOME").unwrap(), root.join(".t3"));
    assert_eq!(fs::read_to_string(&log).unwrap(), "--user reset-failed t3code.service\n--user restart t3code.service\n--user reset-failed o3-t3-server.service\n--user restart o3-t3-server.service\n");
    // Switching a running server checks the target first, then stops the
    // running one and starts the chosen one.
    fs::write(&log, "").unwrap();
    assert_eq!(
        backend_service::switch_to("arbitrary.service").unwrap_err(),
        "backend_not_installed"
    );
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "",
        "an unknown target stops nothing"
    );
    assert_eq!(
        backend_service::switch_to("t3").unwrap()["status"],
        "already-running"
    );
    // A T3 runtime the user started by hand in its own profile.
    let mut manual = std::process::Command::new("sh")
        .args(["-c", "sleep 30; true", "sh", "/opt/t3/apps/server/bin.mjs"])
        .env("T3CODE_HOME", root.join(".t3"))
        .spawn()
        .unwrap();
    fs::write(&runtime, json!({"pid":manual.id()}).to_string()).unwrap();
    assert_eq!(backend_service::status()["owner"], "headless-manual");
    assert_eq!(
        backend_service::switch_to("o3").unwrap()["status"],
        "started"
    );
    assert!(manual.try_wait().unwrap().is_some(), "the previous server was stopped");
    assert_eq!(backend_service::status()["backend"], "o3");
    assert_eq!(
        fs::read_to_string(&log).unwrap(),
        "--user reset-failed t3code.service\n--user restart t3code.service\n"
    );
    fs::remove_file(&runtime).unwrap();
    fs::remove_file(o3.join("userdata/server-runtime.json")).unwrap();
    std::env::set_var("T3_TEST_NOT_READY", "1");
    assert_eq!(
        backend_service::start_as("o3").unwrap_err(),
        "service_start_failed"
    );
    assert_eq!(
        backend_service::status()["backend"],
        "o3",
        "failed O3 readiness never falls back to T3"
    );
    std::env::remove_var("T3_TEST_NOT_READY");
    fs::remove_file(o3.join("userdata/server-runtime.json")).unwrap();
    assert_eq!(
        backend_service::start_as("arbitrary.service").unwrap_err(),
        "backend_not_installed"
    );
    let mut invalid = registry.clone();
    invalid["servers"][0]["unit"] = json!("evil;echo.service");
    fs::write(&config, invalid.to_string()).unwrap();
    assert_eq!(backend_service::status()["canControl"], false);
    assert_eq!(
        backend_service::start().unwrap_err(),
        "backend_configuration_invalid"
    );
    std::env::set_var("PATH", old_path);
    refresh_connections();

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
