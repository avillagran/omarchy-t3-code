//! Native keybinding settings. Parsing/capture/defaults are reused from original T3 JS.
//! This adapter exposes only keybindings, never raw provider/server configuration.
use crate::{json, protocol, text, Environment, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
static VIEWS: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
fn views() -> &'static Mutex<HashMap<String, Value>> {
    VIEWS.get_or_init(Default::default)
}
fn permission(c: &protocol::Connection, scope: &str) -> Result<bool> {
    let s = c.request_json("GET", "/api/auth/session", None, false)?;
    Ok(s["authenticated"] == true
        && s.get("permissions")
            .or_else(|| s.get("scopes"))
            .and_then(Value::as_array)
            .is_some_and(|p| p.iter().any(|p| p == scope)))
}
fn authorize(c: &protocol::Connection, scope: &str) -> Result<()> {
    if permission(c, scope)? {
        Ok(())
    } else {
        Err("permission_denied".into())
    }
}
/// Subscribe to native change notifications without projecting raw server config
/// (which also contains provider settings) into UI IPC. A notification requires
/// a fresh bounded `read`, rather than installing a possibly stale stream value.
pub fn subscribe_changes(
    e: &Environment,
    cancel: impl Fn() -> bool,
    mut changed: impl FnMut() -> Result<()>,
) -> Result<()> {
    let c = e.connection(false)?;
    authorize(&c, "orchestration:read")?;
    c.subscribe("subscribeServerConfig", json!({}), &cancel, |event| {
        if cancel() { return Ok(()); }
        if let Some(event) = event {
            if event["version"] == 1
                && matches!(text(&event, "type"), "snapshot" | "keybindingsUpdated")
            {
                changed()?;
            }
        }
        Ok(())
    })
}
fn command(s: &str) -> bool {
    static COMMANDS: OnceLock<Vec<String>> = OnceLock::new();
    if COMMANDS
        .get_or_init(|| serde_json::from_str(include_str!("keybinding_commands.json")).unwrap())
        .iter()
        .any(|c| c == s)
    {
        return true;
    }
    s.strip_prefix("script.")
        .and_then(|s| s.strip_suffix(".run"))
        .is_some_and(|s| {
            !s.is_empty()
                && s.len() <= 24
                && s.as_bytes()[0].is_ascii_alphanumeric()
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}
fn string(v: &Value, max: usize) -> Result<&str> {
    v.as_str()
        .filter(|s| {
            !s.is_empty()
                && s.trim() == *s
                && s.chars().count() <= max
                && !s.chars().any(char::is_control)
        })
        .ok_or("invalid_keybinding".into())
}
fn ast(v: &Value, depth: usize) -> Result<Value> {
    if depth > 64 {
        return Err("invalid_keybinding".into());
    }
    match text(v, "type") {
        "identifier" => Ok(json!({"type":"identifier","name":string(&v["name"],256)?})),
        "not" => Ok(json!({"type":"not","node":ast(&v["node"],depth+1)?})),
        "and" | "or" => Ok(
            json!({"type":v["type"],"left":ast(&v["left"],depth+1)?,"right":ast(&v["right"],depth+1)?}),
        ),
        _ => Err("invalid_keybinding".into()),
    }
}
pub fn present(config: &Value) -> Result<Value> {
    let rules = config["keybindings"]
        .as_array()
        .filter(|a| a.len() <= 256)
        .ok_or("invalid_keybindings_config")?;
    let mut out = Vec::new();
    for rule in rules {
        let project = (|| -> Result<Value> {
            if !command(text(rule, "command")) {
                return Err("invalid_keybinding".into());
            }
            let key = rule["shortcut"]["key"]
                .as_str()
                .filter(|s| {
                    *s == " "
                        || !s.is_empty()
                            && s.trim() == *s
                            && s.chars().count() <= 64
                            && !s.chars().any(char::is_control)
                })
                .ok_or("invalid_keybinding")?;
            let mut shortcut = json!({"key":key});
            for flag in ["metaKey", "ctrlKey", "shiftKey", "altKey", "modKey"] {
                shortcut[flag] = json!(rule["shortcut"][flag]
                    .as_bool()
                    .ok_or("invalid_keybinding")?)
            }
            let mut r = json!({"command":rule["command"],"shortcut":shortcut});
            if let Some(a) = rule.get("whenAst") {
                r["whenAst"] = ast(a, 0)?
            }
            Ok(r)
        })();
        // Native ForwardCompatibleArray ignores rules this client cannot represent.
        if let Ok(rule) = project {
            out.push(rule)
        }
    }
    let issues=config["issues"].as_array().into_iter().flatten().filter(|i|text(i,"kind").starts_with("keybindings.")).take(64).map(|i|json!({"kind":text(i,"kind"),"message":text(i,"message").chars().take(2048).collect::<String>()})).collect::<Vec<_>>();
    let path = config["keybindingsConfigPath"]
        .as_str()
        .filter(|p| p.starts_with('/') && p.len() <= 4096 && !p.chars().any(char::is_control))
        .unwrap_or("");
    let editors: Vec<Value> = serde_json::from_str(include_str!("native_editors.json")).unwrap();
    let available = config["availableEditors"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let editors = editors
        .into_iter()
        .filter(|e| available.iter().any(|id| id == &e["id"]))
        .collect::<Vec<_>>();
    Ok(
        json!({"editors":editors,"keybindings":out,"issues":issues,"path":path,"revision":format!("{:x}",Sha256::digest(json!({"keybindings":config["keybindings"],"path":path}).to_string()))}),
    )
}
fn remember(e: &Environment, mut data: Value) -> Result<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    data["viewId"] = json!(id);
    data["environment"] = json!(e.id);
    data["originKey"] = json!(format!("{:x}", Sha256::digest(&e.base_url)));
    let mut cache = views().lock().map_err(|_| "local_error")?;
    if cache.len() >= 128 {
        cache.clear()
    }
    cache.insert(
        id,
        json!({"origin":e.base_url,"environment":e.id,"data":data}),
    );
    Ok(data)
}
pub fn read(e: &Environment) -> Result<Value> {
    let c = e.connection(false)?;
    authorize(&c, "orchestration:read")?;
    let mut data = present(&protocol::api::server::config(&c)?)?;
    data["writable"] = json!(e.connection(true)
        .and_then(|c| permission(&c, "settings:write"))
        .unwrap_or(false));
    data["canOpen"] = json!(e.connection(true)
        .and_then(|c| permission(&c, "orchestration:operate"))
        .unwrap_or(false));
    remember(e, data)
}
/// Exact native rule shape; no interpreted shell text or new keybinding algorithm.
pub fn payload(input: &Value) -> Result<Value> {
    fn rule(v: &Value) -> Result<Value> {
        if !command(text(v, "command")) {
            return Err("invalid_keybinding".into());
        }
        let mut r = json!({"command":v["command"],"key":string(&v["key"],64)?});
        if let Some(when) = v.get("when") {
            r["when"] = json!(string(when, 256)?)
        }
        Ok(r)
    }
    let mut p = rule(&input["input"])?;
    match text(input, "action") {
        "upsert" => {
            if let Some(replace) = input["input"].get("replace") {
                p["replace"] = rule(replace)?
            }
        }
        "remove" => {
            if input["input"].get("replace").is_some() {
                return Err("invalid_keybinding".into());
            }
        }
        _ => return Err("invalid_keybinding_action".into()),
    }
    Ok(p)
}
pub fn apply(e: &Environment, input: &Value) -> Result<Value> {
    let payload = payload(input)?;
    let id = text(input, "viewId");
    let expected = views()
        .lock()
        .map_err(|_| "local_error")?
        .get(id)
        .cloned()
        .ok_or("keybindings_snapshot_expired")?;
    if expected["origin"] != e.base_url || expected["environment"] != e.id {
        return Err("keybindings_scope_changed".into());
    }
    let c = e.connection(true)?;
    authorize(&c, "orchestration:read")?;
    authorize(&c, "settings:write")?;
    let fresh = present(&protocol::api::server::config(&c)?)?;
    if fresh["revision"] != expected["data"]["revision"] {
        return Err("keybindings_changed_reload".into());
    }
    // A token is consumed before dispatch; a lost response never implicitly replays it.
    if views()
        .lock()
        .map_err(|_| "local_error")?
        .remove(id)
        .is_none()
    {
        return Err("keybindings_snapshot_expired".into());
    }
    let method = if input["action"] == "upsert" {
        "server.upsertKeybinding"
    } else {
        "server.removeKeybinding"
    };
    let result = c.rpc(method, payload).and_then(|ack| {
        let ack = present(&ack)?;
        let mut data = present(&protocol::api::server::config(&c)?)?;
        if data["keybindings"] != ack["keybindings"] {
            return Err("keybindings_changed_reload".into());
        }
        data["writable"] = json!(true);
        data["canOpen"] = json!(permission(&c, "orchestration:operate").unwrap_or(false));
        remember(e, data)
    });
    match result {
        Ok(data) => Ok(json!({"state":"complete","environment":e.id,"data":data})),
        Err(error) => Ok(json!({"state":"uncertain","environment":e.id,"error":error})),
    }
}

/// Original shell.openInEditor, on the original environment and configured keybinding path.
pub fn open(e: &Environment, input: &Value) -> Result<Value> {
    let expected = views()
        .lock()
        .map_err(|_| "local_error")?
        .get(text(input, "viewId"))
        .cloned()
        .ok_or("keybindings_snapshot_expired")?;
    if expected["origin"] != e.base_url || expected["environment"] != e.id {
        return Err("keybindings_scope_changed".into());
    }
    let c = e.connection(true)?;
    authorize(&c, "orchestration:read")?;
    authorize(&c, "orchestration:operate")?;
    let fresh = present(&protocol::api::server::config(&c)?)?;
    if text(&fresh, "path").is_empty() || fresh["path"] != expected["data"]["path"] {
        return Err("keybindings_changed_reload".into());
    }
    let editors = fresh["editors"].as_array().ok_or("editor_unavailable")?;
    let editor = if let Some(id) = input.get("editor") {
        editors.iter().find(|e| e["id"] == *id)
    } else {
        editors.first()
    }
    .ok_or("editor_unavailable")?;
    c.rpc(
        "shell.openInEditor",
        json!({"cwd":fresh["path"],"editor":editor["id"]}),
    )?;
    Ok(json!({"environment":e.id,"opened":true}))
}
