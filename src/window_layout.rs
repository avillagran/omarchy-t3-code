//! Change only ot3's managed surface, never whichever application has focus.
use crate::*;
use serde_json::{json, Value};
use std::process::Command;
pub fn apply(input: &Value) -> Result<Value> {
    let pid = input["pid"]
        .as_u64()
        .filter(|p| *p > 0)
        .ok_or("invalid_window")?;
    let mode = text(input, "mode");
    if !matches!(mode, "floating" | "tiled") {
        return Err("invalid_window_mode".into());
    }
    let output = Command::new("timeout")
        .args(["2", "hyprctl", "-j", "clients"])
        .output()
        .map_err(|_| "window_manager_unavailable")?;
    if !output.status.success() {
        return Err("window_manager_unavailable".into());
    }
    let clients: Value =
        serde_json::from_slice(&output.stdout).map_err(|_| "window_manager_unavailable")?;
    let window = clients
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["pid"].as_u64() == Some(pid) && c["title"] == "Omarchy T3 Code")
        .ok_or("window_not_ready")?;
    let address = text(window, "address");
    if !address.starts_with("0x")
        || !address[2..].bytes().all(|b| b.is_ascii_hexdigit())
        || address.len() < 3
    {
        return Err("invalid_window".into());
    }
    // Compare observed state first: some shipped Lua IPC implementations treat
    // set/unset like toggle. Avoid flipping an already-correct window on reopen.
    if window["floating"].as_bool() == Some(mode == "floating") {
        return Ok(json!({"mode":mode}));
    }
    let command =
        format!("hl.dsp.window.float({{ action = \"toggle\", window = \"address:{address}\" }})");
    let result = Command::new("timeout")
        .args(["2", "hyprctl", "dispatch", &command])
        .output()
        .map_err(|_| "window_manager_unavailable")?;
    if !result.status.success() || String::from_utf8_lossy(&result.stdout).trim() != "ok" {
        return Err("window_mode_failed".into());
    }
    Ok(json!({"mode":mode}))
}
