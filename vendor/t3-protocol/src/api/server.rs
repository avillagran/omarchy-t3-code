//! `server.*`: environment configuration and settings.
use super::{api_fn, ids, send, Args, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

pub const GET_CONFIG: Function = api_fn! {
    name: "server.getConfig", module: "server", tag: Some("server.getConfig"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [], payload: |_| Ok(json!({})),
};
pub const GET_SETTINGS: Function = api_fn! {
    name: "server.getSettings", module: "server", tag: Some("server.getSettings"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [], payload: |_| Ok(json!({})),
};
/// The scope depends on the patched keys (T3 `requiredScopesForServerSettingsPatch`)
/// and on `providerInstanceMutation`; the registry lists both candidates.
pub const UPDATE_SETTINGS: Function = api_fn! {
    name: "server.updateSettings", module: "server", tag: Some("server.updateSettings"),
    kind: IdempotentMutation, scope: Dynamic(&["settings:write", "providers:manage"]),
    args: [(patch, Object, required), (providerInstanceMutation, Object, optional)],
    payload: |a: &Args| {
        let mutation = a.object("providerInstanceMutation")?.map(ProviderInstanceMutation::from_json).transpose()?;
        update_settings_payload(a.object("patch")?.ok_or_else(|| ids::invalid("patch"))?, mutation.as_ref())
    },
};
pub const FUNCTIONS: &[Function] = &[GET_CONFIG, GET_SETTINGS, UPDATE_SETTINGS];

/// `server.getConfig`: `ServerConfig` (environment, providers, settings, capability flags…).
pub fn config(conn: &Connection) -> Result<Value> {
    send(conn, &GET_CONFIG, json!({}))
}
/// `server.getSettings`: `ServerSettings`. `settings::snapshot` derives the native view.
pub fn settings(conn: &Connection) -> Result<Value> {
    send(conn, &GET_SETTINGS, json!({}))
}
/// `server.updateSettings`: applies `patch` (a `ServerSettingsPatch` object, e.g.
/// `settings::patch(..)?["patch"]` or `models::favorite_payload(..)?["patch"]`) and
/// returns the full `ServerSettings`.
pub fn update_settings(
    conn: &Connection,
    patch: &Value,
    provider_instance_mutation: Option<&ProviderInstanceMutation>,
) -> Result<Value> {
    send(conn, &UPDATE_SETTINGS, update_settings_payload(patch, provider_instance_mutation)?)
}
fn update_settings_payload(patch: &Value, mutation: Option<&ProviderInstanceMutation>) -> Result<Value> {
    if !patch.is_object() {
        return Err(ids::invalid("patch"));
    }
    let mut payload = json!({"patch":patch});
    if let Some(m) = mutation {
        payload["providerInstanceMutation"] = m.to_json()?;
    }
    Ok(payload)
}

/// Atomic provider instance change carried by `server.updateSettings`.
#[derive(Clone, Debug, PartialEq)]
pub enum ProviderInstanceMutation {
    Create { instance_id: String, instance: Value },
    Upsert { instance_id: String, instance: Value },
    Remove { instance_id: String },
}
impl ProviderInstanceMutation {
    pub fn to_json(&self) -> Result<Value> {
        let invalid = || ids::invalid("providerInstanceMutation");
        let (operation, id, instance) = match self {
            Self::Create { instance_id, instance } => ("create", instance_id, Some(instance)),
            Self::Upsert { instance_id, instance } => ("upsert", instance_id, Some(instance)),
            Self::Remove { instance_id } => ("remove", instance_id, None),
        };
        ids::slug("instanceId", id).map_err(|_| invalid())?;
        let mut v = json!({"operation":operation,"instanceId":id});
        if let Some(instance) = instance {
            if !instance["driver"].as_str().is_some_and(|d| ids::slug("driver", d).is_ok()) {
                return Err(invalid());
            }
            v["instance"] = instance.clone();
        }
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let invalid = || ids::invalid("providerInstanceMutation");
        let o = v.as_object().ok_or_else(invalid)?;
        let id = v["instanceId"].as_str().ok_or_else(invalid)?.to_string();
        let instance = || v.get("instance").filter(|i| i.is_object()).cloned().ok_or_else(invalid);
        let keys = |allowed: &[&str]| o.keys().all(|k| allowed.contains(&k.as_str()));
        let m = match v["operation"].as_str() {
            Some("create") if keys(&["operation", "instanceId", "instance"]) => {
                Self::Create { instance_id: id, instance: instance()? }
            }
            Some("upsert") if keys(&["operation", "instanceId", "instance"]) => {
                Self::Upsert { instance_id: id, instance: instance()? }
            }
            Some("remove") if keys(&["operation", "instanceId"]) => Self::Remove { instance_id: id },
            _ => return Err(invalid()),
        };
        m.to_json()?;
        Ok(m)
    }
}
