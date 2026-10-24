// Compiles the new module in isolation; no src/lib.rs integration is required.
pub use omarchy_t3_client::Url;
#[path = "../src/connection_cloud.rs"]
mod connection_cloud;
use connection_cloud::{device_prompt, parse_status, CloudManager, NativeCli};
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

const STATUS: &str = r#"{"desired":true,"authenticated":true,"linked":false,"revoked":false,"cloudUserId":null,"relayUrl":"https://relay.example","publishAgentActivity":false,"relayClient":{"status":"available","executablePath":"hidden"},"accessToken":"fixture-secret"}"#;
const PROMPT: &str = "Headless authorization\nOpen this URL on a device with a browser:\n  https://accounts.example.test/device?user_code=BCDF-GHJK\n\nConfirm this code when asked: BCDF-GHJK\n\nWaiting for approval (expires in 10 min). Press Ctrl+C to cancel.\n";
struct Fixture {
    root: PathBuf,
    cli: NativeCli,
}
impl Fixture {
    fn new(mode: &str) -> Self {
        let root = std::env::temp_dir().join(format!("ot3-cloud-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let executable = root.join("native-fixture");
        let source = format!(
            r##"#!/usr/bin/python3
import sys, pathlib, json, time, subprocess, signal, os
args=sys.argv[1:]
base=pathlib.Path(args[args.index('--base-dir')+1])
with (base/'argv').open('a') as f: f.write(json.dumps(args)+'\n')
if args[1]=='status':
    if (base/'mode').read_text()=='bad-status':
        print('unsupported status fixture-secret',flush=True)
        sys.exit(0)
    print({status:?}, flush=True)
    sys.exit(0)
mode=(base/'mode').read_text()
if mode=='install':
    print('The T3 relay client is required for T3 Connect. Download and install version 1.0?',flush=True)
    reply=sys.stdin.readline()
    if not reply.startswith('y'): sys.exit(3)
    print('Relay client: Verifying download...',flush=True)
if mode=='warning':
    print('Could not revoke the relay-side environment record yet.',flush=True)
    sys.exit(0)
if mode=='denied':
    print('T3 Connect authorization was denied in the browser.',flush=True)
    sys.exit(2)
if mode=='expired':
    print('Timed out waiting for T3 Connect authorization.',flush=True)
    sys.exit(2)
if mode=='failure':
    print('access_token=fixture-secret',flush=True)
    sys.exit(3)
print({prompt:?},flush=True)
print('access_token=fixture-secret',flush=True)
if mode in ('hang','flood'):
    signal.signal(signal.SIGINT,signal.SIG_IGN)
    child=subprocess.Popen([sys.executable,'-c','import signal,time; signal.signal(signal.SIGINT, signal.SIG_IGN); time.sleep(60)'])
    (base/'descendant').write_text(str(child.pid))
    if mode=='flood':
        while True: print('x'*4096,flush=True)
    time.sleep(60)
time.sleep(0.1)
"##,
            status = STATUS,
            prompt = PROMPT
        );
        fs::write(&executable, source).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root.join("mode"), mode).unwrap();
        Self {
            cli: NativeCli::new(executable, root.clone()),
            root,
        }
    }
    fn start(&self, manager: &CloudManager, action: &str) -> mpsc::Receiver<Value> {
        let (tx, rx) = mpsc::channel();
        manager
            .start(
                self.cli.clone(),
                "fixture-request",
                action,
                true,
                move |v| {
                    tx.send(v).unwrap();
                },
            )
            .unwrap();
        rx
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn finish(rx: &mpsc::Receiver<Value>) -> (Value, Vec<Value>) {
    let mut events = vec![];
    loop {
        let event = rx.recv_timeout(Duration::from_secs(8)).unwrap();
        assert!(!event.to_string().contains("fixture-secret"));
        if event["busy"] == false {
            return (event, events);
        }
        events.push(event);
    }
}
#[test]
fn exact_cli_flags_and_validation() {
    let f = Fixture::new("normal");
    assert_eq!(
        f.cli.arguments("read").unwrap(),
        vec![
            "connect",
            "status",
            "--base-dir",
            f.root.to_str().unwrap(),
            "--json"
        ]
    );
    assert!(f
        .cli
        .arguments("register")
        .unwrap()
        .ends_with(&["--headless".into()]));
    assert_eq!(f.cli.arguments("register").unwrap()[1], "login");
    assert!(f
        .cli
        .arguments("link-publish-only")
        .unwrap()
        .ends_with(&["--headless".into(), "--publish-only".into()]));
    assert!(f.cli.arguments("serve").is_err());
    let manager = CloudManager::default();
    assert!(manager
        .start(f.cli.clone(), "id", "logout", false, |_| {})
        .is_err());
    assert!(manager
        .start(f.cli.clone(), "", "login", true, |_| {})
        .is_err());
    let cli = NativeCli::new(f.cli.executable.clone(), "relative".into());
    assert!(cli.arguments("read").is_err());
}
#[test]
fn status_is_strict_and_credential_fields_never_cross_ipc() {
    let v = parse_status(STATUS).unwrap();
    assert_eq!(v["authenticated"], true);
    assert_eq!(v["savedSetupOnly"], true);
    assert!(!v.to_string().contains("fixture-secret"));
    assert!(v.get("relayUrl").is_none());
    assert!(v["relayClient"].get("executablePath").is_none());
    let legacy = STATUS.replace("\"revoked\":false,", "");
    assert!(parse_status(&legacy).unwrap()["revoked"].is_null());
    assert!(parse_status(&STATUS.replace("\"revoked\":false", "\"revoked\":\"false\"")).is_err());
    assert!(parse_status("{\"authenticated\":true}").is_err());
    assert!(parse_status(&STATUS.replace("true", "\"true\"")).is_err());
}
#[test]
fn exact_device_prompt_strips_ansi_and_rejects_unsafe_urls() {
    assert_eq!(
        device_prompt(&format!("\x1b[32m{PROMPT}\x1b[0m")).unwrap()["userCode"],
        "BCDF-GHJK"
    );
    for bad in [
        PROMPT.replace("https://", "file:///"),
        PROMPT.replace("?user_code=", "?access_token="),
        PROMPT.replace(
            "accounts.example.test",
            "user:password@accounts.example.test",
        ),
        PROMPT.replace("BCDF-GHJK", "<img src=x>"),
    ] {
        assert!(device_prompt(&bad).is_none());
    }
    assert!(device_prompt("access_token=fixture-secret").is_none());
}
#[test]
fn asynchronous_device_flow_and_post_mutation_readback() {
    let f = Fixture::new("normal");
    let manager = CloudManager::default();
    let started = Instant::now();
    let rx = f.start(&manager, "register");
    assert!(started.elapsed() < Duration::from_millis(100));
    let (v, events) = finish(&rx);
    assert_eq!(v["error"], "");
    assert_eq!(v["status"]["authenticated"], true);
    assert!(v["device"].is_null());
    assert!(events
        .iter()
        .any(|v| v["stage"] == "authorization" && v["data"]["userCode"] == "BCDF-GHJK"));
    let args: Vec<Value> = fs::read_to_string(f.root.join("argv"))
        .unwrap()
        .lines()
        .map(|v| serde_json::from_str(v).unwrap())
        .collect();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0][1], "login");
    assert_eq!(args[1][1], "status");
}
#[test]
fn relay_install_confirmation_uses_native_terminal() {
    let f = Fixture::new("install");
    let manager = CloudManager::default();
    let rx = f.start(&manager, "link");
    loop {
        let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if event["stage"] == "install-confirmation" {
            break;
        }
    }
    manager.confirm_install("fixture-request", true).unwrap();
    assert_eq!(finish(&rx).0["error"], "");
}
#[test]
fn cancellation_is_request_scoped_and_kills_descendants_without_service_control() {
    let f = Fixture::new("hang");
    let manager = CloudManager::default();
    let rx = f.start(&manager, "login");
    assert!(manager.cancel("unrelated").is_err());
    assert!(manager.confirm_install("fixture-request", true).is_err());
    assert_eq!(
        manager
            .start(f.cli.clone(), "other", "read", false, |_| {})
            .unwrap_err(),
        "connection_cloud_busy"
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while !f.root.join("descendant").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    manager.cancel("fixture-request").unwrap();
    let (v, _) = finish(&rx);
    assert_eq!(v["error"], "cancelled");
    assert!(v["device"].is_null());
    let pid = fs::read_to_string(f.root.join("descendant")).unwrap();
    // SIGKILL delivery to descendants is asynchronous; the leader is already reaped.
    let deadline = Instant::now() + Duration::from_secs(1);
    while let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) {
        if stat.split(") ").nth(1).unwrap().starts_with('Z') {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "cancelled CLI descendant is still running"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read_to_string(f.root.join("argv"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(manager.cancel("fixture-request").is_err());
}
#[test]
fn timeout_and_output_bound_are_reported_without_raw_output() {
    for mode in ["hang", "flood"] {
        let mut f = Fixture::new(mode);
        // Exercise the bounds independently; debug parsing can be slower than 400 ms.
        f.cli.timeout = if mode == "hang" {
            Duration::from_millis(400)
        } else {
            Duration::from_secs(4)
        };
        let manager = CloudManager::default();
        let rx = f.start(&manager, "login");
        let v = finish(&rx).0;
        assert_eq!(
            v["error"],
            if mode == "hang" {
                "t3_connect_timeout"
            } else {
                "t3_connect_output_limit"
            }
        );
    }
}
#[test]
fn mutation_failure_is_not_retried_and_disconnect_warning_survives_as_event() {
    for mode in ["failure", "warning"] {
        let f = Fixture::new(mode);
        let manager = CloudManager::default();
        let rx = f.start(&manager, "unlink");
        let (v, events) = finish(&rx);
        assert_eq!(
            v["error"],
            if mode == "failure" {
                "t3_connect_cli_failed"
            } else {
                ""
            }
        );
        assert_eq!(v["status"]["authenticated"], true);
        if mode == "warning" {
            assert!(events
                .iter()
                .any(|v| v["data"]["code"] == "t3_connect_disconnect_incomplete"));
        }
        assert_eq!(
            fs::read_to_string(f.root.join("argv"))
                .unwrap()
                .lines()
                .count(),
            2
        );
    }
}

#[test]
fn worker_shutdown_cancels_and_install_decline_does_not_continue_link() {
    let f = Fixture::new("hang");
    let manager = CloudManager::default();
    let rx = f.start(&manager, "login");
    manager.cancel_active().unwrap();
    assert_eq!(finish(&rx).0["error"], "cancelled");
    manager.cancel_active().unwrap();

    let f = Fixture::new("install");
    let rx = f.start(&manager, "link");
    loop {
        let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if event["stage"] == "install-confirmation" {
            break;
        }
    }
    manager.confirm_install("fixture-request", false).unwrap();
    assert!(manager.confirm_install("fixture-request", true).is_err());
    assert_eq!(finish(&rx).0["error"], "cancelled");
}

#[test]
fn authorization_denial_expiry_and_readback_failure_are_distinct() {
    for (mode, expected) in [
        ("denied", "t3_connect_authorization_denied"),
        ("expired", "t3_connect_authorization_expired"),
        ("bad-status", ""),
    ] {
        let f = Fixture::new(mode);
        let manager = CloudManager::default();
        let rx = f.start(&manager, "login");
        let (v, _) = finish(&rx);
        assert_eq!(v["error"], expected);
        if mode == "bad-status" {
            assert_eq!(v["statusError"], "t3_connect_invalid_status");
            assert!(v["status"].is_null());
        }
        assert_eq!(
            fs::read_to_string(f.root.join("argv"))
                .unwrap()
                .lines()
                .count(),
            2
        );
    }
}
