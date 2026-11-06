//! Local lifecycle ownership. Never signal or stop a desktop-owned server.
use crate::*;
use std::process::{Command, Stdio};
/// Explicit launch profiles keep O3 and T3 histories and credentials separate.
/// The optional registry maps installed user services; it never supplies shell commands.
#[derive(Clone)]
struct Target {
    id: String,
    unit: String,
    profile: std::path::PathBuf,
    cli: std::path::PathBuf,
    registered: bool,
    /// Both registered servers use this profile (O3 Code's native T3 fallback on O3
    /// Server's profile); its runtime file then says which one is running.
    shared: bool,
}
fn registry_path() -> std::path::PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("backends.json")
}
fn choice_path() -> std::path::PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("backend-choice.json")
}
fn targets() -> Result<(Vec<Target>, String)> {
    let path = registry_path();
    if !path.exists() {
        return Ok((
            vec![Target {
                id: "t3".into(),
                unit: "t3code.service".into(),
                profile: t3_home(),
                cli: home().join(".t3/bin/t3"),
                registered: false,
                shared: false,
            }],
            "t3".into(),
        ));
    }
    if fs::metadata(&path)
        .map_err(|_| "backend_configuration_invalid")?
        .len()
        > 16384
    {
        return Err("backend_configuration_invalid".into());
    }
    let value = read_json(&path).map_err(|_| "backend_configuration_invalid")?;
    let rows = value["servers"]
        .as_array()
        .filter(|r| !r.is_empty() && r.len() <= 2)
        .ok_or("backend_configuration_invalid")?;
    let mut result: Vec<Target> = Vec::new();
    for row in rows {
        let id = text(row, "id");
        let unit = text(row, "unit");
        let profile = std::path::PathBuf::from(text(row, "profile"));
        let cli = std::path::PathBuf::from(text(row, "cli"));
        if !matches!(id, "o3" | "t3")
            || result.iter().any(|t| t.id == id || t.unit == unit)
            || unit.len() > 128
            || !unit.ends_with(".service")
            || !unit
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            || !profile.is_absolute()
            || !cli.is_absolute()
            || profile == std::path::Path::new("/")
        {
            return Err("backend_configuration_invalid".into());
        }
        result.push(Target {
            id: id.into(),
            unit: unit.into(),
            profile,
            cli,
            registered: true,
            shared: false,
        });
    }
    if let [a, b] = result.as_mut_slice() {
        let shared = a.profile == b.profile;
        a.shared = shared;
        b.shared = shared;
    }
    let default = text(&value, "default");
    if value["version"] != 1 || !result.iter().any(|t| t.id == default) {
        return Err("backend_configuration_invalid".into());
    }
    let selected = read_json(&choice_path())
        .ok()
        .and_then(|v| v["server"].as_str().map(str::to_owned))
        .filter(|id| result.iter().any(|t| &t.id == id))
        .unwrap_or_else(|| default.into());
    Ok((result, selected))
}
fn effective_target(targets: &[Target], id: &str) -> Target {
    let chosen = targets.iter().find(|t| t.id == id).unwrap();
    if runtime_pid(chosen).is_none() {
        // An existing service can start at login before the client. Reflect a
        // verified running backend rather than offering to launch its rival.
        if let Some(running) = targets.iter().find(|t| {
            runtime_pid(t).is_some_and(|pid| {
                let (service, gui, manual) = evidence(pid, t);
                service || gui || manual
            })
        }) {
            return running.clone();
        }
    }
    chosen.clone()
}
fn selected() -> Result<Target> {
    let (targets, id) = targets()?;
    Ok(effective_target(&targets, &id))
}
/// Only the local transport changes profile; the client's saved connections and UI stay put.
pub fn local_profile() -> std::path::PathBuf {
    selected().map(|t| t.profile).unwrap_or_else(|_| t3_home())
}
pub fn local_cli() -> std::path::PathBuf {
    selected()
        .map(|t| t.cli)
        .unwrap_or_else(|_| home().join(".t3/bin/t3"))
}
pub fn local_token() -> std::path::PathBuf {
    match selected() {
        Ok(t) if t.registered => {
            xdg("XDG_DATA_HOME", ".local/share").join(format!("backend-{}.token", t.id))
        }
        _ => xdg("XDG_DATA_HOME", ".local/share").join("local.write.token"),
    }
}
fn unit_path(target: &Target) -> std::path::PathBuf {
    home().join(".config/systemd/user").join(&target.unit)
}
fn matching_unit(target: &Target) -> bool {
    let base = unit_path(target);
    let mut files = vec![base.clone()];
    if let Ok(entries) = fs::read_dir(base.with_file_name(format!("{}.d", target.unit))) {
        let mut dropins: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "conf"))
            .collect();
        dropins.sort();
        files.extend(dropins);
    }
    let mut profile = None;
    for file in files {
        for line in fs::read_to_string(file).unwrap_or_default().lines() {
            if let Some(value) = line.trim().strip_prefix("Environment=") {
                if value.is_empty() {
                    profile = None;
                }
                if let Some(v) = value.trim_matches('"').strip_prefix("T3CODE_HOME=") {
                    profile = Some(v.trim_matches('"').to_owned());
                }
            }
        }
    }
    profile.is_some_and(|p| p == target.profile.to_string_lossy())
}
fn alive(pid: u32) -> bool {
    pid > 0
        && unsafe { libc::kill(pid as i32, 0) } == 0
        && fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .is_none_or(|s| s.rsplit_once(") ").is_none_or(|(_, s)| !s.starts_with('Z')))
}
fn runtime_pid(target: &Target) -> Option<u32> {
    let runtime = read_json(&target.profile.join("userdata/server-runtime.json")).ok()?;
    if target.shared && runtime_is_o3(&runtime) != (target.id == "o3") {
        return None;
    }
    runtime
        .get("pid")?
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|p| alive(*p))
}
/// O3 Server marks its runtime's `serverVersion` with `o3-`; T3 does not.
fn runtime_is_o3(runtime: &Value) -> bool {
    runtime["serverVersion"]
        .as_str()
        .is_some_and(|v| v.starts_with("o3-"))
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
fn evidence(pid: u32, target: &Target) -> (bool, bool, bool) {
    let group = fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap_or_default();
    let service = group.split('/').any(|s| s.trim() == target.unit);
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
    let profile = format!("T3CODE_HOME={}", target.profile.display());
    let own_profile = env.split(|b| *b == 0).any(|v| v == profile.as_bytes());
    let args = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    let native = (target.id == "o3"
        && fs::read_link(format!("/proc/{pid}/exe"))
            .ok()
            .is_some_and(|exe| exe == target.cli))
        || args.split(|b| *b == 0).any(|v| {
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
fn target_status(target: &Target) -> Value {
    let installed = unit_path(target).exists();
    let matches = !installed || matching_unit(target);
    let pid = runtime_pid(target);
    let owner = pid
        .map(|p| {
            let (s, g, m) = evidence(p, target);
            classify(s, g, m)
        })
        .unwrap_or("stopped");
    let control = matches && matches!(owner, "stopped" | "headless" | "headless-manual");
    json!({"backend":target.id,"owner":owner,"running":pid.is_some(),"pid":pid,"installed":installed,"nativeService":target.id=="t3" && target.unit=="t3code.service" && target.profile==t3_home(),"available":target.cli.is_file() && (!target.registered || installed),"canControl":control,"error":if matches{""}else{"service_profile_mismatch"}})
}
pub fn status() -> Value {
    let Ok((targets, id)) = targets() else {
        return json!({"owner":"unknown","running":false,"canControl":false,"installed":false,"error":"backend_configuration_invalid","choices":[]});
    };
    let mut selected = target_status(&effective_target(&targets, &id));
    selected["choices"] = json!(targets.iter().map(target_status).collect::<Vec<_>>());
    selected
}
fn systemctl(target: &Target, action: &str) -> Result<()> {
    if !matching_unit(target) {
        return Err("service_profile_mismatch".into());
    }
    let out = Command::new("timeout")
        .args(["30", "systemctl", "--user", action, &target.unit])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "service_start_failed")?;
    if !out.status.success() {
        return Err("service_start_failed".into());
    }
    Ok(())
}
pub fn start() -> Result<Value> {
    start_as(&selected()?.id)
}
pub fn start_as(id: &str) -> Result<Value> {
    use std::{
        fs::OpenOptions,
        os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    };
    let path = choice_path().with_file_name("backend-start.lock");
    fs::create_dir_all(path.parent().ok_or("local_error")?).map_err(|_| "local_error")?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "local_error")?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("backend_start_busy".into());
    }
    let (targets, current) = targets()?;
    let target = targets
        .iter()
        .find(|t| t.id == id)
        .ok_or("backend_not_installed")?;
    if targets
        .iter()
        .any(|t| t.id != id && runtime_pid(t).is_some())
    {
        return Err("backend_already_running".into());
    }
    launchable(target)?;
    // Record an explicit choice only. An unsuccessful launch never selects another server.
    if current != id {
        save_json(&choice_path(), &json!({"server":id}))?;
    }
    start_target(target)
}
fn launchable(target: &Target) -> Result<()> {
    if target.registered && (!unit_path(target).exists() || !target.cli.is_file()) {
        return Err("backend_not_installed".into());
    }
    if target.registered && !matching_unit(target) {
        return Err("service_profile_mismatch".into());
    }
    Ok(())
}
/// Stops the running local server and starts another declared one. The target
/// is checked before anything is stopped; a failed start leaves both stopped
/// (no automatic return to the previous server) and the error is reported.
pub fn switch_to(id: &str) -> Result<Value> {
    let (targets, _) = targets()?;
    let target = targets
        .iter()
        .find(|t| t.id == id)
        .ok_or("backend_not_installed")?;
    launchable(target)?;
    if !target.cli.is_file() {
        return Err("backend_not_installed".into());
    }
    if selected()?.id == id && runtime_pid(target).is_some() {
        return Ok(json!({"status":"already-running","local":status()}));
    }
    stop()?;
    start_as(id)
}
fn start_target(target: &Target) -> Result<Value> {
    if runtime_pid(target).is_some() {
        return Ok(json!({"status":"already-running","local":status()}));
    }
    if unit_path(target).exists() {
        // A GUI can replace/remove the runtime file while the older headless
        // service remains active. `start` is then a successful no-op. Restart
        // only this profile's native unit, with no live runtime to interrupt.
        if target.registered {
            systemctl(target, "reset-failed")?;
        }
        systemctl(target, "restart")?
    } else {
        let cli = target.cli.clone();
        if !cli.is_file() {
            return Err("t3_cli_missing".into());
        }
        let out = Command::new("timeout")
            .arg("45")
            .arg(cli)
            .args(["service", "install", "--base-dir"])
            .arg(&target.profile)
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
    let runtime = target.profile.join("userdata/server-runtime.json");
    let cli = target.cli.clone();
    loop {
        if runtime_pid(target).is_some() {
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
    let target = selected()?;
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
        systemctl(&target, "stop")?
    } else {
        let pid = s["pid"].as_u64().ok_or("backend_owner_unknown")? as u32;
        // Revalidate immediately before signaling. No force-kill or process-group kill.
        if runtime_pid(&target) != Some(pid) || status()["owner"] != "headless-manual" {
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
