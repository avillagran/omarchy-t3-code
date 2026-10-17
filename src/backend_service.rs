//! Local lifecycle ownership. Never signal or stop a desktop-owned server.
use crate::*;
use std::process::{Command, Stdio};
fn unit_path() -> std::path::PathBuf {
    home().join(".config/systemd/user/t3code.service")
}
fn matching_unit() -> bool {
    fs::read_to_string(unit_path()).ok().is_some_and(|s| {
        s.lines().any(|l| {
            l.strip_prefix("Environment=T3CODE_HOME=")
                .is_some_and(|v| v.trim_matches('"') == t3_home().to_string_lossy())
        })
    })
}
fn alive(pid: u32) -> bool {
    pid > 0
        && unsafe { libc::kill(pid as i32, 0) } == 0
        && fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .is_none_or(|s| s.rsplit_once(") ").is_none_or(|(_, s)| !s.starts_with('Z')))
}
fn runtime_pid() -> Option<u32> {
    read_json(&t3_home().join("userdata/server-runtime.json"))
        .ok()?
        .get("pid")?
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|p| alive(*p))
}
/// Pure ownership classifier, also exercised by fixture tests. Service ownership
/// wins over desktop executable names: Electron can also run the headless CLI.
pub fn classify(service: bool, gui: bool, manual: bool) -> &'static str {
    if service {
        "headless"
    } else if gui {
        "gui"
    } else if manual {
        "headless-manual"
    } else {
        "unknown"
    }
}
fn evidence(pid: u32) -> (bool, bool, bool) {
    let group = fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap_or_default();
    let service = group.split('/').any(|s| s.trim() == "t3code.service");
    if service {
        return (true, false, false);
    }
    let mut gui = false;
    let mut p = pid;
    for _ in 0..12 {
        if p <= 1 {
            break;
        }
        let dir = format!("/proc/{p}");
        let group = fs::read_to_string(format!("{dir}/cgroup")).unwrap_or_default();
        if group
            .split('/')
            .any(|s| s.starts_with("app-com.t3tools.T3Code") && s.trim().ends_with(".scope"))
        {
            gui = true;
            break;
        }
        let env = fs::read(format!("{dir}/environ")).unwrap_or_default();
        let node = env
            .split(|b| *b == 0)
            .any(|v| v == b"ELECTRON_RUN_AS_NODE=1");
        let exe = fs::read_link(format!("{dir}/exe")).ok();
        let name = exe
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|p| p.to_str())
            .unwrap_or("");
        if matches!(name, "t3code" | "t3-code" | "T3 Code") && !node {
            gui = true;
            break;
        }
        let stat = fs::read_to_string(format!("{dir}/status")).unwrap_or_default();
        p = stat
            .lines()
            .find_map(|l| l.strip_prefix("PPid:"))
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
    }
    if service || gui {
        return (service, gui, false);
    }
    let env = fs::read(format!("/proc/{pid}/environ")).unwrap_or_default();
    let profile = format!("T3CODE_HOME={}", t3_home().display());
    let own_profile = env.split(|b| *b == 0).any(|v| v == profile.as_bytes());
    let args = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    let native = args.split(|b| *b == 0).any(|v| {
        let s = String::from_utf8_lossy(v);
        s.contains("/apps/server/") && (s.ends_with(".mjs") || s.ends_with(".js"))
    });
    let stat = fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
    let own_uid = stat
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse::<u32>().ok())
        == Some(unsafe { libc::geteuid() });
    (service, gui, native && own_profile && own_uid)
}
pub fn status() -> Value {
    let installed = unit_path().exists();
    let matches = !installed || matching_unit();
    let pid = runtime_pid();
    let owner = pid
        .map(|p| {
            let (s, g, m) = evidence(p);
            classify(s, g, m)
        })
        .unwrap_or("stopped");
    let control = matches && matches!(owner, "stopped" | "headless" | "headless-manual");
    json!({"owner":owner,"running":pid.is_some(),"pid":pid,"installed":installed,"canControl":control,"error":if matches{""}else{"service_profile_mismatch"}})
}
fn systemctl(action: &str) -> Result<()> {
    if !matching_unit() {
        return Err("service_profile_mismatch".into());
    }
    let out = Command::new("timeout")
        .args(["30", "systemctl", "--user", action, "t3code.service"])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "service_start_failed")?;
    if !out.status.success() {
        return Err("service_start_failed".into());
    }
    Ok(())
}
pub fn start() -> Result<Value> {
    if runtime_pid().is_some() {
        return Ok(json!({"status":"already-running","local":status()}));
    }
    if unit_path().exists() {
        // A GUI can replace/remove the runtime file while the older headless
        // service remains active. `start` is then a successful no-op. Restart
        // only this profile's native unit, with no live runtime to interrupt.
        systemctl("restart")?
    } else {
        let cli = home().join(".t3/bin/t3");
        if !cli.is_file() {
            return Err("t3_cli_missing".into());
        }
        let out = Command::new("timeout")
            .arg("45")
            .arg(cli)
            .args(["service", "install", "--base-dir"])
            .arg(t3_home())
            .stdin(Stdio::null())
            .output()
            .map_err(|_| "service_start_failed")?;
        if !out.status.success() {
            return Err("service_start_failed".into());
        }
    }
    // systemd Type=simple acknowledges the launcher before the HTTP server is
    // ready. Require native readiness rather than reporting a false success.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let runtime = t3_home().join("userdata/server-runtime.json");
    let cli = home().join(".t3/bin/t3");
    loop {
        if runtime_pid().is_some() {
            let info = Command::new("timeout")
                .arg("3")
                .arg(&cli)
                .arg("__ssh-helper")
                .arg("runtime-port")
                .arg(&runtime)
                .stdin(Stdio::null())
                .output()
                .map_err(|_| "service_start_failed")?;
            let port = String::from_utf8_lossy(&info.stdout)
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u16>().ok());
            if let Some(port) = port.filter(|_| info.status.success()) {
                let ready = Command::new("timeout")
                    .arg("18")
                    .arg(&cli)
                    .args([
                        "__ssh-helper",
                        "wait-ready",
                        &port.to_string(),
                        "15000",
                        "1000",
                    ])
                    .stdin(Stdio::null())
                    .output()
                    .map_err(|_| "service_start_failed")?;
                if !ready.status.success() {
                    return Err("service_start_failed".into());
                }
                break;
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err("service_start_failed".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    refresh_connections();
    Ok(json!({"status":"started","local":status()}))
}
fn stop() -> Result<Value> {
    let s = status();
    match text(&s, "owner") {
        "stopped" => return Ok(json!({"status":"stopped","local":s})),
        "gui" => return Err("backend_owned_by_gui".into()),
        "unknown" => return Err("backend_owner_unknown".into()),
        _ => {}
    }
    if s["canControl"] != true {
        return Err("service_profile_mismatch".into());
    }
    if s["owner"] == "headless" {
        systemctl("stop")?
    } else {
        let pid = s["pid"].as_u64().ok_or("backend_owner_unknown")? as u32;
        // Revalidate immediately before signaling. No force-kill or process-group kill.
        if runtime_pid() != Some(pid) || status()["owner"] != "headless-manual" {
            return Err("backend_owner_changed".into());
        }
        if unsafe { libc::kill(pid as i32, libc::SIGTERM) } != 0 {
            return Err("service_stop_failed".into());
        }
        for _ in 0..200 {
            if !alive(pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50))
        }
        if alive(pid) {
            return Err("service_stop_failed".into());
        }
    }
    refresh_connections();
    let local = status();
    if local["running"] == true {
        return Err("service_stop_failed".into());
    }
    Ok(json!({"status":"stopped","local":local}))
}
pub fn control(action: &str) -> Result<Value> {
    match action {
        "start" => start(),
        "stop" => stop(),
        "restart" => {
            stop()?;
            start()
        }
        _ => Err("invalid_action".into()),
    }
}
