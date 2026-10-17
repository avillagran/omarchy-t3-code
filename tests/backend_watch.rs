use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

#[test]
fn lifecycle_result_does_not_wait_for_remote_snapshot() {
    let root = std::env::temp_dir().join(format!("t3-backend-watch-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let token = root.join("token");
    fs::write(&token, "synthetic-token").unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let config = root.join("env.json");
    fs::write(&config, json!({"environments":[{"id":"fixture","name":"Fixture","token_file":token,"base_url":format!("http://{}",listener.local_addr().unwrap())}]}).to_string()).unwrap();
    let (accepted_tx, accepted_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        let mut socket = loop {
            if let Ok((socket, _)) = listener.accept() {
                break socket;
            }
            if std::time::Instant::now() >= deadline {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        };
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut buffer = [0; 8192];
        let _ = socket.read(&mut buffer);
        accepted_tx.send(()).unwrap();
        let _ = release_rx.recv_timeout(Duration::from_secs(5));
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_omarchy-t3-client"))
        .arg("watch")
        .env("HOME", root.join("home"))
        .env("T3CODE_HOME", root.join("native"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("OMARCHY_T3_FIXTURE_ENVIRONMENTS", config)
        .env_remove("KIMI_CODING_API_KEY")
        .env_remove("NOUS_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                let _ = tx.send(v);
            }
        }
    });
    let result = (|| {
        accepted_rx
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| "snapshot did not start")?;
        writeln!(
            input,
            "{}",
            json!({"op":"backend-control","action":"start","requestId":"lifecycle"})
        )
        .unwrap();
        loop {
            let event = rx
                .recv_timeout(Duration::from_secs(1))
                .map_err(|_| "lifecycle blocked behind snapshot")?;
            if event["type"] == "action" && event["data"]["requestId"] == "lifecycle" {
                return if event["data"]["error"] == "t3_cli_missing" {
                    Ok(())
                } else {
                    Err("missing lifecycle failure")
                };
            }
        }
    })();
    let _ = child.kill();
    let _ = child.wait();
    let _ = release_tx.send(());
    let _ = server.join();
    fs::remove_dir_all(root).unwrap();
    assert!(result.is_ok(), "{result:?}");
}
