//! `provider`: provider discovery, updates, managed installs and sign-in flows
//! (`server.refreshProviders`, `server.updateProvider`, `provider.auth.*`,
//! `provider.install.*`). Everything but the refresh needs `providers:manage`.
//! `AuthResponse::Credentials` carries user secrets: its `Debug` redacts them and
//! no error string ever contains a value.
use super::reader::{int, one_of, put, Reader};
use super::{api_fn, ids, send, Function};
use crate::{Connection, Result};
use serde_json::{json, Map, Value};
use std::fmt;

/// T3 `SetupOperationId` bound (`flowId`, `operationId`, `interactionId`, `methodId`).
const OPERATION_ID_MAX: usize = 128;
pub const CALLBACK_MODES: &[&str] = &["server", "client"];

pub const REFRESH_PROVIDERS: Function = api_fn! {
    name: "server.refreshProviders", module: "provider", tag: Some("server.refreshProviders"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(instanceId, Slug, optional), (cwd, Path, optional), (fresh, Bool, optional), (refreshModels, Bool, optional)],
    payload: |a| refresh_payload(a.str("instanceId")?, a.str("cwd")?, a.bool("fresh")?, a.bool("refreshModels")?),
};
/// ot3 waits up to 300 s for a provider update.
pub const UPDATE_PROVIDER: Function = api_fn! {
    name: "server.updateProvider", module: "provider", tag: Some("server.updateProvider"),
    kind: UnrepeatableMutation, scope: Fixed("providers:manage"),
    args: [(provider, Slug, required), (targetVersion, Text, optional), (instanceId, Slug, optional)],
    payload: |a| update_payload(a.req("provider")?, a.str("targetVersion")?, a.str("instanceId")?),
    timeout_ms: 300_000,
};
pub const AUTH_START: Function = api_fn! {
    name: "provider.auth.start", module: "provider", tag: Some("provider.auth.start"),
    kind: UnrepeatableMutation, scope: Fixed("providers:manage"),
    args: [(instanceId, Slug, required), (methodId, Token, optional), (returnUrl, Text, optional), (callbackMode, Enum(CALLBACK_MODES), optional)],
    payload: |a| auth_start_payload(a.req("instanceId")?, a.str("methodId")?, a.str("returnUrl")?, a.str("callbackMode")?),
};
pub const AUTH_RESPOND: Function = api_fn! {
    name: "provider.auth.respond", module: "provider", tag: Some("provider.auth.respond"),
    kind: UnrepeatableMutation, scope: Fixed("providers:manage"),
    args: [(instanceId, Slug, required), (flowId, Token, required), (interactionId, Token, required), (response, Object, required)],
    payload: |a| auth_respond_payload(a.req("instanceId")?, a.req("flowId")?, a.req("interactionId")?, &AuthResponse::from_json(a.req_object("response")?)?),
};
pub const AUTH_CANCEL: Function = api_fn! {
    name: "provider.auth.cancel", module: "provider", tag: Some("provider.auth.cancel"),
    kind: IdempotentMutation, scope: Fixed("providers:manage"),
    args: [(instanceId, Slug, required), (flowId, Token, required)],
    payload: |a| auth_cancel_payload(a.req("instanceId")?, a.req("flowId")?),
};
pub const AUTH_LOGOUT: Function = api_fn! {
    name: "provider.auth.logout", module: "provider", tag: Some("provider.auth.logout"),
    kind: IdempotentMutation, scope: Fixed("providers:manage"),
    args: [(instanceId, Slug, required)],
    payload: |a| instance_payload(a.req("instanceId")?),
};
pub const INSTALL_START: Function = api_fn! {
    name: "provider.install.start", module: "provider", tag: Some("provider.install.start"),
    kind: UnrepeatableMutation, scope: Fixed("providers:manage"),
    args: [(instanceId, Slug, required)],
    payload: |a| instance_payload(a.req("instanceId")?),
};
pub const INSTALL_CANCEL: Function = api_fn! {
    name: "provider.install.cancel", module: "provider", tag: Some("provider.install.cancel"),
    kind: IdempotentMutation, scope: Fixed("providers:manage"),
    args: [(instanceId, Slug, required), (operationId, Token, required)],
    payload: |a| install_cancel_payload(a.req("instanceId")?, a.req("operationId")?),
};
pub const INSTALL_REMOVE: Function = api_fn! {
    name: "provider.install.remove", module: "provider", tag: Some("provider.install.remove"),
    kind: IdempotentMutation, scope: Fixed("providers:manage"),
    args: [(instanceId, Slug, required)],
    payload: |a| instance_payload(a.req("instanceId")?),
};
pub const FUNCTIONS: &[Function] = &[
    REFRESH_PROVIDERS, UPDATE_PROVIDER, AUTH_START, AUTH_RESPOND, AUTH_CANCEL, AUTH_LOGOUT,
    INSTALL_START, INSTALL_CANCEL, INSTALL_REMOVE,
];

/// `server.refreshProviders`: `{providers}`. `cwd` only applies with `instance_id`;
/// `refresh_models` forces model rediscovery and should follow an explicit user request.
pub fn refresh(conn: &Connection, instance_id: Option<&str>, cwd: Option<&str>, fresh: Option<bool>, refresh_models: Option<bool>) -> Result<Value> {
    send(conn, &REFRESH_PROVIDERS, refresh_payload(instance_id, cwd, fresh, refresh_models)?)
}
fn refresh_payload(instance_id: Option<&str>, cwd: Option<&str>, fresh: Option<bool>, refresh_models: Option<bool>) -> Result<Value> {
    let mut v = json!({});
    put(&mut v, "instanceId", instance_id.map(|i| ids::slug("instanceId", i)).transpose()?);
    put(&mut v, "cwd", cwd.map(|c| ids::path("cwd", c)).transpose()?);
    put(&mut v, "fresh", fresh);
    put(&mut v, "refreshModels", refresh_models);
    Ok(v)
}
/// `server.updateProvider`: `{providers}`. Runs an installer: not repeatable;
/// reconcile with `server.getConfig` (`providers[].version`).
pub fn update(conn: &Connection, provider: &str, target_version: Option<&str>, instance_id: Option<&str>) -> Result<Value> {
    send(conn, &UPDATE_PROVIDER, update_payload(provider, target_version, instance_id)?)
}
fn update_payload(provider: &str, target_version: Option<&str>, instance_id: Option<&str>) -> Result<Value> {
    let mut v = json!({"provider":ids::slug("provider", provider)?});
    put(&mut v, "targetVersion", target_version.map(|t| ids::text("targetVersion", t, 256)).transpose()?);
    put(&mut v, "instanceId", instance_id.map(|i| ids::slug("instanceId", i)).transpose()?);
    Ok(v)
}
fn operation_id<'a>(field: &str, value: &'a str) -> Result<&'a str> {
    ids::token(field, value, OPERATION_ID_MAX)
}
fn instance_payload(instance_id: &str) -> Result<Value> {
    Ok(json!({"instanceId":ids::slug("instanceId", instance_id)?}))
}
/// `provider.auth.start`: `ProviderAuthState`. Starts a sign-in flow: not repeatable.
pub fn auth_start(conn: &Connection, instance_id: &str, method_id: Option<&str>, return_url: Option<&str>, callback_mode: Option<&str>) -> Result<Value> {
    send(conn, &AUTH_START, auth_start_payload(instance_id, method_id, return_url, callback_mode)?)
}
fn auth_start_payload(instance_id: &str, method_id: Option<&str>, return_url: Option<&str>, callback_mode: Option<&str>) -> Result<Value> {
    let mut v = instance_payload(instance_id)?;
    put(&mut v, "methodId", method_id.map(|m| operation_id("methodId", m)).transpose()?);
    put(&mut v, "returnUrl", return_url.map(|u| ids::path("returnUrl", u)).transpose()?);
    put(&mut v, "callbackMode", callback_mode.map(|m| one_of("callbackMode", m, CALLBACK_MODES)).transpose()?);
    Ok(v)
}

/// T3 `ProviderAuthResponse` bounds.
const TERMINAL_DATA_MAX: usize = 4096;
const CREDENTIAL_VALUE_MAX: usize = 16_384;
const CREDENTIALS_MAX: usize = 16;
/// An answer to a provider sign-in interaction (`ProviderAuthResponse`).
#[derive(Clone, PartialEq)]
pub enum AuthResponse {
    /// `accept` or `decline`.
    Browser { action: String },
    /// Keystrokes for a terminal interaction (may be empty), and the terminal size.
    Terminal { data: String, size: Option<(u64, u64)> },
    /// Credential values by field id. Secret: never logged nor echoed.
    Credentials { values: Map<String, Value> },
}
impl fmt::Debug for AuthResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Browser { action } => f.debug_struct("Browser").field("action", action).finish(),
            Self::Terminal { size, .. } => f.debug_struct("Terminal").field("data", &"[redacted]").field("size", size).finish(),
            Self::Credentials { values } => f.debug_struct("Credentials").field("keys", &values.keys().collect::<Vec<_>>()).finish(),
        }
    }
}
impl AuthResponse {
    pub fn to_json(&self) -> Result<Value> {
        let invalid = || ids::invalid("response");
        Ok(match self {
            Self::Browser { action } => json!({"type":"browser","action":one_of("response", action, &["accept", "decline"])?}),
            Self::Terminal { data, size } => {
                if data.chars().count() > TERMINAL_DATA_MAX {
                    return Err(invalid());
                }
                let mut v = json!({"type":"terminal","data":data});
                if let Some((cols, rows)) = size {
                    v["size"] = json!({"cols":int("response", *cols, 1, 500)?,"rows":int("response", *rows, 1, 200)?});
                }
                v
            }
            Self::Credentials { values } => {
                let ok = values.len() <= CREDENTIALS_MAX
                    && values.iter().all(|(k, v)| {
                        operation_id("response", k).is_ok() && v.as_str().is_some_and(|s| s.chars().count() <= CREDENTIAL_VALUE_MAX)
                    });
                if !ok {
                    return Err(invalid());
                }
                json!({"type":"credentials","values":values})
            }
        })
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let invalid = || ids::invalid("response");
        let mut r = Reader::of("response", v)?;
        let response = match r.req_str("type").map_err(|_| invalid())?.as_str() {
            "browser" => Self::Browser { action: r.req_str("action").map_err(|_| invalid())? },
            "terminal" => {
                let data = r.raw("data").and_then(Value::as_str).ok_or_else(invalid)?.to_string();
                let size = match r.raw("size") {
                    None => None,
                    Some(s) => {
                        let mut s = Reader::of("response", s)?;
                        let size = (s.req_u64("cols").map_err(|_| invalid())?, s.req_u64("rows").map_err(|_| invalid())?);
                        s.done().map_err(|_| invalid())?;
                        Some(size)
                    }
                };
                Self::Terminal { data, size }
            }
            "credentials" => Self::Credentials { values: r.raw("values").and_then(Value::as_object).cloned().ok_or_else(invalid)? },
            _ => return Err(invalid()),
        };
        r.done().map_err(|_| invalid())?;
        response.to_json()?;
        Ok(response)
    }
}
/// `provider.auth.respond`: `ProviderAuthState`. Not repeatable: a second answer
/// can land on the next interaction.
pub fn auth_respond(conn: &Connection, instance_id: &str, flow_id: &str, interaction_id: &str, response: &AuthResponse) -> Result<Value> {
    send(conn, &AUTH_RESPOND, auth_respond_payload(instance_id, flow_id, interaction_id, response)?)
}
fn auth_respond_payload(instance_id: &str, flow_id: &str, interaction_id: &str, response: &AuthResponse) -> Result<Value> {
    let mut v = instance_payload(instance_id)?;
    v["flowId"] = json!(operation_id("flowId", flow_id)?);
    v["interactionId"] = json!(operation_id("interactionId", interaction_id)?);
    v["response"] = response.to_json()?;
    Ok(v)
}
/// `provider.auth.cancel`: `ProviderAuthState`.
pub fn auth_cancel(conn: &Connection, instance_id: &str, flow_id: &str) -> Result<Value> {
    send(conn, &AUTH_CANCEL, auth_cancel_payload(instance_id, flow_id)?)
}
fn auth_cancel_payload(instance_id: &str, flow_id: &str) -> Result<Value> {
    let mut v = instance_payload(instance_id)?;
    v["flowId"] = json!(operation_id("flowId", flow_id)?);
    Ok(v)
}
/// `provider.auth.logout`: `ProviderAuthState`.
pub fn auth_logout(conn: &Connection, instance_id: &str) -> Result<Value> {
    send(conn, &AUTH_LOGOUT, instance_payload(instance_id)?)
}
/// `provider.install.start`: `ProviderInstallState`. Starts a download: not repeatable.
pub fn install_start(conn: &Connection, instance_id: &str) -> Result<Value> {
    send(conn, &INSTALL_START, instance_payload(instance_id)?)
}
/// `provider.install.cancel`: `ProviderInstallState`.
pub fn install_cancel(conn: &Connection, instance_id: &str, operation_id: &str) -> Result<Value> {
    send(conn, &INSTALL_CANCEL, install_cancel_payload(instance_id, operation_id)?)
}
fn install_cancel_payload(instance_id: &str, operation: &str) -> Result<Value> {
    let mut v = instance_payload(instance_id)?;
    v["operationId"] = json!(operation_id("operationId", operation)?);
    Ok(v)
}
/// `provider.install.remove`: `ProviderInstallState`.
pub fn install_remove(conn: &Connection, instance_id: &str) -> Result<Value> {
    send(conn, &INSTALL_REMOVE, instance_payload(instance_id)?)
}
