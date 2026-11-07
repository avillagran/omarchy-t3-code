use omarchy_t3_client::new_thread_models;
use serde_json::json;
#[test]
fn fallback_selection_context_exposes_only_needed_native_configuration() {
    let config = json!({"settings":{"fallbackModelSelection":{"instanceId":"backup","model":"m","secret":"PRIVATE"},"secret":"PRIVATE","providerInstances":{"backup":{"driver":"claudeAgent","enabled":true,"secret":"PRIVATE","config":{"enabled":false,"token":"PRIVATE"}}},"projectSettingsOverrides":{"p":{"providerInstances":{"backup":{"enabled":true,"token":"PRIVATE"}},"secret":"PRIVATE"},"other":{"secret":"PRIVATE"}}},"providers":[{"instanceId":"backup","driver":"claudeAgent","enabled":true,"installed":true,"availability":"available","displayName":"Backup","auth":{"status":"authenticated","token":"PRIVATE"},"config":{"secret":"PRIVATE"},"usageLimits":{"secret":"PRIVATE","checkedAt":"2026-11-07T12:00:00Z","windows":[]}}]});
    let value = new_thread_models::present(&config, "p");
    assert!(!value.to_string().contains("PRIVATE"));
    assert!(value["settings"]["projectSettingsOverrides"]
        .get("other")
        .is_none());
    assert_eq!(
        value["fallbackSelection"],
        json!({"instanceId":"backup","model":"m"})
    );
    assert_eq!(
        value["settings"]["providerInstances"]["backup"],
        json!({"driver":"claudeAgent","enabled":true,"config":{"enabled":false}})
    );
    assert_eq!(
        value["settings"]["projectSettingsOverrides"]["p"]["providerInstances"]["backup"],
        json!({"enabled":true})
    );
    assert_eq!(
        value["providers"][0]["auth"],
        json!({"status":"authenticated"})
    );
    assert_eq!(value["providers"][0]["displayName"], "Backup");
    assert!(new_thread_models::present(&json!({}), "p")["fallbackSelection"].is_null());
}

#[test]
fn actual_workspace_read_uses_one_latest_config_for_default_and_fallback() {
    use serde_json::Value;
    use std::{
        fs,
        net::TcpListener,
        os::unix::fs::PermissionsExt,
        thread,
        time::{Duration, Instant},
    };
    assert!(std::path::Path::new("/run/.containerenv").is_file());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let config = json!({"settings":{"defaultThreadEnvMode":"local","defaultModelSelection":{"instanceId":"primary","model":"current"},"fallbackModelSelection":{"instanceId":"backup","model":"fallback"},"providerInstances":{"primary":{"driver":"codex","enabled":true},"backup":{"driver":"claudeAgent","enabled":true}},"projectSettingsOverrides":{},"secret":"PRIVATE"},"providers":[{"instanceId":"primary","driver":"codex","enabled":true,"installed":true,"availability":"available","auth":{"status":"authenticated","token":"PRIVATE"},"usageLimits":{"windows":[{"id":"session","usedPercent":100,"resetsAt":"2026-11-07T13:00:00Z"}],"secret":"PRIVATE"}},{"instanceId":"backup","driver":"claudeAgent","enabled":true,"installed":true,"availability":"available","auth":{"status":"authenticated"},"usageLimits":{"windows":[]}}]});
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut calls = Vec::new();
        for _ in 0..2 {
            let socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut header = [0; 8192];
            loop {
                let n = socket.peek(&mut header).unwrap();
                if header[..n].windows(4).any(|v| v == b"\r\n\r\n") {
                    assert!(String::from_utf8_lossy(&header[..n])
                        .to_ascii_lowercase()
                        .contains("authorization: bearer write-only"));
                    break;
                }
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(1))
            }
            let mut ws = tungstenite::accept(socket).unwrap();
            loop {
                match ws.read() {
                    Ok(tungstenite::Message::Text(text)) => {
                        let request: Value = serde_json::from_str(&text).unwrap();
                        if request["_tag"] == "Ping" {
                            ws.send(tungstenite::Message::Text(
                                json!({"_tag":"Pong"}).to_string().into(),
                            ))
                            .unwrap();
                            continue;
                        }
                        assert_eq!(request["_tag"], "Request");
                        let value = match request["tag"].as_str().unwrap() {
                            "server.getSettings" => {
                                json!({"defaultThreadEnvMode":"worktree","defaultModelSelection":{"instanceId":"obsolete","model":"stale"}})
                            }
                            "server.getConfig" => config.clone(),
                            other => panic!("unexpected RPC {other}"),
                        };
                        calls.push(request["tag"].clone());
                        ws.send(tungstenite::Message::Text(json!({"_tag":"Exit","requestId":request["id"],"exit":{"_tag":"Success","value":value}}).to_string().into())).unwrap();
                        break;
                    }
                    Ok(tungstenite::Message::Ping(value)) => {
                        ws.send(tungstenite::Message::Pong(value)).unwrap()
                    }
                    other => panic!("{other:?}"),
                }
            }
        }
        calls
    });
    let root = std::env::temp_dir().join(format!("new-thread-model-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let token = root.join("write-token");
    fs::write(&token, "write-only").unwrap();
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let fixture = root.join("environments.json");
    fs::write(&fixture,json!({"environments":[{"id":"fixture","base_url":base,"token_file":token,"write_token_file":token}]}).to_string()).unwrap();
    std::env::set_var("OMARCHY_T3_FIXTURE_ENVIRONMENTS", fixture);
    let result=omarchy_t3_client::write::execute(&json!({"op":"settings","action":"workspace-read","environment":"fixture","projectId":"p","modelDefaults":true})).unwrap();
    assert_eq!(result["mode"], "local");
    assert_eq!(result["defaultModelSelection"]["instanceId"], "primary");
    assert_eq!(
        result["modelDefaults"]["fallbackSelection"]["instanceId"],
        "backup"
    );
    assert!(!result.to_string().contains("PRIVATE"));
    assert_eq!(
        server.join().unwrap(),
        [json!("server.getSettings"), json!("server.getConfig")]
    );
    // All fixture files remain inside the private test container.
}
