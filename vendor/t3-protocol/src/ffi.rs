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
        "capabilities" => Ok(
            json!({"abiVersion":2,"libraryVersion":env!("CARGO_PKG_VERSION"),
            "operations":["connectBearer","connectRelay","request","rpc","disconnect","subscribe","pollSubscription","cancelSubscription"],
            "nativeAttachmentUpload":true,"nativeImageDownload":true,"maxSubscriptions":4,
            "queueMaxItems":64,"queueMaxBytes":MAX_BYTES,"maxPollMs":1000}),
        ),
        "connectBearer" => {
            let mut h = handle.lock().map_err(|_| "internal_error")?;
            let connection = Connection::new(
                field(&request, "httpBaseUrl")?,
                request["wsBaseUrl"].as_str(),
                Credential::bearer(field(&request, "token")?)?,
            )?;
            h.cancel_all();
            h.connection = Some(connection);
            Ok(json!({"connected":true}))
        }
        "connectRelay" => {
            let mut h = handle.lock().map_err(|_| "internal_error")?;
            let connection = connect_relay(
                field(&request, "relayUrl")?,
                field(&request, "cloudJwt")?,
                field(&request, "clientId")?,
                field(&request, "environmentId")?,
                h.key.clone(),
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
        "request" => connection(handle)?.request_json(
            field(&request, "method")?,
            field(&request, "path")?,
            request.get("body"),
            request["protocol"].as_bool().unwrap_or(true),
        ),
        "rpc" => connection(handle)?.rpc(field(&request, "tag")?, request["payload"].clone()),
        "subscribe" => {
            let mut h = handle.lock().map_err(|_| "internal_error")?;
            if h.subscriptions.len() >= 4 {
                return Err("subscription_limit".into());
            }
            let connection = h.connection.clone().ok_or("not_connected")?;
            let tag = field(&request, "tag")?;
            if tag.len() > 256 || tag.chars().any(char::is_control) {
                return Err("invalid_input".into());
            }
            let subscription =
                Subscription::start(connection, tag.into(), request["payload"].clone())?;
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
        _ => Err("unknown_operation".into()),
    }
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
/// # Safety
/// `handle` is a live t3_client_new result; input addresses `len` readable bytes.
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
        if handle.is_null() || input.is_null() || len > MAX_BYTES as usize {
            return Err("invalid_input".into());
        }
        let bytes = std::slice::from_raw_parts(input, len);
        let request = serde_json::from_slice(bytes).map_err(|_| "malformed")?;
        call(&*handle.cast::<Mutex<Handle>>(), request)
    })
    .unwrap_or_else(|_| Err("internal_error".into()));
    response(result)
}
fn response(result: Result<Value>) -> *mut c_char {
    let response = match result {
        Ok(data) => json!({"ok":true,"data":data}),
        Err(code) => json!({"ok":false,"error":code}),
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
/// Callback borrows image bytes only for its duration, on the calling thread.
/// It must not unwind; copy bytes before returning. The host owns context.
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
        let bytes = connection(&*handle.cast::<Mutex<Handle>>())?.attachment_image(
            field(&request, "id")?,
            field(&request, "name")?,
            field(&request, "mimeType")?,
        )?;
        receive(context, bytes.as_ptr(), bytes.len());
        Ok(json!({"sizeBytes":bytes.len()}))
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
}
