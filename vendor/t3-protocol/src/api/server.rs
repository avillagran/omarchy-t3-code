//! `server.*`: environment configuration, settings, keybindings, client
//! activity and the ACP registry. Diagnostics and providers have their own modules.
use super::reader::{one_of, put, timestamp, Reader};
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
pub const DISCOVER_SOURCE_CONTROL: Function = api_fn! {
    name: "server.discoverSourceControl", module: "server", tag: Some("server.discoverSourceControl"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [], payload: |_| Ok(json!({})),
};
pub const REPORT_CLIENT_ACTIVITY: Function = api_fn! {
    name: "server.reportClientActivity", module: "server", tag: Some("server.reportClientActivity"),
    kind: IdempotentMutation, scope: Fixed("orchestration:read"),
    args: [
        (clientId, Token, required), (clientKind, Enum(CLIENT_KINDS), required), (visible, Bool, required),
        (focused, Bool, required), (recentlyInteracted, Bool, required), (scopes, Array, required),
        (observedAt, Text, required), (environmentId, Id, optional), (appState, Enum(APP_STATES), optional),
        (lowPowerMode, Enum(LOW_POWER_MODES), optional), (batteryState, Enum(BATTERY_STATES), optional),
        (networkType, Text, optional), (ttlMs, Integer, optional),
    ],
    payload: |a| ClientActivity::from_json(a.all())?.payload(),
};
pub const UPSERT_KEYBINDING: Function = api_fn! {
    name: "server.upsertKeybinding", module: "server", tag: Some("server.upsertKeybinding"),
    kind: IdempotentMutation, scope: Fixed("settings:write"),
    args: [(key, Text, required), (command, Text, required), (when, Text, optional), (replace, Object, optional)],
    payload: |a| {
        let replace = a.object("replace")?.map(Keybinding::from_json).transpose().map_err(|_| ids::invalid("replace"))?;
        upsert_keybinding_payload(&Keybinding::from_args(a)?, replace.as_ref())
    },
};
pub const REMOVE_KEYBINDING: Function = api_fn! {
    name: "server.removeKeybinding", module: "server", tag: Some("server.removeKeybinding"),
    kind: IdempotentMutation, scope: Fixed("settings:write"),
    args: [(key, Text, required), (command, Text, required), (when, Text, optional)],
    payload: |a| Keybinding::from_args(a)?.to_json(),
};
pub const SEARCH_ACP_REGISTRY: Function = api_fn! {
    name: "server.searchAcpRegistry", module: "server", tag: Some("server.searchAcpRegistry"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(query, Text, required), (limit, Integer, optional)],
    payload: |a| search_acp_registry_payload(a.req("query")?, a.u64("limit")?),
};
pub const PREPARE_ACP_REGISTRY_AGENT: Function = api_fn! {
    name: "server.prepareAcpRegistryAgent", module: "server", tag: Some("server.prepareAcpRegistryAgent"),
    kind: IdempotentMutation, scope: Fixed("providers:manage"),
    args: [(agentId, Token, required)],
    payload: |a| prepare_acp_registry_agent_payload(a.req("agentId")?),
};
pub const FUNCTIONS: &[Function] = &[
    GET_CONFIG, GET_SETTINGS, UPDATE_SETTINGS, DISCOVER_SOURCE_CONTROL, REPORT_CLIENT_ACTIVITY,
    UPSERT_KEYBINDING, REMOVE_KEYBINDING, SEARCH_ACP_REGISTRY, PREPARE_ACP_REGISTRY_AGENT,
];

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

/// `server.discoverSourceControl`: `{versionControlSystems[], sourceControlProviders[]}`.
pub fn discover_source_control(conn: &Connection) -> Result<Value> {
    send(conn, &DISCOVER_SOURCE_CONTROL, json!({}))
}

pub const CLIENT_KINDS: &[&str] = &["web", "desktop-renderer", "mobile", "unknown"];
pub const APP_STATES: &[&str] = &["active", "inactive", "background", "unknown"];
/// Strings, not booleans: T3 `ClientActivityReportInput.lowPowerMode`.
pub const LOW_POWER_MODES: &[&str] = &["true", "false", "unknown"];
pub const BATTERY_STATES: &[&str] = &["unknown", "unplugged", "charging", "full"];
/// T3 `ClientActivityScope` types.
pub const ACTIVITY_SCOPES: &[&str] = &["server-config", "provider-status", "vcs-status", "git-refs", "diagnostics", "thread"];
/// `ClientActivityReportInput` (T3 `background.ts`): what this client is looking at,
/// so the server can keep those reads warm. `observed_at` is ISO 8601 UTC.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientActivity {
    pub client_id: String,
    pub client_kind: String,
    pub visible: bool,
    pub focused: bool,
    pub recently_interacted: bool,
    /// `{type:"server-config"}`, `{type:"provider-status",instanceId?}`, `{type:"vcs-status"|"git-refs",cwd}`,
    /// `{type:"diagnostics"}`, `{type:"thread",threadId}`.
    pub scopes: Vec<Value>,
    pub observed_at: String,
    pub environment_id: Option<String>,
    pub app_state: Option<String>,
    pub low_power_mode: Option<String>,
    pub battery_state: Option<String>,
    pub network_type: Option<String>,
    /// The server clamps it to 1 000..=120 000 (default 45 000).
    pub ttl_ms: Option<u64>,
}
impl ClientActivity {
    pub fn payload(&self) -> Result<Value> {
        let scopes = self.scopes.iter().map(activity_scope).collect::<Result<Vec<_>>>()?;
        let mut v = json!({
            "clientId":ids::token("clientId", &self.client_id, 128)?,
            "clientKind":one_of("clientKind", &self.client_kind, CLIENT_KINDS)?,
            "visible":self.visible, "focused":self.focused, "recentlyInteracted":self.recently_interacted,
            "scopes":scopes, "observedAt":timestamp("observedAt", &self.observed_at)?,
        });
        put(&mut v, "environmentId", self.environment_id.as_deref().map(|e| ids::id("environmentId", e)).transpose()?);
        put(&mut v, "appState", self.app_state.as_deref().map(|s| one_of("appState", s, APP_STATES)).transpose()?);
        put(&mut v, "lowPowerMode", self.low_power_mode.as_deref().map(|s| one_of("lowPowerMode", s, LOW_POWER_MODES)).transpose()?);
        put(&mut v, "batteryState", self.battery_state.as_deref().map(|s| one_of("batteryState", s, BATTERY_STATES)).transpose()?);
        put(&mut v, "networkType", self.network_type.as_deref().map(|s| ids::text("networkType", s, 64)).transpose()?);
        put(&mut v, "ttlMs", self.ttl_ms);
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let activity = Self {
            client_id: r.req_str("clientId")?,
            client_kind: r.req_str("clientKind")?,
            visible: r.req_bool("visible")?,
            focused: r.req_bool("focused")?,
            recently_interacted: r.req_bool("recentlyInteracted")?,
            scopes: r.req_array("scopes")?,
            observed_at: r.req_str("observedAt")?,
            environment_id: r.opt_str("environmentId")?,
            app_state: r.opt_str("appState")?,
            low_power_mode: r.opt_str("lowPowerMode")?,
            battery_state: r.opt_str("batteryState")?,
            network_type: r.opt_str("networkType")?,
            ttl_ms: r.opt_u64("ttlMs")?,
        };
        r.done()?;
        Ok(activity)
    }
}
fn activity_scope(v: &Value) -> Result<Value> {
    let invalid = || ids::invalid("scopes");
    let mut r = Reader::of("scopes", v)?;
    let kind = r.req_str("type").map_err(|_| invalid())?;
    let scope = match kind.as_str() {
        "server-config" | "diagnostics" => json!({"type":kind}),
        "provider-status" => {
            let mut s = json!({"type":kind});
            put(&mut s, "instanceId", r.opt_str("instanceId").map_err(|_| invalid())?.map(|i| ids::slug("scopes", &i).map(str::to_string)).transpose()?);
            s
        }
        "vcs-status" | "git-refs" => json!({"type":kind,"cwd":ids::path("scopes", &r.req_str("cwd").map_err(|_| invalid())?)?}),
        "thread" => json!({"type":kind,"threadId":ids::id("scopes", &r.req_str("threadId").map_err(|_| invalid())?)?}),
        _ => return Err(invalid()),
    };
    r.done().map_err(|_| invalid())?;
    Ok(scope)
}
/// `server.reportClientActivity`: `null`. Repeating a report only refreshes it.
pub fn report_client_activity(conn: &Connection, activity: &ClientActivity) -> Result<Value> {
    send(conn, &REPORT_CLIENT_ACTIVITY, activity.payload()?)
}

/// T3 `MAX_KEYBINDING_VALUE_LENGTH` and the `when` bound.
const KEY_MAX: usize = 64;
const WHEN_MAX: usize = 256;
/// One keybinding rule (`ServerUpsertKeybindingReplaceTarget`). `command` is a T3
/// `KeybindingCommand` (a static command or `script.<id>.run`); the library checks
/// its shape and the server checks the literal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keybinding {
    pub key: String,
    pub command: String,
    pub when: Option<String>,
}
impl Keybinding {
    pub fn to_json(&self) -> Result<Value> {
        let command = ids::text("command", &self.command, 128)?;
        if !command.bytes().all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c)) {
            return Err(ids::invalid("command"));
        }
        let mut v = json!({"key":ids::text("key", &self.key, KEY_MAX)?,"command":command});
        put(&mut v, "when", self.when.as_deref().map(|w| ids::text("when", w, WHEN_MAX)).transpose()?);
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let binding = Self { key: r.req_str("key")?, command: r.req_str("command")?, when: r.opt_str("when")? };
        r.done()?;
        binding.to_json()?;
        Ok(binding)
    }
    fn from_args(a: &Args) -> Result<Self> {
        Ok(Self { key: a.req("key")?.into(), command: a.req("command")?.into(), when: a.str("when")?.map(Into::into) })
    }
}
/// `server.upsertKeybinding`: `{keybindings, issues}`. With `replace`, the server
/// swaps that rule for `binding`.
pub fn upsert_keybinding(conn: &Connection, binding: &Keybinding, replace: Option<&Keybinding>) -> Result<Value> {
    send(conn, &UPSERT_KEYBINDING, upsert_keybinding_payload(binding, replace)?)
}
fn upsert_keybinding_payload(binding: &Keybinding, replace: Option<&Keybinding>) -> Result<Value> {
    let mut v = binding.to_json()?;
    put(&mut v, "replace", replace.map(|r| r.to_json().map_err(|_| ids::invalid("replace"))).transpose()?);
    Ok(v)
}
/// `server.removeKeybinding`: `{keybindings, issues}`.
pub fn remove_keybinding(conn: &Connection, binding: &Keybinding) -> Result<Value> {
    send(conn, &REMOVE_KEYBINDING, binding.to_json()?)
}

/// T3 `ACP_REGISTRY_MAX_AGENTS`.
const ACP_LIMIT_MAX: u64 = 512;
/// `server.searchAcpRegistry`: `{agents[]}`. `query` may be blank (lists everything).
pub fn search_acp_registry(conn: &Connection, query: &str, limit: Option<u64>) -> Result<Value> {
    send(conn, &SEARCH_ACP_REGISTRY, search_acp_registry_payload(query, limit)?)
}
fn search_acp_registry_payload(query: &str, limit: Option<u64>) -> Result<Value> {
    let query = query.trim();
    if query.chars().count() > 120 || query.chars().any(char::is_control) {
        return Err(ids::invalid("query"));
    }
    let mut v = json!({"query":query});
    put(&mut v, "limit", limit.map(|l| super::reader::int("limit", l, 1, ACP_LIMIT_MAX)).transpose()?);
    Ok(v)
}
/// `server.prepareAcpRegistryAgent`: `{agentId, version, distribution, prepared}`.
/// Downloads the agent on the server; preparing it again is harmless.
pub fn prepare_acp_registry_agent(conn: &Connection, agent_id: &str) -> Result<Value> {
    send(conn, &PREPARE_ACP_REGISTRY_AGENT, prepare_acp_registry_agent_payload(agent_id)?)
}
fn prepare_acp_registry_agent_payload(agent_id: &str) -> Result<Value> {
    // T3 `AcpRegistryAgentId`: `^[a-z0-9][a-z0-9._-]*$`, ≤ 128.
    let b = agent_id.as_bytes();
    let ok = !b.is_empty()
        && b.len() <= 128
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(c));
    if !ok {
        return Err(ids::invalid("agentId"));
    }
    Ok(json!({"agentId":agent_id}))
}
