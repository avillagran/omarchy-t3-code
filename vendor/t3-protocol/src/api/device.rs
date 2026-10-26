//! `device.*`: emulators, simulators and device hosts. The deadlines are ot3's
//! (`rpc_with_timeout`). `crate::device_hub` (the hub's HTTP/WebSocket side)
//! is separate and unchanged.
use super::reader::{int, number, one_of, put, Reader};
use super::{api_fn, ids, send, ArgType, Function};
use crate::{Connection, Result};
use serde_json::{json, Value};

/// T3 `DeviceHostId` and `DeviceId` bounds.
const HOST_ID_MAX: usize = 128;
const DEVICE_ID_MAX: usize = 256;
const APP_ID_MAX: usize = 512;
pub const PLATFORMS: &[&str] = &["ios", "android"];
pub const UPDATE_TOOLS: &[&str] = &["hub", "agent"];
pub const ACTION_TYPES: &[&str] = &[
    "setAppearance", "setTextSize", "setToggle", "setLiquidGlass", "setColorFilter", "setOrientation", "setLocation",
    "clearLocation", "setPermission", "openUrl", "launchApp", "terminateApp", "shake", "sendPush",
];
pub const APPEARANCES: &[&str] = &["light", "dark"];
pub const TEXT_SIZES: &[&str] = &["small", "default", "large", "extra-large"];
pub const TOGGLES: &[&str] = &["reduceMotion", "increaseContrast", "reduceTransparency", "showBorders", "voiceOver", "networkEnabled"];
pub const LIQUID_GLASS: &[&str] = &["clear", "tinted"];
pub const COLOR_FILTERS: &[&str] = &["none", "grayscale", "red-green", "green-red", "blue-yellow"];
pub const ORIENTATIONS: &[&str] = &["portrait", "landscape_left", "portrait_upside_down", "landscape_right"];
pub const PERMISSIONS: &[&str] = &[
    "camera", "microphone", "photos", "contacts", "calendar", "reminders", "location", "notifications", "motion",
    "media-library", "faceid",
];
pub const DECISIONS: &[&str] = &["grant", "revoke", "reset"];

/// With `updateTool` or `retryHostId` (and not `inspectOnly` alone) the server
/// installs or restarts tools and requires `orchestration:operate`
/// (`ws.ts`, `requiredScopeForDeviceList`). ot3 waits up to 300 s.
pub const LIST: Function = api_fn! {
    name: "device.list", module: "device", tag: Some("device.list"),
    kind: Read, scope: Dynamic(&["orchestration:read", "orchestration:operate"]),
    args: [(updateTool, Enum(UPDATE_TOOLS), optional), (inspectOnly, Bool, optional), (retryHostId, Token, optional)],
    payload: |a| list_payload(a.str("updateTool")?, a.bool("inspectOnly")?, a.str("retryHostId")?),
    timeout_ms: 300_000,
};
pub const CONFIGURE: Function = api_fn! {
    name: "device.configure", module: "device", tag: Some("device.configure"),
    kind: IdempotentMutation, scope: Fixed("settings:write"),
    args: [(enabled, Bool, optional), (agentAccessEnabled, Bool, optional), (onboardingCompleted, Bool, optional)],
    payload: |a| Ok(configure_payload(a.bool("enabled")?, a.bool("agentAccessEnabled")?, a.bool("onboardingCompleted")?)),
    timeout_ms: 300_000,
};
pub const TEST_HOST: Function = api_fn! {
    name: "device.testHost", module: "device", tag: Some("device.testHost"),
    kind: Read, scope: Fixed("settings:write"),
    args: [
        (id, Token, required), (label, Text, required), (target, Text, required), (kind, Enum(&["ssh"]), optional),
        (identityFile, Path, optional), (port, Integer, optional),
    ],
    payload: |a| SshHost::from_json(a.all())?.to_json(),
    timeout_ms: 300_000,
};
pub const OPEN: Function = api_fn! {
    name: "device.open", module: "device", tag: Some("device.open"),
    kind: UnrepeatableMutation, scope: Fixed("orchestration:operate"),
    args: [(threadId, Id, required), (deviceId, Token, required), (platform, Enum(PLATFORMS), required), (hostId, Token, optional), (boot, Bool, optional)],
    payload: |a| open_payload(a.req("threadId")?, a.req("deviceId")?, a.req("platform")?, a.str("hostId")?, a.bool("boot")?),
    timeout_ms: 180_000,
};
/// The server rejects a shutdown that takes over 15 s; ot3 shows that as "shutting down".
pub const CLOSE: Function = api_fn! {
    name: "device.close", module: "device", tag: Some("device.close"),
    kind: IdempotentMutation, scope: Fixed("orchestration:operate"),
    args: [(threadId, Id, required), (hostId, Token, optional), (deviceId, Token, optional), (shutdown, Bool, optional)],
    payload: |a| close_payload(a.req("threadId")?, a.str("hostId")?, a.str("deviceId")?, a.bool("shutdown")?),
    timeout_ms: 60_000,
};
pub const DETAIL: Function = api_fn! {
    name: "device.detail", module: "device", tag: Some("device.detail"),
    kind: Read, scope: Fixed("orchestration:read"),
    args: [(deviceId, Token, required), (hostId, Token, optional)],
    payload: |a| target_payload(a.req("deviceId")?, a.str("hostId")?),
};
/// `set*` variants are idempotent, `shake`/`sendPush`/`launchApp` are not: the
/// function as a whole is `M!`.
pub const ACTION: Function = api_fn! {
    name: "device.action", module: "device", tag: Some("device.action"),
    kind: UnrepeatableMutation, scope: Fixed("orchestration:operate"),
    args: [(type, ArgType::Enum(ACTION_TYPES), required), (deviceId, Token, required), (hostId, Token, optional)],
    payload: |a| DeviceAction::from_json(a.all())?.payload(),
    timeout_ms: 30_000, open_args: true, variants: ACTION_TYPES,
};
pub const EAS_ACCOUNT: Function = api_fn! {
 name: "device.easAccount", module: "device", tag: Some("device.easAccount"),
 kind: Read, scope: Fixed("settings:write"), args: [(hostId, Token, optional),(accessToken, Text, optional)],
 payload: |a| {if let Some(id)=a.str("hostId")? {host_id(id)?;} Ok(a.all().clone())},
};
pub const FUNCTIONS: &[Function] = &[
    EAS_ACCOUNT,LIST, CONFIGURE, TEST_HOST, OPEN, CLOSE, DETAIL, ACTION];

fn host_id(value: &str) -> Result<&str> {
    ids::token("hostId", value, HOST_ID_MAX)
}
fn device_id(value: &str) -> Result<&str> {
    ids::token("deviceId", value, DEVICE_ID_MAX)
}
fn target_payload(device: &str, host: Option<&str>) -> Result<Value> {
    let mut v = json!({"deviceId":device_id(device)?});
    put(&mut v, "hostId", host.map(host_id).transpose()?);
    Ok(v)
}
/// `device.list`: `DeviceServiceState{hosts[], devices[], sessions[], …}`.
/// `update_tool` (`hub`/`agent`) or `retry_host_id` act on a host and need
/// `orchestration:operate`; see `list_scope`.
pub fn list(conn: &Connection, update_tool: Option<&str>, inspect_only: Option<bool>, retry_host_id: Option<&str>) -> Result<Value> {
    send(conn, &LIST, list_payload(update_tool, inspect_only, retry_host_id)?)
}
/// The scope T3 requires for a `device.list` payload.
pub fn list_scope(update_tool: Option<&str>, inspect_only: Option<bool>, retry_host_id: Option<&str>) -> &'static str {
    let inspect = inspect_only == Some(true) && update_tool.is_none();
    if !inspect && (update_tool.is_some() || retry_host_id.is_some()) {
        "orchestration:operate"
    } else {
        "orchestration:read"
    }
}
fn list_payload(update_tool: Option<&str>, inspect_only: Option<bool>, retry_host_id: Option<&str>) -> Result<Value> {
    let mut v = json!({});
    put(&mut v, "updateTool", update_tool.map(|t| one_of("updateTool", t, UPDATE_TOOLS)).transpose()?);
    put(&mut v, "inspectOnly", inspect_only);
    put(&mut v, "retryHostId", retry_host_id.map(|h| ids::token("retryHostId", h, HOST_ID_MAX)).transpose()?);
    Ok(v)
}
/// `device.configure`: `DeviceServiceState`.
pub fn configure(conn: &Connection, enabled: Option<bool>, agent_access_enabled: Option<bool>, onboarding_completed: Option<bool>) -> Result<Value> {
    send(conn, &CONFIGURE, configure_payload(enabled, agent_access_enabled, onboarding_completed))
}
fn configure_payload(enabled: Option<bool>, agent_access_enabled: Option<bool>, onboarding_completed: Option<bool>) -> Value {
    let mut v = json!({});
    put(&mut v, "enabled", enabled);
    put(&mut v, "agentAccessEnabled", agent_access_enabled);
    put(&mut v, "onboardingCompleted", onboarding_completed);
    v
}

/// `SshDeviceHostConfig` (`kind` is always `ssh`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshHost {
    /// `^[a-zA-Z0-9][a-zA-Z0-9_-]*$`, ≤ 128, not `local`.
    pub id: String,
    pub label: String,
    /// `user@host`: no whitespace, no leading `-`.
    pub target: String,
    pub identity_file: Option<String>,
    pub port: Option<u64>,
}
impl SshHost {
    pub fn to_json(&self) -> Result<Value> {
        let b = self.id.as_bytes();
        let id_ok = !b.is_empty()
            && b.len() <= HOST_ID_MAX
            && b[0].is_ascii_alphanumeric()
            && b.iter().all(|c| c.is_ascii_alphanumeric() || b"_-".contains(c))
            && self.id != "local";
        if !id_ok {
            return Err(ids::invalid("id"));
        }
        let target = self.target.trim();
        if target.is_empty() || target.starts_with('-') || target.chars().any(|c| c.is_whitespace() || c.is_control()) || target.len() > 1024 {
            return Err(ids::invalid("target"));
        }
        let mut v = json!({"kind":"ssh","id":self.id,"label":ids::text("label", &self.label, 255)?,"target":target});
        put(&mut v, "identityFile", self.identity_file.as_deref().map(|f| ids::path("identityFile", f).map(str::trim)).transpose()?);
        put(&mut v, "port", self.port.map(|p| int("port", p, 1, 65_535)).transpose()?);
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        if let Some(kind) = r.opt_str("kind")? {
            one_of("kind", &kind, &["ssh"])?;
        }
        let host = Self {
            id: r.req_str("id")?,
            label: r.req_str("label")?,
            target: r.req_str("target")?,
            identity_file: r.opt_str("identityFile")?,
            port: r.opt_u64("port")?,
        };
        r.done()?;
        Ok(host)
    }
}
/// `device.testHost`: `DeviceHostSummary`. A probe; guarded by `settings:write`.
pub fn test_host(conn: &Connection, host: &SshHost) -> Result<Value> {
    send(conn, &TEST_HOST, host.to_json()?)
}
/// `device.open`: `DeviceSession{threadId, hostId, deviceId, platform, openedAt}`.
/// Boots by default. Not repeatable: reconcile with `detail`.
pub fn open(conn: &Connection, thread_id: &str, device_id: &str, platform: &str, host_id: Option<&str>, boot: Option<bool>) -> Result<Value> {
    send(conn, &OPEN, open_payload(thread_id, device_id, platform, host_id, boot)?)
}
fn open_payload(thread_id: &str, device: &str, platform: &str, host: Option<&str>, boot: Option<bool>) -> Result<Value> {
    let mut v = target_payload(device, host)?;
    v["threadId"] = json!(ids::id("threadId", thread_id)?);
    v["platform"] = json!(one_of("platform", platform, PLATFORMS)?);
    put(&mut v, "boot", boot);
    Ok(v)
}
/// `device.close`: `null`. Without `device_id`, closes every device of the thread.
pub fn close(conn: &Connection, thread_id: &str, host_id: Option<&str>, device_id: Option<&str>, shutdown: Option<bool>) -> Result<Value> {
    send(conn, &CLOSE, close_payload(thread_id, host_id, device_id, shutdown)?)
}
fn close_payload(thread_id: &str, host: Option<&str>, device: Option<&str>, shutdown: Option<bool>) -> Result<Value> {
    let mut v = json!({"threadId":ids::id("threadId", thread_id)?});
    put(&mut v, "hostId", host.map(host_id).transpose()?);
    put(&mut v, "deviceId", device.map(device_id).transpose()?);
    put(&mut v, "shutdown", shutdown);
    Ok(v)
}
/// `device.detail`: `DeviceDetail{hostId, deviceId, settings, foregroundApp, readAt}`.
pub fn detail(conn: &Connection, device_id: &str, host_id: Option<&str>) -> Result<Value> {
    send(conn, &DETAIL, target_payload(device_id, host_id)?)
}

/// What `device.action` does (`DeviceActionInput` without the target).
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    SetAppearance(String),
    SetTextSize(String),
    SetToggle { setting: String, value: bool },
    SetLiquidGlass(String),
    SetColorFilter(String),
    SetOrientation(String),
    SetLocation { latitude: f64, longitude: f64 },
    ClearLocation,
    SetPermission { app_id: String, permission: String, decision: String },
    OpenUrl(String),
    LaunchApp(String),
    TerminateApp(String),
    Shake,
    /// `payload` is a JSON string or an object.
    SendPush { app_id: String, payload: Value },
}
/// `device.action` on one device.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceAction {
    pub device_id: String,
    pub host_id: Option<String>,
    pub action: Action,
}
fn app_id(value: &str) -> Result<&str> {
    ids::text("appId", value, APP_ID_MAX)
}
impl DeviceAction {
    pub fn payload(&self) -> Result<Value> {
        let mut v = target_payload(&self.device_id, self.host_id.as_deref())?;
        let value = |kind: &str, value: &str, allowed: &[&str]| -> Result<Value> { Ok(json!({"type":kind,"value":one_of("value", value, allowed)?})) };
        let fields = match &self.action {
            Action::SetAppearance(a) => value("setAppearance", a, APPEARANCES)?,
            Action::SetTextSize(s) => value("setTextSize", s, TEXT_SIZES)?,
            Action::SetToggle { setting, value } => {
                json!({"type":"setToggle","setting":one_of("setting", setting, TOGGLES)?,"value":value})
            }
            Action::SetLiquidGlass(g) => value("setLiquidGlass", g, LIQUID_GLASS)?,
            Action::SetColorFilter(f) => value("setColorFilter", f, COLOR_FILTERS)?,
            Action::SetOrientation(o) => value("setOrientation", o, ORIENTATIONS)?,
            Action::SetLocation { latitude, longitude } => json!({"type":"setLocation",
                "latitude":number("latitude", *latitude, -90.0, 90.0)?,"longitude":number("longitude", *longitude, -180.0, 180.0)?}),
            Action::ClearLocation => json!({"type":"clearLocation"}),
            Action::SetPermission { app_id: app, permission, decision } => json!({"type":"setPermission","appId":app_id(app)?,
                "permission":one_of("permission", permission, PERMISSIONS)?,"decision":one_of("decision", decision, DECISIONS)?}),
            Action::OpenUrl(url) => {
                let url = ids::text("url", url, 4096)?;
                if url.chars().any(char::is_control) {
                    return Err(ids::invalid("url"));
                }
                json!({"type":"openUrl","url":url})
            }
            Action::LaunchApp(app) => json!({"type":"launchApp","appId":app_id(app)?}),
            Action::TerminateApp(app) => json!({"type":"terminateApp","appId":app_id(app)?}),
            Action::Shake => json!({"type":"shake"}),
            Action::SendPush { app_id: app, payload } => {
                if !(payload.is_string() || payload.is_object()) || payload.to_string().len() > 1024 * 1024 {
                    return Err(ids::invalid("payload"));
                }
                json!({"type":"sendPush","appId":app_id(app)?,"payload":payload})
            }
        };
        for (k, field) in fields.as_object().into_iter().flatten() {
            v[k] = field.clone();
        }
        Ok(v)
    }
    pub fn from_json(v: &Value) -> Result<Self> {
        let mut r = Reader::new(v)?;
        let device_id = r.req_str("deviceId")?;
        let host_id = r.opt_str("hostId")?;
        let action = match r.req_str("type").map_err(|_| ids::invalid("type"))?.as_str() {
            "setAppearance" => Action::SetAppearance(r.req_str("value")?),
            "setTextSize" => Action::SetTextSize(r.req_str("value")?),
            "setToggle" => Action::SetToggle { setting: r.req_str("setting")?, value: r.req_bool("value")? },
            "setLiquidGlass" => Action::SetLiquidGlass(r.req_str("value")?),
            "setColorFilter" => Action::SetColorFilter(r.req_str("value")?),
            "setOrientation" => Action::SetOrientation(r.req_str("value")?),
            "setLocation" => Action::SetLocation { latitude: r.req_f64("latitude")?, longitude: r.req_f64("longitude")? },
            "clearLocation" => Action::ClearLocation,
            "setPermission" => Action::SetPermission { app_id: r.req_str("appId")?, permission: r.req_str("permission")?, decision: r.req_str("decision")? },
            "openUrl" => Action::OpenUrl(r.req_str("url")?),
            "launchApp" => Action::LaunchApp(r.req_str("appId")?),
            "terminateApp" => Action::TerminateApp(r.req_str("appId")?),
            "shake" => Action::Shake,
            "sendPush" => Action::SendPush { app_id: r.req_str("appId")?, payload: r.req_value("payload")? },
            _ => return Err(ids::invalid("type")),
        };
        r.done()?;
        let action = Self { device_id, host_id, action };
        action.payload()?;
        Ok(action)
    }
}
/// `device.action`: the device's `DeviceDetail` after the action.
pub fn action(conn: &Connection, action: &DeviceAction) -> Result<Value> {
    send(conn, &ACTION, action.payload()?)
}
pub fn eas_account(conn: &Connection, host: Option<&str>, token: Option<&str>) -> Result<Value> {
    let mut args=json!({});put(&mut args,"hostId",host);put(&mut args,"accessToken",token);
    EAS_ACCOUNT.call(conn,&args,None)
}
