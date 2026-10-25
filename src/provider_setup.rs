//! Provider install and sign-in from T3's provider settings (provider.install.* and
//! provider.auth.*). States are read from the server's subscriptions, first item only.
use crate::{json, protocol::api::{provider::{self, AuthResponse}, streams::{self, StreamSpec}}, text, Environment, Result};
use serde_json::Value;
use std::{cell::Cell, time::{Duration, Instant}};

/// Like `streams::install_state`/`auth_state`, but a stream that ends without a state reads as `null`.
fn first(e: &Environment, spec: StreamSpec) -> Result<Value> {
    let c = e.connection(true)?;
    let done = Cell::new(false);
    let started = Instant::now();
    let mut state = Value::Null;
    let result = spec.subscribe(&c, || done.get() || started.elapsed() > Duration::from_secs(10), || false, |item| {
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
    let c = || e.connection(true);
    let (kind, state) = match text(input, "action") {
        "install" => ("install", provider::install_start(&c()?, &instance)?),
        "remove" => ("install", provider::install_remove(&c()?, &instance)?),
        "cancel-install" => ("install", provider::install_cancel(&c()?, &instance, &op_id(input, "operationId")?)?),
        "install-state" => ("install", first(e, streams::provider_install_spec(&instance)?)?),
        "login" => ("auth", provider::auth_start(&c()?, &instance, op_id(input, "methodId").ok().as_deref(), None, None)?),
        "logout" => ("auth", provider::auth_logout(&c()?, &instance)?),
        "cancel-login" => ("auth", provider::auth_cancel(&c()?, &instance, &op_id(input, "flowId")?)?),
        "respond" => {
            let (flow, interaction) = (op_id(input, "flowId")?, op_id(input, "interactionId")?);
            let answer = AuthResponse::from_json(&response(&input["response"])?).map_err(|_| "invalid_provider_setup")?;
            ("auth", provider::auth_respond(&c()?, &instance, &flow, &interaction, &answer)?)
        }
        "auth-state" => ("auth", first(e, streams::provider_auth_spec(&instance)?)?),
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
