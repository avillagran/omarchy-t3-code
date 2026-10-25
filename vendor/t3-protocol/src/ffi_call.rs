//! JSON ABI 3: `call` and `describe` over the typed API registry (`api`).
//! Stream functions go through `subscribe` with `fn` (`ffi_stream::target`).
//! `ffi.rs` only dispatches here. Every check that needs no network (function
//! name, `timeoutMs`, `args`) runs before a connection is even looked up.
use crate::{api, errors, Connection, Result};
use serde_json::{json, Value};

pub(crate) const ABI_VERSION: u64 = 3;

/// `{"op":"call","fn":"vcs.refreshStatus","args":{"cwd":"/r"},"timeoutMs"?:20000}`.
/// Sends exactly one request and never retries.
pub(crate) fn call(request: &Value, connection: impl FnOnce() -> Result<Connection>) -> Result<Value> {
    let name = request["fn"].as_str().filter(|s| !s.is_empty()).ok_or("invalid_input")?;
    let function = api::function(name).ok_or("unknown_function")?;
    if function.stream {
        return Err("stream_function".into());
    }
    let timeout = api::timeout(request.get("timeoutMs"))?;
    let args = match request.get("args") {
        None | Some(Value::Null) => json!({}),
        Some(args) => args.clone(),
    };
    let payload = function.payload(&args)?;
    api::send_with(&connection()?, function, payload, timeout.unwrap_or(function.timeout()))
}
/// `{"op":"describe"}`: the registry, without a connection.
pub(crate) fn describe() -> Value {
    api::describe()
}
/// Adds ABI 3 to the existing capabilities document; every earlier op stays listed.
pub(crate) fn capabilities(mut base: Value) -> Value {
    base["abiVersion"] = json!(ABI_VERSION);
    if let Some(ops) = base["operations"].as_array_mut() {
        for op in ["call", "describe"] {
            if !ops.iter().any(|o| o == op) {
                ops.push(json!(op));
            }
        }
    }
    base["functions"] = json!(api::functions().count());
    base["apiVersion"] = json!(api::API_VERSION);
    base["contract"] = json!(api::CONTRACT);
    base
}
/// `{"ok":false,"error":code}` plus the server's `message` and, for
/// `write_not_authorized`, the `requiredScope` it named.
pub(crate) fn error_envelope(error: &str) -> Value {
    let mut v = json!({"ok":false,"error":crate::error_code(error)});
    if let Some(message) = errors::message(error) {
        v["message"] = json!(message);
    }
    if let Some(scope) = errors::required_scope(error) {
        v["requiredScope"] = json!(scope);
    }
    v
}
