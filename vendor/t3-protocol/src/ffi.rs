//! Small C ABI for Kotlin/JNI or other hosts. All I/O stays in the Rust library.
use super::*;
use crate::ffi_attachments::ForeignReader;
pub use crate::ffi_attachments::{ImageCallback, ReadCallback, ReleaseCallback};
use crate::ffi_stream::Subscription;
use std::collections::HashMap;
#[cfg(test)]
use std::ffi::CStr;
use std::{
    ffi::{c_char, CString},
    sync::Mutex,
};
struct Handle {
    connection: Option<Connection>,
    key: Arc<DpopKey>,
    subscriptions: HashMap<String, Arc<Subscription>>,
}
/// # Safety
/// Returned handle must be freed once, after all calls have completed.
#[no_mangle]
pub extern "C" fn t3_client_new() -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(Mutex::new(Handle {
        connection: None,
        key: Arc::new(DpopKey::generate()),
        subscriptions: HashMap::new(),
    })))
    .cast()
}
fn call(handle: &Mutex<Handle>, request: Value) -> Result<Value> {
    match field(&request, "op")? {
        "capabilities" => Ok(crate::ffi_stream::with_capabilities(crate::ffi_call::capabilities(
            json!({"abiVersion":3,"libraryVersion":env!("CARGO_PKG_VERSION"),
            "operations":["connectBearer","connectRelay","request","requestOk","rpc","rpcWithTimeout","disconnect","subscribe","pollSubscription","cancelSubscription","wakeSubscriptions","fetchDescriptor","pairBearer","boundedThread","assetBytes","projectFavicon"],
            "binaryOperations":["image","assetBytes","projectFavicon"],
            "connectionOptions":["requestTimeoutMs","connectTimeoutMs","poolIdleTimeoutMs","maxBytes"],
            "nativeAttachmentUpload":true,"nativeImageDownload":true,
            "maxAssetBytes":assets::MAX_ASSET_BYTES}),
        ))),
        "call" => crate::ffi_call::call(&request, || connection(handle)),
        "describe" => Ok(crate::ffi_call::describe()),
        "domain" => crate::ffi_call::domain(&request),
        "connectBearer" => {
            let options = options(&request)?;
            let mut h = handle.lock().map_err(|_| "internal_error")?;
            let connection = Connection::with_options(
                field(&request, "httpBaseUrl")?,
                request["wsBaseUrl"].as_str(),
                Credential::bearer(field(&request, "token")?)?,
                &options,
            )?;
            h.cancel_all();
            h.connection = Some(connection);
            Ok(json!({"connected":true}))
        }
        "connectRelay" => {
            let options = options(&request)?;
            let mut h = handle.lock().map_err(|_| "internal_error")?;
            let connection = connect_relay_with_options(
                field(&request, "relayUrl")?,
                field(&request, "cloudJwt")?,
                field(&request, "clientId")?,
                field(&request, "environmentId")?,
                h.key.clone(),
                &options,
            )?;
            h.cancel_all();
            h.connection = Some(connection);
            Ok(json!({"connected":true}))
        }
        "disconnect" => {
            let mut h = handle.lock().map_err(|_| "internal_error")?;
            h.cancel_all();
            h.connection = None;
            Ok(json!({"connected":false}))
        }
        "request" => {
            let c = connection(handle)?;
            let (method, path) = (field(&request, "method")?, field(&request, "path")?);
            let protocol = request["protocol"].as_bool().unwrap_or(true);
            match request.get("maxBytes") {
                None | Some(Value::Null) => c.request_json(method, path, request.get("body"), protocol),
                Some(v) => c.request_json_with_limit(method, path, request.get("body"), protocol, v.as_u64().ok_or("invalid_max_bytes")?),
            }
        }
        "requestOk" => connection(handle)?
            .request_ok(
                field(&request, "method")?,
                field(&request, "path")?,
                request.get("body"),
                request["protocol"].as_bool().unwrap_or(true),
            )
            .map(|_| json!({"succeeded":true})),
        "fetchDescriptor" => {
            let options = options(&request)?;
            let base = field(&request, "httpBaseUrl")?;
            match request["environmentId"].as_str() {
                Some(id) => verify_environment(base, id, &options),
                None => fetch_descriptor_with(base, &options),
            }
        }
        "pairBearer" => {
            let options = options(&request)?;
            let client = pairing_client(&request["client"])?;
            let paired = pair_bearer(
                field(&request, "httpBaseUrl")?,
                request["pairingCode"].as_str().ok_or("invalid_pairing_code")?,
                request["environmentId"].as_str(),
                &client,
                &options,
            )?;
            let connected = request["connect"].as_bool().unwrap_or(false);
            if connected {
                let connection = paired.connection(&options)?;
                let mut h = handle.lock().map_err(|_| "internal_error")?;
                h.cancel_all();
                h.connection = Some(connection);
            }
            Ok(json!({"accessToken":paired.access_token.as_str(),"tokenType":"Bearer",
                "environmentId":paired.environment_id,"descriptor":paired.descriptor,
                "scope":paired.scope,"expiresIn":paired.expires_in,"connected":connected}))
        }
        "boundedThread" => connection(handle)?.bounded_thread(field(&request, "threadId")?),
        // Bytes never travel through JSON: these go through t3_client_image.
        "assetBytes" | "projectFavicon" => Err("binary_operation".into()),
        "rpc" => connection(handle)?.rpc(field(&request, "tag")?, request["payload"].clone()),
        "rpcWithTimeout" => {
            // Validated before connecting: 1 ms to 300 s, as `rpc_with_timeout` requires.
            let timeout = request["timeoutMs"]
                .as_u64()
                .filter(|ms| (1..=300_000).contains(ms))
                .map(Duration::from_millis)
                .ok_or("invalid_rpc_timeout")?;
            connection(handle)?.rpc_with_timeout(field(&request, "tag")?, request["payload"].clone(), timeout)
        }
        "subscribe" => {
            let (tag, payload) = crate::ffi_stream::target(&request)?;
            let mut h = handle.lock().map_err(|_| "internal_error")?;
            let connection = h.connection.clone().ok_or("not_connected")?;
            crate::ffi_stream::admit(&h.subscriptions, &tag, &payload)?;
            let subscription = Subscription::start(connection, tag, payload)?;
            let id = uuid::Uuid::new_v4().to_string();
            h.subscriptions.insert(id.clone(), Arc::new(subscription));
            Ok(json!({"subscriptionId":id}))
        }
        "pollSubscription" => {
            let id = field(&request, "subscriptionId")?;
            let timeout = match request.get("timeoutMs") {
                None => 1000,
                Some(v) => v.as_u64().filter(|v| *v <= 1000).ok_or("invalid_input")?,
            };
            let subscription = handle
                .lock()
                .map_err(|_| "internal_error")?
                .subscriptions
                .get(id)
                .cloned()
                .ok_or("unknown_subscription")?;
            subscription.poll(timeout)
        }
        "cancelSubscription" => {
            let id = field(&request, "subscriptionId")?;
            let subscription = handle
                .lock()
                .map_err(|_| "internal_error")?
                .subscriptions
                .remove(id);
            if let Some(s) = &subscription {
                s.cancel();
            }
            Ok(json!({"cancelled":subscription.is_some()}))
        }
        "wakeSubscriptions" => crate::ffi_stream::wake_subscriptions(
            &handle.lock().map_err(|_| "internal_error")?.subscriptions,
            &request,
        ),
        _ => Err("unknown_operation".into()),
    }
}
/// Optional `options`: {requestTimeoutMs, connectTimeoutMs, poolIdleTimeoutMs,
/// maxBytes}. Absent keys keep the defaults; null clears connect/pool timeouts.
fn options(request: &Value) -> Result<ConnectionOptions> {
    let mut o = ConnectionOptions::default();
    let map = match request.get("options") {
        None | Some(Value::Null) => return Ok(o),
        Some(Value::Object(map)) => map,
        Some(_) => return Err("invalid_connection_options".into()),
    };
    let ms = |v: &Value| v.as_u64().map(Duration::from_millis).ok_or("invalid_connection_options");
    for (key, value) in map {
        o = match (key.as_str(), value) {
            ("requestTimeoutMs", v) => o.with_request_timeout(ms(v)?),
            ("connectTimeoutMs", Value::Null) => o.with_connect_timeout(None),
            ("connectTimeoutMs", v) => o.with_connect_timeout(Some(ms(v)?)),
            ("poolIdleTimeoutMs", Value::Null) => o.with_pool_idle_timeout(None),
            ("poolIdleTimeoutMs", v) => o.with_pool_idle_timeout(Some(ms(v)?)),
            ("maxBytes", v) => o.with_max_bytes(v.as_u64().ok_or("invalid_connection_options")?),
            _ => return Err("invalid_connection_options".into()),
        };
    }
    o.validate()?;
    Ok(o)
}
/// Optional `client`: {label, deviceType, os}.
fn pairing_client(client: &Value) -> Result<PairingClient> {
    let mut c = PairingClient::default();
    let map = match client {
        Value::Null => return Ok(c),
        Value::Object(map) => map,
        _ => return Err("invalid_pairing_client".into()),
    };
    for (key, value) in map {
        let value = value.as_str().ok_or("invalid_pairing_client")?;
        c = match key.as_str() {
            "label" => c.with_label(value),
            "deviceType" => c.with_device_type(value),
            "os" => c.with_os(value),
            _ => return Err("invalid_pairing_client".into()),
        };
    }
    Ok(c)
}
fn connection(handle: &Mutex<Handle>) -> Result<Connection> {
    handle
        .lock()
        .map_err(|_| "internal_error")?
        .connection
        .clone()
        .ok_or_else(|| "not_connected".into())
}
impl Handle {
    fn cancel_all(&mut self) {
        for subscription in self.subscriptions.values() {
            subscription.cancel();
        }
        self.subscriptions.clear();
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        for subscription in self.subscriptions.values() {
            subscription.cancel();
        }
    }
}
/// Largest `domain` request: a bounded thread snapshot plus its envelope. Every
/// other operation keeps the 8 MiB input limit.
pub const DOMAIN_MAX_BYTES: u64 = THREAD_SNAPSHOT_MAX_BYTES + MAX_BYTES;
/// # Safety
/// `handle` is a live t3_client_new result; input addresses `len` readable bytes
/// (at most 8 MiB, or `DOMAIN_MAX_BYTES` for `{"op":"domain"}`).
/// The UTF-8 response is owned by Rust and must be freed with t3_string_free.
/// Calls may run concurrently; poll never holds the connection lock while waiting.
/// Do not free a handle while a call is active.
#[no_mangle]
pub unsafe extern "C" fn t3_client_call(
    handle: *mut std::ffi::c_void,
    input: *const u8,
    len: usize,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        if handle.is_null() || input.is_null() || len > DOMAIN_MAX_BYTES as usize {
            return Err("invalid_input".into());
        }
        let bytes = std::slice::from_raw_parts(input, len);
        let request: Value = serde_json::from_slice(bytes).map_err(|_| "malformed")?;
        // Only the pure domain functions take thread-sized input (a bounded projection).
        if len > MAX_BYTES as usize && request["op"] != "domain" {
            return Err("invalid_input".into());
        }
        call(&*handle.cast::<Mutex<Handle>>(), request)
    })
    .unwrap_or_else(|_| Err("internal_error".into()));
    response(result)
}
fn response(result: Result<Value>) -> *mut c_char {
    let response = match result {
        Ok(data) => json!({"ok":true,"data":data}),
        // `error` stays a stable code; a server's reason travels separately.
        Err(error) => crate::ffi_call::error_envelope(&error),
    };
    CString::new(response.to_string()).unwrap().into_raw()
}
/// # Safety
/// Takes ownership of context if both callbacks are present, including on errors.
/// Callbacks must be thread-safe, must not unwind, and remain valid until release.
/// The live handle and metadata must remain valid during this call.
#[no_mangle]
pub unsafe extern "C" fn t3_client_upload(
    handle: *mut std::ffi::c_void,
    input: *const u8,
    len: usize,
    read: Option<ReadCallback>,
    release: Option<ReleaseCallback>,
    context: *mut std::ffi::c_void,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        let reader = ForeignReader {
            read: read.ok_or("invalid_callback")?,
            release: release.ok_or("invalid_callback")?,
            context: context as usize,
        };
        let request = input_json(handle, input, len)?;
        let connection = connection(&*handle.cast::<Mutex<Handle>>())?;
        connection.upload_attachment(
            field(&request, "name")?,
            field(&request, "mimeType")?,
            request["sizeBytes"].as_u64().ok_or("invalid_file")?,
            reader,
        )
    })
    .unwrap_or_else(|_| Err("internal_error".into()));
    response(result)
}
/// # Safety
/// Callback borrows the bytes only for its duration, on the calling thread.
/// It must not unwind; copy bytes before returning. The host owns context.
/// Metadata without `op` reads an attachment image; `op` may also be
/// `assetBytes` or `projectFavicon`. The callback is not called when no
/// favicon exists or on any error.
#[no_mangle]
pub unsafe extern "C" fn t3_client_image(
    handle: *mut std::ffi::c_void,
    input: *const u8,
    len: usize,
    receive: Option<ImageCallback>,
    context: *mut std::ffi::c_void,
) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        let receive = receive.ok_or("invalid_callback")?;
        let request = input_json(handle, input, len)?;
        let connection = connection(&*handle.cast::<Mutex<Handle>>())?;
        match request.get("op").and_then(Value::as_str) {
            None => {
                let bytes = connection.attachment_image(
                    field(&request, "id")?,
                    field(&request, "name")?,
                    field(&request, "mimeType")?,
                )?;
                receive(context, bytes.as_ptr(), bytes.len());
                Ok(json!({"sizeBytes":bytes.len()}))
            }
            Some("assetBytes") => {
                let limit = request["maxBytes"].as_u64().ok_or("invalid_asset_limit")?;
                let mut options = AssetOptions::new(limit);
                if let Some(ms) = request.get("timeoutMs").filter(|v| !v.is_null()) {
                    options = options.with_timeout(Duration::from_millis(ms.as_u64().ok_or("invalid_asset_timeout")?));
                }
                let resource = request.get("resource").filter(|r| r.is_object()).ok_or("invalid_asset_resource")?;
                let asset = connection.asset_bytes_with(resource.clone(), &options)?;
                receive(context, asset.bytes.as_ptr(), asset.bytes.len());
                Ok(json!({"sizeBytes":asset.bytes.len(),"mimeType":asset.mime}))
            }
            Some("projectFavicon") => {
                let path = match request.get("path") {
                    None | Some(Value::Null) => None,
                    Some(p) => Some(p.as_str().ok_or("invalid_asset_resource")?),
                };
                match connection.project_favicon(field(&request, "cwd")?, path)? {
                    Some(icon) => {
                        receive(context, icon.bytes.as_ptr(), icon.bytes.len());
                        Ok(json!({"found":true,"sizeBytes":icon.bytes.len(),"mimeType":icon.mime}))
                    }
                    None => Ok(json!({"found":false,"sizeBytes":0})),
                }
            }
            Some(_) => Err("unknown_operation".into()),
        }
    })
    .unwrap_or_else(|_| Err("internal_error".into()));
    response(result)
}
unsafe fn input_json(handle: *mut std::ffi::c_void, input: *const u8, len: usize) -> Result<Value> {
    if handle.is_null() || input.is_null() || len > MAX_BYTES as usize {
        return Err("invalid_input".into());
    }
    serde_json::from_slice(std::slice::from_raw_parts(input, len)).map_err(|_| "malformed".into())
}
/// # Safety
/// Free only a non-freed pointer returned by t3_client_call, or null.
#[no_mangle]
pub unsafe extern "C" fn t3_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}
/// # Safety
/// Free only a live handle, once, with no concurrent calls; null is accepted.
#[no_mangle]
pub unsafe extern "C" fn t3_client_free(handle: *mut std::ffi::c_void) {
    if !handle.is_null() {
        drop(Box::from_raw(handle.cast::<Mutex<Handle>>()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c_abi_ownership_and_errors() {
        unsafe {
            let h = t3_client_new();
            let input = br#"{"op":"request","method":"GET","path":"/api/orchestration/shell"}"#;
            let response = t3_client_call(h, input.as_ptr(), input.len());
            let v: Value = serde_json::from_slice(CStr::from_ptr(response).to_bytes()).unwrap();
            assert_eq!(v, json!({"ok":false,"error":"not_connected"}));
            t3_string_free(response);
            let input=br#"{"op":"connectBearer","httpBaseUrl":"https://t3.example","token":"fixture-secret"}"#;
            let response = t3_client_call(h, input.as_ptr(), input.len());
            assert_eq!(
                serde_json::from_slice::<Value>(CStr::from_ptr(response).to_bytes()).unwrap()["ok"],
                true
            );
            t3_string_free(response);
            let response = t3_client_call(std::ptr::null_mut(), input.as_ptr(), input.len());
            assert_eq!(
                serde_json::from_slice::<Value>(CStr::from_ptr(response).to_bytes()).unwrap()
                    ["error"],
                "invalid_input"
            );
            t3_string_free(response);
            t3_client_free(h);
        }
    }
    #[test]
    fn c_abi_errors_keep_a_stable_code_and_carry_the_reason_apart() {
        unsafe {
            let read = |r: *mut c_char| {
                let v: Value = serde_json::from_slice(CStr::from_ptr(r).to_bytes()).unwrap();
                t3_string_free(r);
                v
            };
            assert_eq!(
                read(response(Err("rpc_rejected: Project p-1 was not found.".into()))),
                json!({"ok":false,"error":"rpc_rejected","message":"Project p-1 was not found."})
            );
            assert_eq!(read(response(Err("not_connected".into()))), json!({"ok":false,"error":"not_connected"}));
        }
    }
}
