//! Native Integrations device management. T3 owns discovery, SSH, pinned tool
//! installation and lifecycle. The client never executes adb/SSH/installers.
use crate::{json, protocol, text, Environment, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

static VIEWS: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
static OPERATIONS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
fn views() -> &'static Mutex<HashMap<String, Value>> {
    VIEWS.get_or_init(Default::default)
}
fn connection(e: &Environment) -> Result<protocol::Connection> {
    if let Some(error) = &e.connection_error {
        return Err(error.clone());
    }
    if let Some(c) = &e.connection {
        return Ok(c.as_ref().clone());
    }
    protocol::Connection::new(
        &e.base_url,
        None,
        protocol::Credential::bearer(&crate::environment_token(e, true)?)?,
    )
}
fn roles(c: &protocol::Connection) -> Result<Value> {
    let s = c.request_json("GET", "/api/auth/session", None, false)?;
    let scopes = s
        .get("permissions")
        .or_else(|| s.get("scopes"))
        .and_then(Value::as_array);
    let has = |scope: &str| {
        s["authenticated"] == true && scopes.is_some_and(|s| s.iter().any(|v| v == scope))
    };
    if !has("orchestration:read") {
        return Err("permission_denied".into());
    }
    Ok(json!({"configure":has("settings:write"),"operate":has("orchestration:operate")}))
}
fn string(value: &Value, max: usize) -> Result<&str> {
    value
        .as_str()
        .filter(|s| s.len() <= max && !s.contains('\0'))
        .ok_or("invalid_device_state".into())
}
fn required<'a>(v: &'a Value, key: &str, max: usize) -> Result<&'a str> {
    string(&v[key], max)
}
fn list<'a>(v: &'a Value, max: usize) -> Result<&'a Vec<Value>> {
    v.as_array()
        .filter(|a| a.len() <= max)
        .ok_or("invalid_device_state".into())
}
fn boolean(v: &Value) -> Result<bool> {
    v.as_bool().ok_or("invalid_device_state".into())
}
fn tool(v: &Value) -> Result<Value> {
    let versions = list(&v["installedVersions"], 128)?
        .iter()
        .map(|s| string(s, 256).map(str::to_owned))
        .collect::<Result<Vec<_>>>()?;
    if !v.get("runningVersion").is_some_and(|v| {
        v.is_null()
            || v.as_str()
                .is_some_and(|s| s.len() <= 256 && !s.contains('\0'))
    }) {
        return Err("invalid_device_state".into());
    }
    Ok(
        json!({"requiredVersion":required(v,"requiredVersion",256)?,"installedVersions":versions,"runningVersion":v["runningVersion"]}),
    )
}
pub fn present_host(v: &Value) -> Result<Value> {
    if !matches!(text(v, "kind"), "local" | "ssh") {
        return Err("invalid_device_state".into());
    }
    let platforms = list(&v["platforms"], 8)?
        .iter()
        .map(|p| {
            if !matches!(text(p, "platform"), "ios" | "android") {
                return Err("invalid_device_state".into());
            }
            let mut out = json!({"platform":p["platform"],"available":boolean(&p["available"])?});
            if let Some(reason) = p.get("reason") {
                out["reason"] = json!(string(reason, 4096)?);
            }
            Ok(out)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut out = json!({"id":required(v,"id",128)?,"kind":v["kind"],"label":required(v,"label",4096)?,"platforms":platforms,"hubInstalled":boolean(&v["hubInstalled"])?,"agentDeviceInstalled":boolean(&v["agentDeviceInstalled"])?});
    if let Some(tools) = v.get("tools") {
        out["tools"] = json!({"hub":tool(&tools["hub"])?,"agent":tool(&tools["agent"])?});
    }
    if let Some(error) = v.get("toolInspectionError") {
        out["toolInspectionError"] = json!(string(error, 4096)?);
    }
    Ok(out)
}
fn status(v: &Value) -> Result<&str> {
    v.as_str()
        .filter(|s| {
            matches!(
                *s,
                "disabled" | "idle" | "installing" | "starting" | "ready" | "failed"
            )
        })
        .ok_or("invalid_device_state".into())
}
fn device(v: &Value, session: bool) -> Result<Value> {
    if !matches!(text(v, "platform"), "ios" | "android") {
        return Err("invalid_device_state".into());
    }
    let mut out = json!({"hostId":required(v,"hostId",128)?,"id":required(v,"id",256)?,"name":required(v,"name",4096)?,"platform":v["platform"],"version":required(v,"version",256)?,"booted":boolean(&v["booted"])?,"physical":boolean(&v["physical"])?});
    if session {
        out["threadId"] = json!(required(v, "threadId", 2048)?);
    }
    Ok(out)
}
/// Strict bounded projection: no endpoint credentials, raw config or extra tool data.
pub fn present_state(v: &Value) -> Result<Value> {
    let hosts = list(&v["hosts"], 512)?
        .iter()
        .map(present_host)
        .collect::<Result<Vec<_>>>()?;
    let devices = list(&v["devices"], 2048)?
        .iter()
        .map(|v| device(v, false))
        .collect::<Result<Vec<_>>>()?;
    let sessions=list(&v["sessions"],4096)?.iter().map(|s|Ok(json!({"threadId":required(s,"threadId",2048)?,"hostId":required(s,"hostId",128)?,"deviceId":required(s,"deviceId",256)?,"platform":match text(s,"platform"){"ios"=>"ios","android"=>"android",_=>return Err("invalid_device_state".into())},"openedAt":required(s,"openedAt",128)?}))).collect::<Result<Vec<_>>>()?;
    let statuses = v["hostStatuses"]
        .as_object()
        .filter(|o| o.len() <= 512)
        .ok_or("invalid_device_state")?;
    let mut by_host = json!({});
    for (id, s) in statuses {
        if id.len() > 128 {
            return Err("invalid_device_state".into());
        }
        let mut out = json!({"status":status(&s["status"])?});
        if let Some(detail) = s.get("detail") {
            out["detail"] = json!(string(detail, 4096)?);
        }
        by_host[id] = out;
    }
    let revision = v["revision"].as_i64().ok_or("invalid_device_state")?;
    let mut out = json!({"hosts":hosts,"hostStatus":status(&v["hostStatus"])?,"hostStatuses":by_host,"devices":devices,"sessions":sessions,"onboardingCompleted":boolean(&v["onboardingCompleted"])?,"agentAccessEnabled":boolean(&v["agentAccessEnabled"])?,"revision":revision});
    for flag in [
        "supportsHostRetry",
        "supportsToolUpdate",
        "supportsToolInspection",
    ] {
        out[flag] = json!(v[flag] == true);
    }
    if let Some(detail) = v.get("hostStatusDetail") {
        out["hostStatusDetail"] = json!(string(detail, 4096)?);
    }
    if let Some(booting) = v.get("bootingDevices") {
        out["bootingDevices"] = json!(list(booting, 2048)?
            .iter()
            .map(|v| device(v, true))
            .collect::<Result<Vec<_>>>()?);
    }
    if out.to_string().len() > 2 * 1024 * 1024 {
        return Err("device_state_too_large".into());
    }
    Ok(out)
}
/// Snapshot first, exactly as native DeviceService.stateStream. Subscribing
/// never installs or starts tools (unlike device.list's default behavior).
pub fn subscribe_state(
    c: &protocol::Connection,
    cancel: impl Fn() -> bool,
    mut receive: impl FnMut(Value) -> Result<()>,
) -> Result<()> {
    c.subscribe("subscribeDeviceState", json!({}), cancel, |event| {
        if let Some(v) = event {
            receive(present_state(&v)?)?;
        }
        Ok(())
    })
}
fn state(c: &protocol::Connection) -> Result<Value> {
    let done = Cell::new(false);
    let start = Instant::now();
    let mut latest = None;
    subscribe_state(
        c,
        || done.get() || start.elapsed() > Duration::from_secs(8),
        |v| {
            latest = Some(v);
            done.set(true);
            Ok(())
        },
    )?;
    latest.ok_or("device_state_unavailable".into())
}
/// Native SshDeviceHostConfig. Alias/key paths are server-local data, never
/// interpreted as a local path or executed by ot3.
pub fn host(v: &Value) -> Result<Value> {
    let id = required(v, "id", 128)?;
    let label = required(v, "label", 4096)?;
    let target = required(v, "target", 4096)?;
    if id == "local"
        || id.is_empty()
        || !id.as_bytes()[0].is_ascii_alphanumeric()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        || label.trim().is_empty()
        || label.trim() != label
        || target.is_empty()
        || target.starts_with('-')
        || target.chars().any(char::is_whitespace)
    {
        return Err("invalid_device_host".into());
    }
    let mut out = json!({"id":id,"label":label,"target":target});
    if let Some(identity) = v.get("identityFile") {
        let s = string(identity, 4096)?;
        if s.is_empty() || s.trim() != s {
            return Err("invalid_device_host".into());
        }
        out["identityFile"] = json!(s);
    }
    if let Some(port) = v.get("port") {
        out["port"] = json!(port
            .as_u64()
            .filter(|p| *p >= 1 && *p <= 65535)
            .ok_or("invalid_device_host")?);
    }
    Ok(out)
}
fn settings(raw: &Value) -> Result<Value> {
    let mut ids = HashSet::new();
    let hosts = list(&raw["deviceHosts"], 512)?
        .iter()
        .map(|h| {
            let h = host(h)?;
            if !ids.insert(text(&h, "id").to_owned()) {
                return Err("invalid_device_host".into());
            }
            Ok(h)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"enableDeviceSupport":boolean(&raw["enableDeviceSupport"])?,"enableAgentDeviceAccess":boolean(&raw["enableAgentDeviceAccess"])?,"deviceOnboardingCompleted":boolean(&raw["deviceOnboardingCompleted"])?,"deviceHosts":hosts}),
    )
}
fn digest(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string()))
}
fn remember(e: &Environment, permissions: Value, settings: Value, state: Value) -> Result<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let out = json!({"environment":e.id,"originKey":digest(&json!(e.base_url)),"viewId":id,"permissions":permissions,"settings":settings,"state":state});
    let mut cache = views().lock().map_err(|_| "local_error")?;
    if cache.len() >= 128 {
        cache.clear();
    }
    cache.insert(
        id,
        json!({"origin":e.base_url,"environment":e.id,"settingsDigest":digest(&settings)}),
    );
    Ok(out)
}
pub fn read(e: &Environment) -> Result<Value> {
    let c = connection(e)?;
    let permissions = roles(&c)?;
    let settings = settings(&c.rpc("server.getSettings", json!({}))?)?;
    let state = state(&c)?;
    remember(e, permissions, settings, state)
}
/// Read-authorized progress stream, bound to the same origin as the view.
pub fn watch(
    e: &Environment,
    cancel: impl Fn() -> bool,
    mut receive: impl FnMut(&str, Value) -> Result<()>,
) -> Result<()> {
    let c = connection(e)?;
    roles(&c)?;
    let origin = digest(&json!(e.base_url));
    subscribe_state(&c, cancel, |state| receive(&origin, state))
}
/// Direct port of native deviceHostsSettings.logic.ts:updateDeviceHosts (MIT,
/// pinned T3 Code5a4e998). Preserve each environment's existing host ID/list.
pub fn update_hosts(
    hosts: &[Value],
    next: &Value,
    remove: bool,
    original: &Value,
) -> Result<Value> {
    let same = |a: &Value, b: &Value| {
        a["target"] == b["target"]
            && a["port"] == b["port"]
            && a["identityFile"] == b["identityFile"]
    };
    let destination = |h: &Value| -> Result<Option<&Value>> {
        let found = hosts.iter().filter(|c| same(c, h)).collect::<Vec<_>>();
        if found.len() > 1 {
            Err("ambiguous_device_host".into())
        } else {
            Ok(found.first().copied())
        }
    };
    let existing = if let Some(h) = hosts.iter().find(|h| h["id"] == original["id"]) {
        Some(h)
    } else {
        if let Some(existing) = destination(original)? {
            Some(existing)
        } else if remove {
            None
        } else {
            destination(next)?
        }
    };
    let mut out = hosts.to_vec();
    if remove {
        out.retain(|h| Some(&h["id"]) != existing.map(|h| &h["id"]));
    } else if let Some(existing) = existing {
        for h in &mut out {
            if h["id"] == existing["id"] {
                *h = next.clone();
                h["id"] = existing["id"].clone();
            }
        }
    } else {
        out.push(next.clone());
    }
    Ok(json!(out))
}
/// Lifecycle calls can legitimately outlast the shared short RPC deadline.
/// Same native Effect non-stream RPC framing, heartbeats, no retry/replay.
fn lifecycle_rpc(c: &protocol::Connection, tag: &str, payload: Value) -> Result<Value> {
    c.rpc_with_timeout(tag, payload, Duration::from_secs(300))
        .map_err(|error| match error.as_str() {
            "rpc_outcome_unknown" => "device_outcome_unknown".into(),
            "write_not_authorized" => "permission_denied".into(),
            "rpc_rejected" => "device_action_rejected".into(),
            _ => error,
        })
}

pub fn apply(e: &Environment, input: &Value) -> Result<Value> {
    let expected = views()
        .lock()
        .map_err(|_| "local_error")?
        .get(text(input, "viewId"))
        .cloned()
        .ok_or("device_view_expired")?;
    if expected["origin"] != e.base_url || expected["environment"] != e.id {
        return Err("device_scope_changed".into());
    }
    let locks = OPERATIONS.get_or_init(Default::default);
    let lock = {
        let mut locks = locks.lock().map_err(|_| "local_error")?;
        locks
            .entry(format!("{}/{}", e.id, e.base_url))
            .or_default()
            .clone()
    };
    let _guard = lock.try_lock().map_err(|_| "device_busy")?;
    let c = connection(e)?;
    let permissions = roles(&c)?;
    let raw = settings(&c.rpc("server.getSettings", json!({}))?)?;
    if expected["settingsDigest"] != digest(&raw) {
        return Err("device_settings_changed_reload".into());
    }
    let current = state(&c)?;
    let action = text(input, "action");
    if input.get("projectId").is_some_and(|v| !v.is_null())
        && !["inspect", "refresh", "update-tool"].contains(&action)
    {
        return Err("device_environment_only".into());
    }
    let (tag, payload) = match action {
        "hub" | "agent" => {
            if permissions["configure"] != true {
                return Err("permission_denied".into());
            }
            let enabled = input["enabled"].as_bool().ok_or("invalid_device_action")?;
            let payload = if action == "hub" {
                if enabled {
                    json!({"enabled":true,"onboardingCompleted":true})
                } else {
                    json!({"enabled":false,"agentAccessEnabled":false})
                }
            } else {
                json!({"agentAccessEnabled":enabled})
            };
            ("device.configure", payload)
        }
        "inspect" => {
            if current["supportsToolInspection"] != true {
                return Err("device_inspection_unsupported".into());
            }
            ("device.list", json!({"inspectOnly":true}))
        }
        "refresh" => ("device.list", json!({})),
        "update-tool" => {
            if permissions["operate"] != true {
                return Err("permission_denied".into());
            }
            if current["supportsToolUpdate"] != true {
                return Err("device_update_unsupported".into());
            }
            let tool = text(input, "tool");
            if !matches!(tool, "hub" | "agent") {
                return Err("invalid_device_action".into());
            }
            ("device.list", json!({"updateTool":tool}))
        }
        "retry-host" => {
            if permissions["operate"] != true {
                return Err("permission_denied".into());
            }
            let id = text(input, "hostId");
            if current["supportsHostRetry"] != true
                || !current["hosts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|h| h["id"] == id)
            {
                return Err("device_host_unavailable".into());
            }
            ("device.list", json!({"retryHostId":id}))
        }
        "test-host" => {
            if permissions["configure"] != true {
                return Err("permission_denied".into());
            }
            ("device.testHost", host(&input["host"])?)
        }
        "host-upsert" | "host-remove" => {
            if permissions["configure"] != true {
                return Err("permission_denied".into());
            }
            let h = host(&input["host"])?;
            let original = host(input.get("original").unwrap_or(&input["host"]))?;
            let next = update_hosts(
                raw["deviceHosts"].as_array().unwrap(),
                &h,
                action == "host-remove",
                &original,
            )?;
            (
                "server.updateSettings",
                json!({"patch":{"deviceHosts":next}}),
            )
        }
        _ => return Err("invalid_device_action".into()),
    };
    if ["installing", "starting"].contains(&text(&current, "hostStatus")) && action != "test-host" {
        return Err("device_busy".into());
    }
    if views()
        .lock()
        .map_err(|_| "local_error")?
        .remove(text(input, "viewId"))
        .is_none()
    {
        return Err("device_view_expired".into());
    }
    let result = lifecycle_rpc(&c, tag, payload.clone());
    let result = (|| -> Result<Value> {
        let ack = result?;
        if action == "test-host" {
            return Ok(json!({"host":present_host(&ack)?,"data":read(e)?}));
        }
        if tag != "server.updateSettings" {
            present_state(&ack)?;
        }
        let fresh = read(e)?;
        if tag == "device.configure" {
            for (source, key) in [
                ("enabled", "enableDeviceSupport"),
                ("agentAccessEnabled", "enableAgentDeviceAccess"),
                ("onboardingCompleted", "deviceOnboardingCompleted"),
            ] {
                if payload
                    .get(source)
                    .is_some_and(|v| *v != fresh["settings"][key])
                {
                    return Err("device_settings_changed_reload".into());
                }
            }
        }
        if tag == "server.updateSettings"
            && fresh["settings"]["deviceHosts"] != payload["patch"]["deviceHosts"]
        {
            return Err("device_settings_changed_reload".into());
        }
        Ok(json!({"data":fresh}))
    })();
    Ok(match result {
        Ok(value) => {
            json!({"state":"complete","environment":e.id,"action":action,"data":value["data"],"host":value["host"]})
        }
        Err(error) => {
            json!({"state":if matches!(error.as_str(),"device_action_rejected"|"permission_denied"){"failed"}else{"uncertain"},"environment":e.id,"action":action,"error":error})
        }
    })
}
