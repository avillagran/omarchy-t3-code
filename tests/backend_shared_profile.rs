use omarchy_t3_client::backend_service;
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt};

/// O3 Code may register O3 Server and its native T3 fallback on one profile. The
/// registry stays valid and the shared runtime file names the server that runs.
#[test]
fn servers_sharing_a_profile_follow_the_runtime_that_is_there() {
    let root = std::env::temp_dir().join(format!("t3-shared-profile-{}", uuid::Uuid::new_v4()));
    let profile = root.join(".t3");
    fs::create_dir_all(profile.join("userdata")).unwrap();
    fs::create_dir_all(root.join(".config/omarchy-t3-code")).unwrap();
    std::env::set_var("HOME", &root);
    std::env::set_var("XDG_CONFIG_HOME", root.join(".config"));
    std::env::set_var("XDG_STATE_HOME", root.join(".local/state"));
    std::env::set_var("XDG_DATA_HOME", root.join(".local/share"));
    std::env::remove_var("T3CODE_HOME");
    fs::write(
        root.join(".config/omarchy-t3-code/backends.json"),
        json!({"version":1,"default":"o3","servers":[
            {"id":"o3","unit":"t3code.service","profile":profile,"cli":root.join("bin/o3-server")},
            {"id":"t3","unit":"o3-t3-server.service","profile":profile,"cli":root.join("bin/t3")}]})
        .to_string(),
    )
    .unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join(".config/systemd/user")).unwrap();
    for (cli, unit) in [
        ("o3-server", "t3code.service"),
        ("t3", "o3-t3-server.service"),
    ] {
        fs::write(root.join("bin").join(cli), "#!/bin/sh\nexit 1\n").unwrap();
        fs::set_permissions(
            root.join("bin").join(cli),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        fs::write(
            root.join(".config/systemd/user").join(unit),
            format!("[Service]\nEnvironment=T3CODE_HOME={}\n", profile.display()),
        )
        .unwrap();
    }
    let runtime = profile.join("userdata/server-runtime.json");
    let running = |status: &serde_json::Value, id: &str| {
        status["choices"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["backend"] == id)
            .unwrap()["running"]
            .clone()
    };

    let stopped = backend_service::status();
    assert_eq!(stopped["error"], "", "a shared profile is a valid registry");
    assert_eq!(stopped["backend"], "o3");
    assert_eq!(stopped["running"], false);

    let pid = std::process::id();
    fs::write(
        &runtime,
        json!({"pid":pid,"serverVersion":"o3-0.1.0"}).to_string(),
    )
    .unwrap();
    let o3 = backend_service::status();
    assert_eq!(o3["backend"], "o3");
    assert_eq!(o3["running"], true);
    assert_eq!(running(&o3, "o3"), true);
    assert_eq!(running(&o3, "t3"), false, "O3's runtime is not T3's");
    assert_eq!(
        backend_service::start().unwrap()["status"],
        "already-running",
        "starting the running server is not blocked by its rival"
    );

    fs::write(
        &runtime,
        json!({"pid":pid,"serverVersion":"0.1.6-nightly.20261030.3904"}).to_string(),
    )
    .unwrap();
    let t3 = backend_service::status();
    assert_eq!(running(&t3, "o3"), false);
    assert_eq!(running(&t3, "t3"), true);
    assert_eq!(
        backend_service::start_as("o3").unwrap_err(),
        "backend_already_running",
        "T3 serving the profile still blocks a second server"
    );
    fs::remove_file(&runtime).unwrap();
}
