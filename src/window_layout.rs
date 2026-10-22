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
    if window["floating"].as_bool() != Some(mode == "floating") {
        let command =
            format!("hl.dsp.window.float({{ action = \"toggle\", window = \"address:{address}\" }})");
        let result = Command::new("timeout")
            .args(["2", "hyprctl", "dispatch", &command])
            .output()
            .map_err(|_| "window_manager_unavailable")?;
        if !result.status.success() || String::from_utf8_lossy(&result.stdout).trim() != "ok" {
            return Err("window_mode_failed".into());
        }
        // A window that just started floating keeps its tiled size (often the
        // whole monitor): give it its own floating size, centred. Best effort.
        if mode == "floating" {
            if let Some((w, h)) = floating_size(input) {
                let batch = format!(
                    "dispatch hl.dsp.window.resize({{ x = {w}, y = {h}, window = \"address:{address}\" }}) ; dispatch hl.dsp.window.center({{ window = \"address:{address}\" }})"
                );
                let _ = Command::new("timeout").args(["2", "hyprctl", "--batch", &batch]).output();
            }
        }
    }
    restore_decorations(address);
    Ok(json!({"mode":mode}))
}

/// The floating size the panel remembered, in logical pixels, when sane.
fn floating_size(input: &Value) -> Option<(u32, u32)> {
    let w = input["width"].as_f64().filter(|w| (360.0..=10000.0).contains(w))?;
    let h = input["height"].as_f64().filter(|h| (240.0..=10000.0).contains(h))?;
    Some((w.round() as u32, h.round() as u32))
}

/// Broad `org.quickshell` rules (e.g. overlay veils) strip borders, shadows,
/// animations and focus-follows-mouse from every shell window. As a regular
/// window ot3 follows the user's own settings instead, so "no gaps or
/// borders" and a normal border both apply. Best effort: a failure leaves
/// the window usable, only undecorated.
fn restore_decorations(address: &str) {
    let border = Command::new("timeout")
        .args(["2", "hyprctl", "getoption", "general:border_size", "-j"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
        .and_then(|v| v["int"].as_i64())
        .filter(|n| (0..=100).contains(n))
        .unwrap_or(2);
    let props = [
        ("border_size", border.to_string()),
        ("decorate", "true".into()),
        ("no_shadow", "false".into()),
        ("no_anim", "false".into()),
        ("no_follow_mouse", "false".into()),
    ];
    let batch = props
        .iter()
        .map(|(prop, value)| {
            format!("dispatch hl.dsp.window.set_prop({{ window = \"address:{address}\", prop = \"{prop}\", value = \"{value}\" }})")
        })
        .collect::<Vec<_>>()
        .join(" ; ");
    let _ = Command::new("timeout")
        .args(["2", "hyprctl", "--batch", &batch])
        .output();
}

#[cfg(test)]
mod floating_size_tests {
    use super::*;
    #[test]
    fn floating_size_accepts_only_sane_sizes() {
        assert_eq!(floating_size(&json!({"width":1358.4,"height":722.0})), Some((1358, 722)));
        assert_eq!(floating_size(&json!({"width":100,"height":722})), None);
        assert_eq!(floating_size(&json!({"width":1358})), None);
        assert_eq!(floating_size(&json!({"width":"1358","height":722})), None);
    }
}
