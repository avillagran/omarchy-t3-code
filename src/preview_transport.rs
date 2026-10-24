//! Native preview metadata reader. No page process, automation or mutation is
//! started by observing tabs. The original native RPC owns session metadata.
use crate::{json, protocol, text, Environment, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

fn string(v: &Value, max: usize, nonempty: bool) -> Result<&str> {
    v.as_str()
        .filter(|s| {
            s.encode_utf16().count() <= max
                && !s.contains('\0')
                && (!nonempty || !s.trim().is_empty())
        })
        .ok_or("invalid_preview_state".into())
}
fn number(v: &Value) -> Result<u64> {
    v.as_f64()
        .filter(|n| n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= 9_007_199_254_740_991.0)
        .map(|n| n as u64)
        .ok_or("invalid_preview_state".into())
}
fn boolean(v: &Value) -> Result<bool> {
    v.as_bool().ok_or("invalid_preview_state".into())
}
fn download(v: &Value) -> Result<Value> {
    Ok(json!({"id":string(&v["id"],2048,false)?,"fileName":string(&v["fileName"],4096,false)?}))
}
fn navigation(v: &Value) -> Result<Value> {
    let tag = text(v, "_tag");
    if tag == "Idle" {
        return Ok(json!({"_tag":"Idle"}));
    }
    if !matches!(tag, "Loading" | "Success" | "LoadFailed") {
        return Err("invalid_preview_state".into());
    }
    let mut out = json!({"_tag":tag,"url":string(&v["url"],2048,true)?,"title":string(&v["title"],512,false)?});
    if tag == "LoadFailed" {
        let code = v["code"]
            .as_f64()
            .filter(|n| n.is_finite() && n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0)
            .ok_or("invalid_preview_state")?;
        out["code"] = json!(code as i64);
        out["description"] = json!(string(&v["description"], 65536, false)?);
        if let Some(d) = v.get("download") {
            out["download"] = download(d)?;
        }
    }
    Ok(out)
}
pub fn snapshot(v: &Value) -> Result<Value> {
    let mut out = json!({"threadId":string(&v["threadId"],2048,true)?,"tabId":string(&v["tabId"],128,true)?,"navStatus":navigation(&v["navStatus"])?,"canGoBack":boolean(&v["canGoBack"])?,"canGoForward":boolean(&v["canGoForward"])?,"updatedAt":string(&v["updatedAt"],128,false)?});
    // Reuse the validated native viewport/zoom/profile contracts, not raw config.
    if let Some(viewport) = v.get("viewport") {
        out["viewport"] = crate::browser_preferences::validate("browserDefaultViewport", viewport)
            .map_err(|_| "invalid_preview_state")?;
    }
    if let Some(profile) = v.get("profileId") {
        let id = string(profile, 128, true)?;
        crate::browser_preferences::validate("browserDefaultProfileId", &json!(id))
            .map_err(|_| "invalid_preview_state")?;
        out["profileId"] = json!(id);
    }
    for (key, values) in [
        ("runtime", &["desktop", "server"][..]),
        ("colorScheme", &["system", "light", "dark"][..]),
    ] {
        if let Some(value) = v.get(key) {
            if !values.contains(&text(v, key)) {
                return Err("invalid_preview_state".into());
            }
            out[key] = value.clone();
        }
    }
    if let Some(zoom) = v.get("zoomFactor") {
        let allowed = crate::browser_preferences::contract()["zoomLevels"]
            .as_array()
            .cloned()
            .ok_or("invalid_preview_state")?;
        if !allowed.iter().any(|n| n.as_f64() == zoom.as_f64()) || zoom.as_f64().is_none() {
            return Err("invalid_preview_state".into());
        }
        out["zoomFactor"] = zoom.clone();
    }
    if let Some(owner) = v.get("automationOwner") {
        out["automationOwner"] = json!(string(owner, 2048, false)?);
    }
    if let Some(reveal) = v.get("reveal") {
        out["reveal"] = json!(boolean(reveal)?);
    }
    if let Some(request) = v.get("revealRequest") {
        out["revealRequest"] =
            json!({"id":string(&request["id"],2048,false)?,"force":boolean(&request["force"])?});
    }
    Ok(out)
}
pub fn list(v: &Value, thread: &str) -> Result<Value> {
    let values = v["sessions"]
        .as_array()
        .filter(|a| a.len() <= 4096)
        .ok_or("invalid_preview_state")?;
    let mut seen = std::collections::HashSet::new();
    let sessions = values
        .iter()
        .map(|v| {
            let s = snapshot(v)?;
            if s["threadId"] != thread || !seen.insert(text(&s, "tabId").to_owned()) {
                return Err("invalid_preview_state".into());
            }
            Ok(s)
        })
        .collect::<Result<Vec<_>>>()?;
    let out = json!({"sessions":sessions,"serverEpoch":string(&v["serverEpoch"],2048,true)?,"revision":number(&v["revision"])?});
    bounded(out)
}
pub fn event(v: &Value) -> Result<Value> {
    let tag = text(v, "type");
    let mut out = json!({"type":tag,"threadId":string(&v["threadId"],2048,true)?,"tabId":string(&v["tabId"],128,true)?,"createdAt":string(&v["createdAt"],128,false)?,"serverEpoch":string(&v["serverEpoch"],2048,true)?,"revision":number(&v["revision"])?});
    if out["revision"] == 0 {
        return Err("invalid_preview_state".into());
    }
    match tag {
        "opened" | "navigated" | "resized" => {
            let s = snapshot(&v["snapshot"])?;
            if s["threadId"] != out["threadId"] || s["tabId"] != out["tabId"] {
                return Err("invalid_preview_state".into());
            }
            out["snapshot"] = s;
            if tag == "resized" {
                if let Some(r) = v.get("request") {
                    let mut request = json!({});
                    if let Some(reload) = r.get("hardReload") {
                        request["hardReload"] = json!(boolean(reload)?);
                    }
                    if let Some(clear) = r.get("clear") {
                        if !matches!(clear.as_str(), Some("cookies" | "cache")) {
                            return Err("invalid_preview_state".into());
                        }
                        request["clear"] = clear.clone();
                    }
                    out["request"] = request;
                }
            }
        }
        "failed" => {
            let n = navigation(
                &json!({"_tag":"LoadFailed","url":v["url"],"title":v["title"],"code":v["code"],"description":v["description"]}),
            )?;
            for key in ["url", "title", "code", "description"] {
                out[key] = n[key].clone();
            }
            if let Some(d) = v.get("download") {
                out["download"] = download(d)?;
            }
        }
        "closed" => {}
        _ => return Err("invalid_preview_state".into()),
    }
    bounded(out)
}
fn bounded(v: Value) -> Result<Value> {
    if v.to_string().len() > 2 * 1024 * 1024 {
        Err("preview_state_too_large".into())
    } else {
        Ok(v)
    }
}
pub(crate) fn inspect(e: &Environment) -> Result<(protocol::Connection, Value)> {
    let c = e.connection(true)?;
    let auth = c.request_json("GET", "/api/auth/session", None, false)?;
    let has = |scope: &str| {
        auth["authenticated"] == true
            && auth
                .get("permissions")
                .or_else(|| auth.get("scopes"))
                .and_then(Value::as_array)
                .is_some_and(|p| p.iter().any(|v| v == scope))
    };
    if !has("orchestration:read") {
        return Err("permission_denied".into());
    }
    let config = protocol::api::server::config(&c)?;
    Ok((
        c,
        json!({"environment":e.id,"originKey":format!("{:x}",Sha256::digest(e.base_url.as_bytes())),"canOperate":has("preview:operate"),"serverBrowser":config["environment"]["capabilities"]["serverBrowser"]==true}),
    ))
}
fn valid_thread(thread: &str) -> Result<()> {
    string(&json!(thread), 2048, true).map(|_| ())
}
pub fn read(e: &Environment, thread: &str) -> Result<Value> {
    valid_thread(thread)?;
    let (c, mut out) = inspect(e)?;
    out["kind"] = json!("list");
    out["thread"] = json!(thread);
    out["result"] = list(&protocol::api::preview::list(&c, thread)?, thread)?;
    Ok(out)
}
/// Authorize a local website surface from current native metadata. UI fields
/// select a tab, never supply its URL/profile/ownership or native permissions.
pub fn surface(e: &Environment, input: &Value) -> Result<Value> {
    if input["environment"] != e.id {
        return Err("preview_scope_changed".into());
    }
    let thread = string(&input["thread"], 2048, true)?;
    let tab = string(&input["tabId"], 128, true)?;
    let current = read(e, thread)?;
    if current["canOperate"] != true {
        return Err("permission_denied".into());
    }
    if current["originKey"] != input["originKey"] {
        return Err("preview_scope_changed".into());
    }
    if current["result"]["serverEpoch"] != input["serverEpoch"] {
        return Err("preview_server_restarted".into());
    }
    let snapshot = current["result"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tabId"] == tab)
        .ok_or("preview_tab_closed")?;
    if snapshot["runtime"] == "server" {
        return Err("server_browser_surface_required".into());
    }
    Ok(
        json!({"environment":e.id,"thread":thread,"tabId":tab,"originKey":current["originKey"],"serverEpoch":current["result"]["serverEpoch"],"revision":current["result"]["revision"],"canOperate":current["canOperate"],"snapshot":snapshot}),
    )
}
/// Private guest-to-native status path. No UI command exposes it. Scope and
/// profile identity are checked afresh; only the native five status fields go
/// over the wire, never renderer paths, provider identity or process handles.
pub fn report_status(e: &Environment, scope: &Value, status: &Value) -> Result<()> {
    let (c, auth) = inspect(e)?;
    if scope["environment"] != e.id || auth["originKey"] != scope["originKey"] {
        return Err("preview_scope_changed".into());
    }
    if auth["canOperate"] != true {
        return Err("permission_denied".into());
    }
    let thread = string(&scope["thread"], 2048, true)?;
    let tab = string(&scope["tabId"], 128, true)?;
    let current = list(&protocol::api::preview::list(&c, thread)?, thread)?;
    if current["serverEpoch"] != scope["serverEpoch"] {
        return Err("preview_server_restarted".into());
    }
    let native = current["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["tabId"] == tab)
        .ok_or("preview_tab_closed")?;
    if native["runtime"] == "server" {
        return Err("server_browser_surface_required".into());
    }
    let native_profile = native
        .get("profileId")
        .and_then(Value::as_str)
        .or_else(|| scope.get("profileId").and_then(Value::as_str))
        .unwrap_or("default");
    let checked = snapshot(status)?;
    if checked["threadId"] != thread
        || checked["tabId"] != tab
        || text(&checked, "profileId") != native_profile
        || checked["runtime"] != "desktop"
    {
        return Err("preview_scope_changed".into());
    }
    if scope
        .get("profileId")
        .and_then(Value::as_str)
        .unwrap_or("default")
        != native_profile
    {
        return Err("preview_scope_changed".into());
    }
    c.rpc("preview.reportStatus",json!({"threadId":thread,"tabId":tab,"navStatus":checked["navStatus"],"canGoBack":checked["canGoBack"],"canGoForward":checked["canGoForward"]}))?;
    Ok(())
}
/// Subscribe first, then reconcile a list at the first stream boundary/idle
/// tick. Periodic lists close any startup/reconnect gap; epochs/revisions in
/// the original reducer determine ordering, never callback arrival time.
pub fn observe(
    e: &Environment,
    thread: &str,
    cancel: impl Fn() -> bool,
    mut receive: impl FnMut(Value) -> Result<()>,
) -> Result<()> {
    valid_thread(thread)?;
    if cancel() {
        return Ok(());
    }
    let (c, base) = inspect(e)?;
    let mut refreshed = None::<Instant>;
    let mut epoch = String::new();
    c.subscribe("subscribePreviewEvents", json!({}), &cancel, |item| {
        if cancel() {
            return Ok(());
        }
        let relevant = item.as_ref().filter(|v| text(v, "threadId") == thread);
        let new_epoch = relevant.is_some_and(|v| text(v, "serverEpoch") != epoch);
        if refreshed.is_none_or(|t| t.elapsed() >= Duration::from_secs(5)) || new_epoch {
            let result = list(&protocol::api::preview::list(&c, thread)?, thread)?;
            epoch = text(&result, "serverEpoch").to_owned();
            if cancel() {
                return Ok(());
            }
            let mut message = base.clone();
            message["kind"] = json!("list");
            message["thread"] = json!(thread);
            message["result"] = result;
            receive(message)?;
            refreshed = Some(Instant::now());
        }
        if cancel() {
            return Ok(());
        }
        if let Some(v) = relevant {
            let mut message = base.clone();
            message["kind"] = json!("event");
            message["thread"] = json!(thread);
            message["result"] = event(v)?;
            receive(message)?;
        }
        Ok(())
    })
}
