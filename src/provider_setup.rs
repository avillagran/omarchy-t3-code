//! Provider install and sign-in from T3's provider settings (provider.install.* and
//! provider.auth.*). States are read from the server's subscriptions, first item only.
use crate::{json, text, write::rpc, Environment, Result};
use serde_json::Value;
use std::{cell::Cell, time::{Duration, Instant}};

fn first(e: &Environment, tag: &str, payload: Value) -> Result<Value> {
    let c = e.connection(true)?;
    let done = Cell::new(false);
    let started = Instant::now();
    let mut state = Value::Null;
    let result = c.subscribe(tag, payload, || done.get() || started.elapsed() > Duration::from_secs(10), |item| {
        if let Some(v) = item {
            state = v;
            done.set(true);
        }
        Ok(())
    });
    // A finite stream may close right after its state; only a missing state is an error.
    if state.is_null() {
        result?;
    }
    Ok(state)
}
fn op_id(v: &Value, key: &str) -> Result<String> {
    v[key].as_str().filter(|s| !s.trim().is_empty() && s.len() <= 128).map(str::to_string).ok_or("invalid_provider_setup".into())
}
/// The server keeps the typed value only for the flow; it never comes back.
pub fn response(v: &Value) -> Result<Value> {
    Ok(match text(v, "type") {
        "browser" => json!({"type":"browser","action":if v["action"]=="decline"{"decline"}else{"accept"}}),
        "credentials" => {
            let values = v["values"].as_object().filter(|o| o.len() <= 16 && o.iter().all(|(k, x)| k.len() <= 128 && x.as_str().is_some_and(|s| s.len() <= 16_384))).ok_or("invalid_provider_setup")?;
            json!({"type":"credentials","values":values})
        }
        "terminal" => json!({"type":"terminal","data":v["data"].as_str().filter(|s| s.len() <= 4096).ok_or("invalid_provider_setup")?}),
        _ => return Err("invalid_provider_setup".into()),
    })
}
pub fn run(e: &Environment, input: &Value) -> Result<Value> {
    let instance = op_id(input, "instanceId")?;
    let id = json!({"instanceId":instance});
    let (kind, state) = match text(input, "action") {
        "install" => ("install", rpc(e, "provider.install.start", id)?),
        "remove" => ("install", rpc(e, "provider.install.remove", id)?),
        "cancel-install" => ("install", rpc(e, "provider.install.cancel", json!({"instanceId":instance,"operationId":op_id(input,"operationId")?}))?),
        "install-state" => ("install", first(e, "provider.install.subscribe", id)?),
        "login" => {
            let mut p = id.clone();
            if let Ok(method) = op_id(input, "methodId") {
                p["methodId"] = json!(method);
            }
            ("auth", rpc(e, "provider.auth.start", p)?)
        }
        "logout" => ("auth", rpc(e, "provider.auth.logout", id)?),
        "cancel-login" => ("auth", rpc(e, "provider.auth.cancel", json!({"instanceId":instance,"flowId":op_id(input,"flowId")?}))?),
        "respond" => ("auth", rpc(e, "provider.auth.respond", json!({"instanceId":instance,"flowId":op_id(input,"flowId")?,"interactionId":op_id(input,"interactionId")?,"response":response(&input["response"])?}))?),
        "auth-state" => ("auth", first(e, "provider.auth.subscribe", id)?),
        "update" => {
            let driver = input["driver"].as_str().filter(|d| !d.is_empty() && d.len() <= 64).ok_or("invalid_provider_setup")?;
            ("update", crate::provider_updates::run_update(e, driver, &instance)?)
        }
        _ => return Err("invalid_command".into()),
    };
    Ok(json!({"instanceId":instance,"kind":kind,"state":state}))
}

#[cfg(test)]
mod tests {
    use super::response;
    use serde_json::json;
    #[test]
    fn responses_are_bounded() {
        assert_eq!(response(&json!({"type":"browser","action":"weird"})).unwrap()["action"], "accept");
        assert_eq!(response(&json!({"type":"credentials","values":{"apiKey":"k"}})).unwrap()["values"]["apiKey"], "k");
        assert!(response(&json!({"type":"credentials","values":{"apiKey":1}})).is_err());
        assert!(response(&json!({"type":"other"})).is_err());
    }
}
