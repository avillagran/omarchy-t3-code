//! Native preview lifecycle commands. No browser automation or OS navigation.
//! Requests are bound to a read origin/epoch and never replayed automatically.
use crate::{json, preview_transport as transport, protocol::{self, api::{preview as pv, Function}}, text, Environment, Result};
use serde_json::Value;
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};
static DISPATCHED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
fn string(v: &Value, max: usize) -> Result<&str> {
    v.as_str()
        .filter(|s| !s.trim().is_empty() && s.encode_utf16().count() <= max && !s.contains('\0'))
        .ok_or("invalid_preview_command".into())
}
fn optional(out: &mut Value, input: &Value, key: &str, max: usize) -> Result<()> {
    if let Some(v) = input.get(key) {
        out[key] = json!(string(v, max)?);
    }
    Ok(())
}
fn preference(key: &str, value: &Value) -> Result<Value> {
    crate::browser_preferences::validate(key, value).map_err(|_| "invalid_preview_command".into())
}
fn address(value: &Value) -> Result<String> {
    let template = crate::connection_preferences::client_settings()
        .map(|s| crate::browser_address::search_template(&s))
        .unwrap_or_else(|_| crate::browser_address::search_template(&Value::Null));
    let url = crate::browser_address::resolve_with(string(value, 2048)?, &template)?;
    if url.encode_utf16().count() > 2048 {
        return Err("invalid_preview_address".into());
    }
    Ok(url)
}
/// Whitelist exact native inputs. Address text uses the original-helper port;
/// native T3 remains authoritative for URL normalization and session mutation.
pub fn payload(input: &Value) -> Result<(&'static Function, Value)> {
    let action = text(input, "action");
    let mut out = json!({"threadId":string(&input["thread"],2048)?});
    if action != "open" {
        if action == "close" {
            optional(&mut out, input, "tabId", 128)?;
        } else {
            out["tabId"] = json!(string(&input["tabId"], 128)?);
        }
    }
    let method = match action {
        "open" => {
            if let Some(address) = input.get("address") {
                out["url"] = json!(self::address(address)?);
            } else {
                optional(&mut out, input, "url", 2048)?;
            }
            if let Some(v) = input.get("viewport") {
                out["viewport"] = preference("browserDefaultViewport", v)?;
            }
            if let Some(v) = input.get("profileId") {
                out["profileId"] = preference("browserDefaultProfileId", v)?;
            }
            if let Some(v) = input.get("runtime") {
                if !matches!(v.as_str(), Some("desktop" | "server")) {
                    return Err("invalid_preview_command".into());
                }
                out["runtime"] = v.clone();
            }
            // Human controls do not forge provider automationOwner/reveal claims.
            &pv::OPEN
        }
        "navigate" => {
            out["url"] = if let Some(address) = input.get("address") {
                json!(self::address(address)?)
            } else {
                json!(string(&input["url"], 2048)?)
            };
            if let Some(value) = input.get("resolvedTitle") {
                let title = value
                    .as_str()
                    .filter(|s| s.encode_utf16().count() <= 512 && !s.contains('\0'))
                    .ok_or("invalid_preview_command")?;
                out["resolvedTitle"] = json!(title);
            }
            &pv::NAVIGATE
        }
        "resize" => {
            out["viewport"] = preference("browserDefaultViewport", &input["viewport"])?;
            &pv::RESIZE
        }
        "adjust" => {
            for (key, native) in [
                ("colorScheme", "browserDefaultAppearance"),
                ("zoomFactor", "browserDefaultZoomFactor"),
            ] {
                if let Some(v) = input.get(key) {
                    out[key] = preference(native, v)?;
                }
            }
            if let Some(v) = input.get("hardReload") {
                out["hardReload"] = json!(v.as_bool().ok_or("invalid_preview_command")?);
            }
            if let Some(v) = input.get("clear") {
                if !matches!(v.as_str(), Some("cookies" | "cache")) {
                    return Err("invalid_preview_command".into());
                }
                out["clear"] = v.clone();
            }
            if out.as_object().unwrap().len() == 2 {
                return Err("invalid_preview_command".into());
            }
            &pv::ADJUST
        }
        "refresh" => &pv::REFRESH,
        "close" => &pv::CLOSE,
        _ => return Err("invalid_preview_command".into()),
    };
    // The typed function checks the arguments too, before anything is reserved or sent.
    method.payload(&out).map_err(|_| "invalid_preview_command")?;
    Ok((method, out))
}
pub fn execute(e: &Environment, input: &Value) -> Result<Value> {
    let (method, payload) = payload(input)?;
    if input["environment"] != e.id {
        return Err("preview_scope_changed".into());
    }
    let request = string(&input["requestId"], 128)?;
    let expected_origin = string(&input["originKey"], 128)?;
    let expected_epoch = string(&input["serverEpoch"], 2048)?;
    let (c, context) = transport::inspect(e)?;
    if context["originKey"] != expected_origin {
        return Err("preview_scope_changed".into());
    }
    if context["canOperate"] != true {
        return Err("permission_denied".into());
    }
    if payload["runtime"] == "server" && context["serverBrowser"] != true {
        return Err("server_browser_unavailable".into());
    }
    let thread = text(input, "thread");
    let current = transport::list(&protocol::api::preview::list(&c, thread)?, thread)?;
    if current["serverEpoch"] != expected_epoch {
        return Err("preview_server_restarted".into());
    }
    if let Some(tab) = payload.get("tabId") {
        if !current["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| &s["tabId"] == tab)
        {
            return Err("preview_tab_closed".into());
        }
    }
    // Reserve before the first mutation, including malformed/lost replies.
    // Bounded admission fails closed instead of evicting replay protection.
    {
        let mut dispatched = DISPATCHED
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| "local_error")?;
        if dispatched.contains(request) {
            return Err("preview_command_replayed".into());
        }
        if dispatched.len() >= 4096 {
            return Err("preview_command_limit".into());
        }
        dispatched.insert(request.to_owned());
    }
    let result = method.call(&c, &payload, None);
    let mut receipt = json!({"environment":e.id,"thread":thread,"originKey":expected_origin,"serverEpoch":expected_epoch,"action":input["action"],"tabId":input["tabId"],"state":"uncertain","error":"preview_command_uncertain"});
    // Once dispatched, generic transport/RPC failure cannot establish that the
    // server did nothing. Never return raw error bodies or retry a mutation.
    if let Ok(result) = result {
        let checked = if [pv::REFRESH.name, pv::CLOSE.name].contains(&method.name) {
            Ok(Value::Null)
        } else {
            transport::snapshot(&result).and_then(|s| {
                if s["threadId"] == thread
                    && (method.name == pv::OPEN.name || s["tabId"] == input["tabId"])
                {
                    Ok(s)
                } else {
                    Err("invalid_preview_state".into())
                }
            })
        };
        if let Ok(snapshot) = checked {
            receipt["state"] = json!("complete");
            receipt["error"] = json!("");
            receipt["snapshot"] = snapshot;
        }
    }
    Ok(receipt)
}
