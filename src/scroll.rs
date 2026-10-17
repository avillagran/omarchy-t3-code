//! Read effective compositor scroll support. Never changes desktop settings.
use serde_json::{json, Value};
use std::{io::{Read, Write}, os::unix::net::UnixStream, process::{Command, Stdio}, time::Duration};

// The compositor socket avoids spawning hyprctl while tracking popup underlays.
fn compositor(request: &str) -> Option<Value> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let instance = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let path = std::path::PathBuf::from(runtime).join("hypr").join(instance).join(".socket.sock");
    let mut socket = UnixStream::connect(path).ok()?;
    socket.set_read_timeout(Some(Duration::from_millis(100))).ok()?;
    socket.set_write_timeout(Some(Duration::from_millis(100))).ok()?;
    socket.write_all(request.as_bytes()).ok()?;
    let mut bytes = Vec::new();
    socket.take(4 * 1024 * 1024).read_to_end(&mut bytes).ok()?;
    serde_json::from_slice(&bytes).ok()
}
fn option(key: &str) -> Option<Value> {
    let value = compositor(&format!("j/getoption {key}")).or_else(|| {
        let out = Command::new("timeout").args(["1", "hyprctl", "getoption", key, "-j"])
            .stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
        if !out.status.success() { return None }
        serde_json::from_slice(&out.stdout).ok()
    })?;
    (value["option"] == key).then_some(value)
}
fn device(profile: Option<&Value>, decel: Option<&Value>) -> bool {
    profile.and_then(|v| v["int"].as_i64()).is_some_and(|n| n > 0)
        && decel.and_then(|v| v["int"].as_i64()).is_some_and(|n| n > 0)
}
fn excluded(pattern: &str, cursor: &Value, clients: &Value) -> bool {
    let Some(x) = cursor["x"].as_f64() else { return false };
    let Some(y) = cursor["y"].as_f64() else { return false };
    // Layer surfaces are absent from clients, just as in the patch's windowAt
    // lookup: a bar popup inherits the exclusion of the window underneath it.
    let window = clients.as_array().into_iter().flatten().filter(|w| {
        if w["mapped"] != true || w["hidden"] == true || w["visible"] == false || w["acceptsInput"] == false { return false }
        let (Some(left), Some(top), Some(width), Some(height)) =
            (w["at"][0].as_f64(), w["at"][1].as_f64(), w["size"][0].as_f64(), w["size"][1].as_f64()) else { return false };
        x >= left && y >= top && x < left + width && y < top + height
    }).min_by_key(|w| (w["floating"] != true, w["focusHistoryID"].as_i64().unwrap_or(i64::MAX)));
    let class = window.and_then(|w| w["class"].as_str()).unwrap_or("").to_lowercase();
    pattern.split(',').any(|token| !token.is_empty() && class.contains(&token.to_lowercase()))
}
pub fn read() -> Value {
    let touch = option("input:touchpad:scroll_accel_profile");
    let mouse = option("input:mouse:scroll_accel_profile");
    let td = if touch.is_some() { option("input:touchpad:scroll_decel") } else { None };
    let md = if mouse.is_some() { option("input:mouse:scroll_decel") } else { None };
    let configured = device(touch.as_ref(), td.as_ref());
    let ignore = if configured { option("input:touchpad:scroll_ignore_classes") } else { None };
    let pattern = ignore.as_ref().and_then(|v| v["str"].as_str()).unwrap_or("");
    let ignored = !pattern.is_empty() && compositor("j/cursorpos").zip(compositor("j/clients"))
        .is_some_and(|(cursor, clients)| excluded(pattern, &cursor, &clients));
    json!({"touchpadNative": configured && !ignored, "mouseNative": device(mouse.as_ref(), md.as_ref()),
        "trackUnderlay": configured && !pattern.is_empty(), "touchpadExcluded": ignored})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_effective_coast_suppresses_local_fallback() {
        assert!(!device(None, None));
        assert!(!device(Some(&json!({"int":0})), Some(&json!({"int":600}))));
        assert!(!device(Some(&json!({"int":2})), Some(&json!({"int":0}))));
        assert!(device(Some(&json!({"int":2})), Some(&json!({"int":600}))));
    }
    #[test]
    fn popup_exclusions_follow_pointer_and_stacking_not_active_window() {
        let mut clients = json!([
            {"mapped":true,"visible":true,"class":"Google-Chrome","at":[0,0],"size":[800,800],"focusHistoryID":0},
            {"mapped":true,"visible":true,"floating":true,"class":"org.quickshell","at":[100,100],"size":[200,200],"focusHistoryID":1}
        ]);
        assert!(excluded("google-chrome,kitty", &json!({"x":50,"y":50}), &clients));
        assert!(!excluded("google-chrome,kitty", &json!({"x":150,"y":150}), &clients));
        clients[1]["hidden"] = json!(true);
        assert!(excluded("google-chrome,kitty", &json!({"x":150,"y":150}), &clients));
        clients[0]["visible"] = json!(false);
        assert!(!excluded("google-chrome,kitty", &json!({"x":150,"y":150}), &clients));
        assert!(!excluded("", &json!({"x":50,"y":50}), &clients));
    }
}
