//! Explicit, local-only native service start. Never stops a desktop-owned backend.
use crate::*;
use std::process::{Command, Stdio};
fn runtime_alive() -> bool {
    read_json(&t3_home().join("userdata/server-runtime.json"))
        .ok()
        .and_then(|r| r["pid"].as_i64())
        .is_some_and(|pid| pid > 0 && unsafe { libc::kill(pid as i32, 0) } == 0)
}
pub fn start() -> Result<Value> {
    if runtime_alive() {
        return Ok(json!({"status":"already-running"}));
    }
    let cli = home().join(".t3/bin/t3");
    if !cli.is_file() {
        return Err("t3_cli_missing".into());
    }
    let unit = home().join(".config/systemd/user/t3code.service");
    let output = if unit.exists() {
        let contents = fs::read_to_string(unit).map_err(|_| "service_read_failed")?;
        // Never repoint a service installed for another profile.
        if !contents.lines().any(|l| {
            l.strip_prefix("Environment=T3CODE_HOME=")
                .is_some_and(|value| value.trim_matches('"') == t3_home().to_string_lossy())
        }) {
            return Err("service_profile_mismatch".into());
        }
        Command::new("timeout")
            .args(["30", "systemctl", "--user", "start", "t3code.service"])
            .stdin(Stdio::null())
            .output()
    } else {
        // Native installer owns service files, persistence, locking and restart policy.
        Command::new("timeout")
            .arg("45")
            .arg(cli)
            .args(["service", "install", "--base-dir"])
            .arg(t3_home())
            .stdin(Stdio::null())
            .output()
    }
    .map_err(|_| "service_start_failed")?;
    if !output.status.success() {
        return Err("service_start_failed".into());
    }
    refresh_connections();
    Ok(json!({"status":"starting"}))
}
